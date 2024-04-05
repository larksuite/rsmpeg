//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/encode_video.c
use anyhow::{anyhow, Context, Result};
use cstr::cstr;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avutil::{opt_set, ra, AVFrame},
    error::RsmpegError,
    ffi::{self},
};
use std::{
    ffi::CStr,
    fs::{self, File},
    io::{BufWriter, Write},
};

const WIDTH: usize = 352;
const HEIGHT: usize = 288;

fn encode(
    encode_context: &mut AVCodecContext,
    frame: Option<&AVFrame>,
    file: &mut BufWriter<File>,
) -> Result<()> {
    encode_context.send_frame(frame)?;
    loop {
        let packet = match encode_context.receive_packet() {
            Ok(packet) => packet,
            Err(RsmpegError::EncoderDrainError) | Err(RsmpegError::EncoderFlushedError) => break,
            Err(e) => return Err(e.into()),
        };
        let data = unsafe { std::slice::from_raw_parts(packet.data, packet.size as usize) };
        file.write_all(data)?;
    }
    Ok(())
}

fn encode_video(codec_name: &CStr, file_name: &str) -> Result<()> {
    let encoder =
        AVCodec::find_encoder_by_name(codec_name).context("Failed to find encoder codec")?;
    let mut encode_context = AVCodecContext::new(&encoder);
    encode_context.set_bit_rate(400000);
    encode_context.set_width(WIDTH as i32);
    encode_context.set_height(HEIGHT as i32);
    encode_context.set_time_base(ra(1, 25));
    encode_context.set_framerate(ra(25, 1));
    encode_context.set_gop_size(10);
    encode_context.set_max_b_frames(1);
    encode_context.set_pix_fmt(ffi::AV_PIX_FMT_YUV420P);
    if encoder.id == ffi::AV_CODEC_ID_H264 {
        unsafe { opt_set(encode_context.priv_data, cstr!("preset"), cstr!("slow"), 0) }
            .context("Set preset failed.")?;
    }
    encode_context.open(None).context("Could not open codec")?;

    let mut frame = AVFrame::new();
    frame.set_format(encode_context.pix_fmt);
    frame.set_width(encode_context.width);
    frame.set_height(encode_context.height);
    frame
        .alloc_buffer()
        .context("Could not allocate the video frame data")?;

    let file = File::create(file_name).with_context(|| anyhow!("Could not open: {}", file_name))?;
    let mut writer = BufWriter::new(file);

    for i in 0..25 {
        frame
            .make_writable()
            .context("Failed to make frame writable")?;
        // prepare colorful frame
        {
            let data = frame.data;
            let linesize = frame.linesize;
            let linesize_y = linesize[0] as usize;
            let linesize_cb = linesize[1] as usize;
            let linesize_cr = linesize[2] as usize;
            let y_data = unsafe { std::slice::from_raw_parts_mut(data[0], HEIGHT * linesize_y) };
            let cb_data =
                unsafe { std::slice::from_raw_parts_mut(data[1], HEIGHT / 2 * linesize_cb) };
            let cr_data =
                unsafe { std::slice::from_raw_parts_mut(data[2], HEIGHT / 2 * linesize_cr) };
            // prepare a dummy image
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    y_data[y * linesize_y + x] = (x + y + i * 3) as u8;
                }
            }

            for y in 0..HEIGHT / 2 {
                for x in 0..WIDTH / 2 {
                    cb_data[y * linesize_cb + x] = (128 + y + i * 2) as u8;
                    cr_data[y * linesize_cr + x] = (64 + x + i * 5) as u8;
                }
            }
        }

        frame.set_pts(i as i64);

        encode(&mut encode_context, Some(&frame), &mut writer)?;
    }
    encode(&mut encode_context, None, &mut writer)?;

    let endcode: [u8; 4] = [0, 0, 1, 0xb7];
    writer.write_all(&endcode).context("Write endcode failed")?;

    writer.flush().context("Flush file failed.")
}

#[test]
fn encode_video_test() {
    fs::create_dir_all("tests/output/encode_video/").unwrap();
    encode_video(cstr!("mpeg4"), "tests/output/encode_video/output.mp4").unwrap();
}
