//! A test that split a video(H.264, AAC) file to a AAC file and a H.264
//! file(Annex B). Showing the usage of `AVBitStream` related APIs.
use anyhow::{bail, Context, Result};
use cstr::cstr;
use rsmpeg::{
    avcodec::{AVBSFContextUninit, AVBitStreamFilter},
    avformat::{AVFormatContextInput, AVFormatContextOutput},
    error::RsmpegError,
    ffi::{self},
};
use std::{
    ffi::CStr,
    fs::{self, File},
    io::Write,
};

fn av_spliter(file_path: &CStr, out_video: &str, out_audio: &CStr) -> Result<()> {
    let mut out_video = File::create(out_video)?;

    let mut input_format_context = AVFormatContextInput::open(file_path)?;
    input_format_context.dump(0, file_path)?;

    let video_index = input_format_context
        .streams()
        .into_iter()
        .position(|x| x.codecpar().codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)
        .context("Cannot find video stream!")?;
    let audio_index = input_format_context
        .streams()
        .into_iter()
        .position(|x| x.codecpar().codec_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO)
        .context("Cannot find audio stream!")?;

    let bsf = AVBitStreamFilter::find_by_name(cstr!("h264_mp4toannexb"))
        .context("Failed to find bit stream filter")?;

    let mut bsf_context = {
        let mut bsf_context = AVBSFContextUninit::new(&bsf);
        let video_stream = input_format_context.streams().get(video_index).unwrap();
        bsf_context.set_par_in(&video_stream.codecpar());
        bsf_context.set_time_base_in(video_stream.time_base);
        bsf_context.init()?
    };

    let mut out_audio_format_context = AVFormatContextOutput::create(out_audio, None)?;
    {
        let mut new_audio_stream = out_audio_format_context.new_stream();
        let audio_stream = input_format_context.streams().get(audio_index).unwrap();
        new_audio_stream.set_codecpar(audio_stream.codecpar().clone());
        new_audio_stream.set_time_base(audio_stream.time_base);
    }
    out_audio_format_context.write_header(&mut None)?;

    while let Some(mut packet) = input_format_context.read_packet()? {
        let packet_stream_index = packet.stream_index as usize;
        if packet_stream_index == video_index {
            bsf_context.send_packet(Some(&mut packet))?;
            loop {
                match bsf_context.receive_packet(&mut packet) {
                    Ok(()) => {
                        let data = unsafe {
                            std::slice::from_raw_parts(packet.data, packet.size as usize)
                        };
                        out_video.write_all(data)?;
                    }
                    Err(RsmpegError::BitstreamDrainError)
                    | Err(RsmpegError::BitstreamFlushedError) => break,
                    Err(e) => bail!(e),
                }
            }
        } else if packet_stream_index == audio_index {
            packet.set_stream_index(0);
            out_audio_format_context.write_frame(&mut packet)?;
        }
    }

    out_audio_format_context.write_trailer()?;

    out_video.flush()?;

    Ok(())
}

#[test]
fn test_av_spliter0() {
    fs::create_dir_all("tests/output/av_spliter").unwrap();
    av_spliter(
        cstr!("tests/assets/vids/bunny.flv"),
        "tests/output/av_spliter/out_video_bunny.h264",
        cstr!("tests/output/av_spliter/out_audio_bunny.aac"),
    )
    .unwrap();
}

#[test]
fn test_av_spliter1() {
    fs::create_dir_all("tests/output/av_spliter").unwrap();
    av_spliter(
        cstr!("tests/assets/vids/bear.mp4"),
        "tests/output/av_spliter/out_video_bear.h264",
        cstr!("tests/output/av_spliter/out_audio_bear.aac"),
    )
    .unwrap();
}
