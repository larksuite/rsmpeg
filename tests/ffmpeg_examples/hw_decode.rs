//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/hw_decode.c
//! HW-accelerated decoding example using rsmpeg
use anyhow::{bail, Context, Result};
use once_cell::sync::OnceCell;
use rsmpeg::{
    avcodec::AVCodecContext,
    avformat::AVFormatContextInput,
    avutil::{
        hwdevice_find_type_by_name, hwdevice_get_type_name, hwdevice_iterate_types, AVFrame,
        AVHWDeviceContext, AVHWDeviceType, AVImage, AVPixelFormat,
    },
    build_array, ffi,
};
use std::{
    ffi::CStr,
    fs::{self, File},
    io::Write,
    path::Path,
};

static HW_PIX_FMT: OnceCell<ffi::AVPixelFormat> = OnceCell::new();

fn hw_decoder_init(
    ctx: &mut AVCodecContext,
    device_type: AVHWDeviceType,
) -> Result<AVHWDeviceContext> {
    let hw_device_ctx = AVHWDeviceContext::create(device_type, None, None, 0)
        .context("Failed to create specified HW device.")?;
    ctx.set_hw_device_ctx(hw_device_ctx.clone());
    Ok(hw_device_ctx)
}

unsafe extern "C" fn get_hw_format<'a>(
    _ctx: *mut ffi::AVCodecContext,
    pix_fmts: *const AVPixelFormat,
) -> AVPixelFormat {
    let pix_fmts = unsafe { build_array(pix_fmts, ffi::AV_PIX_FMT_NONE) }.unwrap_or_default();
    for &pix_fmt in pix_fmts {
        if pix_fmt == *HW_PIX_FMT.get().unwrap() {
            return pix_fmt;
        }
    }
    eprintln!("Failed to get HW surface format.");
    ffi::AV_PIX_FMT_NONE
}

fn decode_write(
    dec_ctx: &mut AVCodecContext,
    packet: Option<&rsmpeg::avcodec::AVPacket>,
    output_file: &mut File,
) -> Result<()> {
    dec_ctx.send_packet(packet)?;
    loop {
        let frame = match dec_ctx.receive_frame() {
            Ok(f) => f,
            Err(rsmpeg::error::RsmpegError::DecoderDrainError)
            | Err(rsmpeg::error::RsmpegError::DecoderFlushedError) => break,
            Err(e) => bail!(e),
        };
        let tmp_frame = if frame.format == *HW_PIX_FMT.get().unwrap() {
            let mut sw_frame = AVFrame::new();
            sw_frame
                .hwframe_transfer_data(&frame)
                .context("Error transferring the data to system memory")?;
            sw_frame
        } else {
            frame
        };
        let size = AVImage::get_buffer_size(tmp_frame.format, tmp_frame.width, tmp_frame.height, 1)
            .context("Get image buffer size failed.")?;
        let mut buffer = vec![0u8; size as usize];
        tmp_frame
            .image_copy_to_buffer(&mut buffer, 1)
            .context("Can not copy image to buffer")?;
        output_file.write_all(&buffer)?;
    }
    Ok(())
}

fn hw_decode(device_type_raw: &CStr, input_file: &CStr, output_file: &CStr) -> Result<()> {
    let device_type = hwdevice_find_type_by_name(device_type_raw);
    if device_type == ffi::AV_HWDEVICE_TYPE_NONE {
        bail!(
            "Device type {} is not supported.",
            device_type_raw.to_string_lossy()
        );
    }
    let mut input_ctx = AVFormatContextInput::open(input_file)?;
    let (video_stream_idx, decoder) = input_ctx
        .find_best_stream(ffi::AVMEDIA_TYPE_VIDEO)?
        .context("No video stream")?;
    let mut dec_ctx = AVCodecContext::new(&decoder);
    dec_ctx.apply_codecpar(&input_ctx.streams()[video_stream_idx].codecpar())?;
    let _hw_ctx = hw_decoder_init(&mut dec_ctx, device_type)?;
    // Find supported hw_pix_fmt
    let mut found = false;
    for i in 0.. {
        let Some(config) = decoder.hw_config(i) else {
            break;
        };
        if config.methods & ffi::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32 != 0
            && config.device_type == device_type
        {
            HW_PIX_FMT.set(config.pix_fmt).unwrap();
            found = true;
            break;
        }
    }
    if !found {
        bail!(
            "Decoder {} does not support device type {}",
            decoder.name().to_string_lossy(),
            hwdevice_get_type_name(device_type)
                .map(|x| x.to_string_lossy())
                .unwrap_or_default()
        );
    }
    dec_ctx.set_get_format(Some(get_hw_format));
    dec_ctx.open(None)?;
    let output_file = Path::new(output_file.to_str().unwrap());
    let _ = fs::create_dir_all(output_file.parent().unwrap());
    let mut output_file = File::create(output_file)?;
    while let Some(pkt) = input_ctx.read_packet()? {
        if pkt.stream_index as usize == video_stream_idx {
            decode_write(&mut dec_ctx, Some(&pkt), &mut output_file)?;
        }
    }
    // flush
    decode_write(&mut dec_ctx, None, &mut output_file)?;
    Ok(())
}

#[test]
fn test_hw_decode() {
    let device_type = hwdevice_iterate_types().next().unwrap();
    // ffplay -f rawvideo -video_size 320x180 tests/output/transcode/bear.frames
    hw_decode(
        hwdevice_get_type_name(device_type).expect("Failed to get device type name"),
        c"tests/assets/vids/bear.mp4",
        c"tests/output/transcode/bear.frames",
    )
    .unwrap();
}
