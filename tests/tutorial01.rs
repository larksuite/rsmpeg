//! Ported from http://dranger.com/ffmpeg/tutorial01.c
//!
//! Extracts the first five rgb frames from the video and save them as ppm
//! files.

use anyhow::{Context, Result};
use cstr::cstr;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext},
    avformat::AVFormatContextInput,
    avutil::{AVFrame, AVFrameWithImage, AVImage},
    error::RsmpegError,
    ffi,
    swscale::SwsContext,
};
use std::{
    ffi::CStr,
    fs::{self, File},
    io::prelude::*,
    slice,
};

/// Save a `AVFrame` as *colorful* pgm file.
fn pgm_save(frame: &AVFrame, filename: &str) -> Result<()> {
    // Here we only capture the first layer of frame.
    let data = frame.data[0];
    let linesize = frame.linesize[0] as usize;

    let width = frame.width as usize;
    let height = frame.height as usize;

    let buffer = unsafe { slice::from_raw_parts(data, height * linesize * 3) };

    // Create pgm file
    let mut pgm_file = File::create(filename)?;

    // Write pgm header(P6 means colorful)
    pgm_file.write_all(&format!("P6\n{} {}\n{}\n", width, height, 255).into_bytes())?;

    // Write pgm data
    for i in 0..height {
        // Here the linesize is bigger than width * 3.
        pgm_file.write_all(&buffer[i * linesize..i * linesize + width * 3])?;
    }
    Ok(())
}

fn _main(file: &CStr, out_dir: &str) -> Result<()> {
    fs::create_dir_all(out_dir)?;
    let mut input_format_context = AVFormatContextInput::open(file)?;
    input_format_context.dump(0, file)?;
    let video_stream_index = input_format_context
        .streams()
        .into_iter()
        .position(|stream| stream.codecpar().codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)
        .context("No video stream")?;
    let mut decode_context = {
        let video_stream = input_format_context
            .streams()
            .get(video_stream_index)
            .unwrap();
        let decoder = AVCodec::find_decoder(video_stream.codecpar().codec_id)
            .context("Cannot find the decoder for video stream")?;
        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context.apply_codecpar(&video_stream.codecpar())?;
        decode_context.open(None)?;
        decode_context
    };

    let image_buffer = AVImage::new(
        ffi::AVPixelFormat_AV_PIX_FMT_RGB24,
        decode_context.width,
        decode_context.height,
        1,
    )
    .context("Failed to create image buffer.")?;

    let mut frame_rgb = AVFrameWithImage::new(image_buffer);

    let mut sws_context = SwsContext::get_context(
        decode_context.width,
        decode_context.height,
        decode_context.pix_fmt,
        decode_context.width,
        decode_context.height,
        ffi::AVPixelFormat_AV_PIX_FMT_RGB24,
        ffi::SWS_BILINEAR,
    )
    .context("Failed to create a swscale context.")?;

    let mut i = 0;
    while let Some(packet) = input_format_context.read_packet().unwrap() {
        if packet.stream_index != video_stream_index as i32 {
            continue;
        }
        decode_context.send_packet(Some(&packet))?;
        loop {
            let frame = match decode_context.receive_frame() {
                Ok(frame) => frame,
                Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => {
                    break
                }
                Err(e) => return Err(e.into()),
            };

            sws_context.scale_frame(&frame, 0, decode_context.height, &mut frame_rgb)?;
            if i >= 5 {
                break;
            }
            i += 1;
            pgm_save(&frame_rgb, &format!("{}/frame{}.ppm", out_dir, i))?;
        }
    }
    Ok(())
}

#[test]
fn tutorial01_test0() {
    _main(
        cstr!("tests/assets/vids/centaur.mpg"),
        "tests/output/tutorial01/centaur",
    )
    .unwrap();
}

#[test]
fn tutorial01_test1() {
    _main(
        cstr!("tests/assets/vids/bear.mp4"),
        "tests/output/tutorial01/bear",
    )
    .unwrap();
}

#[test]
fn tutorial01_test2() {
    _main(
        cstr!("tests/assets/vids/mov_sample.mov"),
        "tests/output/tutorial01/mov_sample",
    )
    .unwrap();
}

/*
#[test]
fn tutorial01_test3() {
    _main(
        cstr!("tests/assets/vids/vp8.mp4"),
        "tests/output/tutorial01/vp8",
    )
    .unwrap();
}
*/
