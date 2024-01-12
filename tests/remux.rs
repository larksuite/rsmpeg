//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/remux.c
use anyhow::{Context, Result};
use cstr::cstr;
use rsmpeg::{
    avcodec::AVPacket,
    avformat::{AVFormatContextInput, AVFormatContextOutput},
    avutil::{ts2str, ts2timestr},
    ffi::AVRational,
};
use std::ffi::CStr;

fn log_packet(time_base: AVRational, pkt: &AVPacket, tag: &str) {
    println!(
        "{}: pts:{} pts_time:{} dts:{} dts_time:{} duration:{} duration_time:{} stream_index:{}",
        tag,
        ts2str(pkt.pts),
        ts2timestr(pkt.pts, time_base),
        ts2str(pkt.dts),
        ts2timestr(pkt.dts, time_base),
        ts2str(pkt.duration),
        ts2timestr(pkt.duration, time_base),
        pkt.stream_index
    );
}

fn remux(input_path: &CStr, output_path: &CStr) -> Result<()> {
    let mut input_format_context = AVFormatContextInput::open(input_path, None, &mut None)
        .context("Create input format context failed.")?;
    input_format_context
        .dump(0, input_path)
        .context("Dump input format context failed.")?;
    let mut output_format_context = AVFormatContextOutput::create(output_path, None)
        .context("Create output format context failed.")?;
    let stream_mapping: Vec<_> = {
        let mut stream_index = 0usize;
        input_format_context
            .streams()
            .into_iter()
            .map(|stream| {
                let codec_type = stream.codecpar().codec_type();
                if !codec_type.is_video() && !codec_type.is_audio() && !codec_type.is_subtitle() {
                    None
                } else {
                    output_format_context
                        .new_stream()
                        .set_codecpar(stream.codecpar().clone());
                    stream_index += 1;
                    Some(stream_index - 1)
                }
            })
            .collect()
    };
    output_format_context
        .dump(0, output_path)
        .context("Dump output format context failed.")?;

    output_format_context
        .write_header(&mut None)
        .context("Writer header failed.")?;

    while let Some(mut packet) = input_format_context
        .read_packet()
        .context("Read packet failed.")?
    {
        let input_stream_index = packet.stream_index as usize;
        let Some(output_stream_index) = stream_mapping[input_stream_index] else {
            continue;
        };
        {
            let input_stream = &input_format_context.streams()[input_stream_index];
            let output_stream = &output_format_context.streams()[output_stream_index];
            log_packet(input_stream.time_base, &packet, "in");
            packet.rescale_ts(input_stream.time_base, output_stream.time_base);
            packet.set_stream_index(output_stream_index as i32);
            packet.set_pos(-1);
            log_packet(output_stream.time_base, &packet, "out");
        }
        output_format_context
            .interleaved_write_frame(&mut packet)
            .context("Interleaved write frame failed.")?;
    }
    output_format_context
        .write_trailer()
        .context("Write trailer failed.")
}

/// Remux MP4 to MOV, with h.264 codec.
#[test]
fn remux_test0() {
    std::fs::create_dir_all("tests/output/remux/").unwrap();
    remux(
        cstr!("tests/assets/vids/big_buck_bunny.mp4"),
        cstr!("tests/output/remux/big_buck_bunny.mov"),
    )
    .unwrap();
}
