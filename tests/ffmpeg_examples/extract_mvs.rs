//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/extract_mvs.c
use anyhow::{anyhow, Context, Result};
use cstr::cstr;
use rsmpeg::{
    avcodec::{AVCodecContext, AVPacket},
    avformat::AVFormatContextInput,
    avutil::{get_media_type_string, AVDictionary},
    error::RsmpegError,
    ffi,
};
use std::ffi::{CStr, CString};

fn decode_packet(
    decode_context: &mut AVCodecContext,
    packet: Option<&AVPacket>,
    video_frame_count: &mut usize,
) -> Result<()> {
    decode_context
        .send_packet(packet)
        .context("Error while sending a packet to the decoder")?;

    loop {
        let frame = match decode_context.receive_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => break,
            Err(e) => return Err(e.into()),
        };

        *video_frame_count += 1;

        if let Some(side_data) = frame.get_side_data(ffi::AV_FRAME_DATA_MOTION_VECTORS) {
            let raw_motion_vectors = unsafe { side_data.as_motion_vectors() };
            for &motion_vector in raw_motion_vectors {
                println!(
                    "{},{:2},{:2},{:2},{:4},{:4},{:4},{:4},{:#x},{:4},{:4},{:4}",
                    video_frame_count,
                    motion_vector.source,
                    motion_vector.w,
                    motion_vector.h,
                    motion_vector.src_x,
                    motion_vector.src_y,
                    motion_vector.dst_x,
                    motion_vector.dst_y,
                    motion_vector.flags,
                    motion_vector.motion_x,
                    motion_vector.motion_y,
                    motion_vector.motion_scale,
                );
            }
        };
    }
    Ok(())
}

/// Extract motion vectors from a video.
fn extract_mvs(video_path: &CStr) -> Result<()> {
    let mut input_format_context = AVFormatContextInput::open(video_path, None, &mut None)?;
    let media_type = ffi::AVMEDIA_TYPE_VIDEO;

    let (stream_index, mut decode_context) = {
        let (stream_index, decoder) = input_format_context
            .find_best_stream(media_type)?
            .with_context(|| {
                anyhow!(
                    "Could not find {} stream in input file '{}'",
                    get_media_type_string(media_type).unwrap().to_string_lossy(),
                    video_path.to_string_lossy()
                )
            })?;

        let stream = &input_format_context.streams()[stream_index];

        let mut decode_context = AVCodecContext::new(&decoder);

        decode_context
            .apply_codecpar(&stream.codecpar())
            .context("Failed to copy codec parameters to codec context")?;

        let opts = AVDictionary::new(cstr!("flags2"), cstr!("+export_mvs"), 0);

        decode_context.open(Some(opts)).with_context(|| {
            anyhow!(
                "Failed to open {} codec",
                get_media_type_string(media_type).unwrap().to_string_lossy()
            )
        })?;

        (stream_index, decode_context)
    };

    input_format_context
        .dump(0, video_path)
        .context("Input format context dump failed.")?;

    println!(
        "framenum,source,blockw,blockh,srcx,srcy,dstx,dsty,flags,motion_x,motion_y,motion_scale"
    );

    let mut video_frame_count = 0;

    while let Some(packet) = input_format_context.read_packet().unwrap() {
        if packet.stream_index == stream_index as i32 {
            decode_packet(&mut decode_context, Some(&packet), &mut video_frame_count)?;
        }
    }

    decode_packet(&mut decode_context, None, &mut video_frame_count)?;

    Ok(())
}

#[test]
fn extract_mvs_test() {
    let video_path = &CString::new("tests/assets/vids/bear.mp4").unwrap();
    extract_mvs(video_path).unwrap();
}
