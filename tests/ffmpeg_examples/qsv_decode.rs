//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/qsv_decode.c
use anyhow::{anyhow, Context, Result};
use cstr::cstr;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext, AVPacket},
    avformat::AVFormatContextInput,
    avutil::{get_media_type_string, AVDictionary, AVHWDeviceContext},
    error::RsmpegError,
    ffi::{
        self, AVCodecID_AV_CODEC_ID_H264, AVDiscard_AVDISCARD_ALL,
        AVHWDeviceType_AV_HWDEVICE_TYPE_QSV,
    },
};
use std::ffi::{CStr, CString};

fn qsv_decode(input: &CStr, output: &CStr) -> Result<()> {
    // open the input file
    let mut input_ctx =
        AVFormatContextInput::open(input, None, &mut None).context("Cannot open input file.")?;

    let mut video_st = None;
    // find the first H.264 video stream
    for (i, st) in input_ctx.streams_mut().into_iter().enumerate() {
        if st.codecpar().codec_id == AVCodecID_AV_CODEC_ID_H264 && video_st.is_none() {
            video_st = Some(i);
        } else {
            st.set_discard(AVDiscard_AVDISCARD_ALL);
        }
    }
    let video_st = video_st.context("No H.264 video stream in the input file")?;

    // open the hardware device
    let device_context = AVHWDeviceContext::create(
        AVHWDeviceType_AV_HWDEVICE_TYPE_QSV,
        Some(cstr!("auto")),
        None,
        0,
    )
    .context("Cannot open the hardware device")?;

    let decoder = AVCodec::find_decoder_by_name(cstr!("h264_qsv"))
        .context("The QSV decoder is not present in libavcodec")?;

    let decoder_ctx = AVCodecContext::new(&decoder);
    dbg!(decoder_ctx.codec_id);
    Ok(())
}

#[test]
fn extract_mvs_test() {
    qsv_decode(
        cstr!("tests/assets/vids/bear.mp4"),
        cstr!("tests/output/qsv_decode/bear.mp4"),
    )
    .unwrap();
}
