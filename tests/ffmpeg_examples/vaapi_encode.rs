//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/vaapi_encode.c
use anyhow::{Context, Result};
use cstr::cstr;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avutil::{ra, AVFrame, AVHWDeviceContext},
    error::RsmpegError,
    ffi::{
        AVHWDeviceType, AVPixelFormat, AV_HWDEVICE_TYPE_CUDA, AV_HWDEVICE_TYPE_VAAPI,
        AV_PIX_FMT_CUDA, AV_PIX_FMT_NV12, AV_PIX_FMT_VAAPI,
    },
};
use std::{
    ffi::CStr,
    fs::File,
    io::{self, Read, Write},
    path::Path,
    slice,
};

fn set_hwframe_ctx(
    avctx: &mut AVCodecContext,
    hw_device_ctx: &AVHWDeviceContext,
    width: i32,
    height: i32,
    hw_format: AVPixelFormat,
    sw_format: AVPixelFormat,
) -> Result<()> {
    let mut hw_frames_ref = hw_device_ctx.hwframe_ctx_alloc();
    hw_frames_ref.data().format = hw_format;
    hw_frames_ref.data().sw_format = sw_format;
    hw_frames_ref.data().width = width;
    hw_frames_ref.data().height = height;
    hw_frames_ref.data().initial_pool_size = 20;

    hw_frames_ref
        .init()
        .context("Failed to initialize VAAPI frame context")?;

    avctx.set_hw_frames_ctx(hw_frames_ref);

    Ok(())
}

fn encode_write(
    avctx: &mut AVCodecContext,
    frame: Option<&AVFrame>,
    fout: &mut File,
) -> Result<()> {
    avctx.send_frame(frame).context("Send frame failed")?;
    loop {
        let mut packet = match avctx.receive_packet() {
            Ok(packet) => packet,
            Err(RsmpegError::EncoderDrainError) | Err(RsmpegError::EncoderFlushedError) => {
                break;
            }
            Err(e) => Err(e).context("Receive packet failed.")?,
        };
        packet.set_stream_index(0);
        let data = unsafe { slice::from_raw_parts(packet.data, packet.size as usize) };
        fout.write_all(data).context("Write output frame failed.")?;
    }
    Ok(())
}

fn hw_encode(
    input: &Path,
    output: &Path,
    width: i32,
    height: i32,
    encoder: &CStr,
    device_type: AVHWDeviceType,
    hw_format: AVPixelFormat,
    sw_format: AVPixelFormat,
) -> Result<()> {
    let size = width as usize * height as usize;

    let mut fin = File::open(input).context("Fail to open input file")?;
    let mut fout = File::create(output).context("Fail to open output file")?;

    let hw_device_ctx = AVHWDeviceContext::create(device_type, None, None, 0)
        .context("Failed to create a VAAPI device")?;

    let codec = AVCodec::find_encoder_by_name(encoder).context("Could not find encoder.")?;

    let mut avctx = AVCodecContext::new(&codec);

    avctx.set_width(width);
    avctx.set_height(height);
    avctx.set_time_base(ra(1, 25));
    avctx.set_framerate(ra(25, 1));
    avctx.set_sample_aspect_ratio(ra(1, 1));
    avctx.set_pix_fmt(hw_format);

    set_hwframe_ctx(
        &mut avctx,
        &hw_device_ctx,
        width,
        height,
        hw_format,
        sw_format,
    )
    .context("Failed to set hwframe context.")?;

    avctx
        .open(None)
        .context("Cannot open video encoder codec")?;

    loop {
        let mut sw_frame = AVFrame::new();

        // read data into software frame, and transfer them into hw frame
        sw_frame.set_width(width);
        sw_frame.set_height(height);
        sw_frame.set_format(sw_format);
        sw_frame.get_buffer(0).context("Get buffer failed.")?;

        let y = unsafe { slice::from_raw_parts_mut(sw_frame.data_mut()[0], size) };
        match fin.read_exact(y) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            e @ Err(_) => e.context("Read Y failed.")?,
        }
        let uv = unsafe { slice::from_raw_parts_mut(sw_frame.data_mut()[1], size / 2) };
        match fin.read_exact(uv) {
            Ok(()) => {}
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => break,
            e @ Err(_) => e.context("Read UV failed.")?,
        }

        let mut hw_frame = AVFrame::new();
        avctx
            .hw_frames_ctx_mut()
            .unwrap()
            .get_buffer(&mut hw_frame)
            .context("Get buffer failed")?;
        hw_frame
            .hwframe_transfer_data(&sw_frame)
            .context("Error while transferring frame data to surface.")?;

        encode_write(&mut avctx, Some(&hw_frame), &mut fout).context("Failed to encode.")?;
    }

    encode_write(&mut avctx, None, &mut fout).context("Failed to encode.")?;
    Ok(())
}

#[test]
#[ignore = "Github actions doesn't have vaapi device"]
fn vaapi_encode_test_vaapi() {
    std::fs::create_dir_all("tests/output/vaapi_encode/").unwrap();
    // Produced by ffmpeg -i tests/assets/vids/bear.mp4 -pix_fmt nv12 tests/assets/vids/bear.yuv
    hw_encode(
        Path::new("tests/assets/vids/bear.yuv"),
        Path::new("tests/output/vaapi_encode/vaapi_encode_test_vaapi.h264"),
        320,
        180,
        cstr!("h264_vaapi"),
        AV_HWDEVICE_TYPE_VAAPI,
        AV_PIX_FMT_VAAPI,
        AV_PIX_FMT_NV12,
    )
    .unwrap();
}

/// You should test this with nvenc enabled in compilation(e.g. utils/linux_ffmpeg.rs) https://trac.ffmpeg.org/wiki/HWAccelIntro#NVENC
///
/// I use this rather than vaapi test(since I don't have a vaapi compatible device).
///
/// They are almost the same, the only differences:
///
/// - device_type:  AV_HWDEVICE_TYPE_CUDA for nvenc,    AV_HWDEVICE_TYPE_VAAPI for vaapi
/// - hw_format:    AV_PIX_FMT_CUDA for nvenc,          AV_PIX_FMT_VAAPI for vaapi
#[test]
#[ignore = "Github actions doesn't have nvdia graphics card"]
fn vaapi_encode_test_nvenc() {
    std::fs::create_dir_all("tests/output/vaapi_encode/").unwrap();
    // Produced by ffmpeg -i tests/assets/vids/bear.mp4 -pix_fmt nv12 tests/assets/vids/bear.yuv
    hw_encode(
        Path::new("tests/assets/vids/bear.yuv"),
        Path::new("tests/output/vaapi_encode/vaapi_encode_test_nvenc.h264"),
        320,
        180,
        cstr!("h264_nvenc"),
        AV_HWDEVICE_TYPE_CUDA,
        AV_PIX_FMT_CUDA,
        AV_PIX_FMT_NV12,
    )
    .unwrap();
}
