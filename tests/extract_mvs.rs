#![feature(once_cell)]
use rsmpeg::{
    avcodec::{AVCodecContext, AVPacket},
    avformat::AVFormatContextInput,
    avutil::AVDictionary,
    error::RsmpegError,
    ffi,
};
use std::{ffi::CString, lazy::SyncLazy, sync::Mutex};

macro_rules! cstr {
    ($s: literal) => {
        &CString::new($s).unwrap()
    };
    ($s: expr) => {
        &CString::new($s)?
    };
}

static VIDEO_FRAME_COUNT: SyncLazy<Mutex<usize>> = SyncLazy::new(|| Mutex::new(0));

fn decode_packet(
    decode_context: &mut AVCodecContext,
    packet: Option<&AVPacket>,
    motion_vectors_serialized: &mut Vec<String>,
) {
    decode_context
        .send_packet(packet)
        .expect("Error while sending a packet to the decoder");
    let mut count = VIDEO_FRAME_COUNT.lock().unwrap();
    loop {
        let frame = match decode_context.receive_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => break,
            Err(e) => panic!("{}", e),
        };

        *count += 1;
        if let Some(motion_vectors) = frame.get_motion_vectors() {
            for motion_vector in motion_vectors {
                // framenum,source,blockw,blockh,srcx,srcy,dstx,dsty,flags
                motion_vectors_serialized.push(format!(
                    "{},{:2},{:2},{:2},{:4},{:4},{:4},{:4},{:#x}",
                    *count,
                    motion_vector.source,
                    motion_vector.w,
                    motion_vector.h,
                    motion_vector.src_x,
                    motion_vector.src_y,
                    motion_vector.dst_x,
                    motion_vector.dst_y,
                    motion_vector.flags,
                ));
            }
        }
    }
}

fn open_codec_context(
    input_format_context: &mut AVFormatContextInput,
    media_type: ffi::AVMediaType,
) -> (usize, AVCodecContext) {
    let (stream_index, decoder) = input_format_context
        .find_best_stream(media_type)
        .unwrap()
        .expect("Cannot find best stream in this file.");
    let stream = input_format_context
        .streams()
        .get(stream_index as usize)
        .unwrap();

    let mut decode_context = AVCodecContext::new(&decoder);
    decode_context
        .set_codecpar(stream.codecpar())
        .expect("Failed to set codec parameters to codec context.");

    let opts = AVDictionary::new(cstr!("flags2"), cstr!("+export_mvs"), 0);
    decode_context
        .open(Some(opts))
        .expect("failed to open decode codec");
    (stream_index, decode_context)
}

fn extract_mvs(file: &str) -> Vec<String> {
    let src_filename = &CString::new(file).unwrap();
    let mut input_format_context = AVFormatContextInput::open(src_filename).unwrap();

    let (stream_index, mut decode_context) = open_codec_context(
        &mut input_format_context,
        ffi::AVMediaType_AVMEDIA_TYPE_VIDEO,
    );

    input_format_context
        .dump(0, src_filename)
        .expect("Input format context dump failed.");

    input_format_context
        .streams()
        .get(stream_index)
        .expect("Could not find video stream in the input, aborting");

    let mut motion_vectors_serialized = vec![];
    while let Some(packet) = input_format_context.read_packet().unwrap() {
        if packet.stream_index == stream_index as i32 {
            decode_packet(
                &mut decode_context,
                Some(&packet),
                &mut motion_vectors_serialized,
            );
        }
    }

    decode_packet(&mut decode_context, None, &mut motion_vectors_serialized);

    motion_vectors_serialized
}

#[test]
fn extract_mvs_test() {
    let mvs = extract_mvs("tests/utils/extract_mvs/bear.mp4");
    assert_eq!(10783, mvs.len());
    assert_eq!("2, 1,16,16, 264,  56, 264,  56,0x0", mvs[114]);
    assert_eq!("3,-1, 8, 8, 220,  52, 220,  52,0x0", mvs[514]);
    assert_eq!("7,-1,16,16,  87,   8,  88,   8,0x0", mvs[1919]);
    assert_eq!("4, 1,16,16, 232,  24, 232,  24,0x0", mvs[810]);
}
