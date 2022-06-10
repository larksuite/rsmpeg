use anyhow::{Context, Result};
use cstr::cstr;
use rsmpeg::{
    avcodec::{AVCodecContext, AVPacket},
    avformat::AVFormatContextInput,
    avutil::{AVDictionary, AVMotionVector},
    error::RsmpegError,
    ffi,
};
use std::ffi::{CStr, CString};

struct MotionVector {
    motion_vector: AVMotionVector,
    frame_index: usize,
}

fn decode_packet(
    decode_context: &mut AVCodecContext,
    packet: Option<&AVPacket>,
    frame_index: &mut usize,
    motion_vectors: &mut Vec<MotionVector>,
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

        *frame_index += 1;

        if let Some(raw_motion_vectors) = frame.get_motion_vectors() {
            for motion_vector in raw_motion_vectors {
                // framenum,source,blockw,blockh,srcx,srcy,dstx,dsty,flags
                motion_vectors.push(MotionVector {
                    frame_index: *frame_index,
                    motion_vector: motion_vector.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Extract motion vectors from a video.
fn extract_mvs(video_path: &CStr) -> Result<Vec<MotionVector>> {
    let mut input_format_context = AVFormatContextInput::open(video_path)?;

    let (stream_index, mut decode_context) = {
        let (stream_index, decoder) = input_format_context
            .find_best_stream(ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)?
            .context("Cannot find best stream in this file.")?;

        let stream = input_format_context.streams().get(stream_index).unwrap();

        let mut decode_context = AVCodecContext::new(&decoder);

        decode_context.apply_codecpar(&stream.codecpar())?;

        let key = cstr!("flags2");
        let value = cstr!("+export_mvs");
        let opts = AVDictionary::new(key, value, 0);

        decode_context
            .open(Some(opts))
            .context("failed to open decode codec")?;

        (stream_index, decode_context)
    };

    input_format_context
        .dump(0, video_path)
        .context("Input format context dump failed.")?;

    let mut video_frame_count = 0;
    let mut motion_vectors = vec![];

    while let Some(packet) = input_format_context.read_packet().unwrap() {
        if packet.stream_index == stream_index as i32 {
            decode_packet(
                &mut decode_context,
                Some(&packet),
                &mut video_frame_count,
                &mut motion_vectors,
            )?;
        }
    }

    decode_packet(
        &mut decode_context,
        None,
        &mut video_frame_count,
        &mut motion_vectors,
    )?;

    Ok(motion_vectors)
}

#[test]
fn extract_mvs_test() {
    fn to_string(motion_vector: &MotionVector) -> String {
        format!(
            "{},{:2},{:2},{:2},{:4},{:4},{:4},{:4},{:#x}",
            motion_vector.frame_index,
            motion_vector.motion_vector.source,
            motion_vector.motion_vector.w,
            motion_vector.motion_vector.h,
            motion_vector.motion_vector.src_x,
            motion_vector.motion_vector.src_y,
            motion_vector.motion_vector.dst_x,
            motion_vector.motion_vector.dst_y,
            motion_vector.motion_vector.flags,
        )
    }
    let video_path = &CString::new("tests/assets/vids/bear.mp4").unwrap();
    let mvs = extract_mvs(video_path).unwrap();
    assert_eq!(10783, mvs.len());
    assert_eq!("2, 1,16,16, 264,  56, 264,  56,0x0", to_string(&mvs[114]));
    assert_eq!("3,-1, 8, 8, 220,  52, 220,  52,0x0", to_string(&mvs[514]));
    assert_eq!("7,-1,16,16,  87,   8,  88,   8,0x0", to_string(&mvs[1919]));
    assert_eq!("4, 1,16,16, 232,  24, 232,  24,0x0", to_string(&mvs[810]));
}
