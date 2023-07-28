use anyhow::{Context, Result};
use cstr::cstr;
use rsmpeg::avformat::{AVFormatContextInput, AVFormatContextOutput};
use std::ffi::CStr;

fn remux(input_path: &CStr, output_path: &CStr) -> Result<()> {
    let mut input_format_context = AVFormatContextInput::open(input_path, None)
        .context("Create input format context failed.")?;
    input_format_context
        .dump(0, input_path)
        .context("Dump input format context failed.")?;
    let mut output_format_context = AVFormatContextOutput::create(output_path, None)
        .context("Create output format context failed.")?;
    let stream_mapping = {
        let mut stream_mapping = vec![None; input_format_context.nb_streams as usize];
        let mut stream_index = 0;
        for (i, stream) in input_format_context.streams().into_iter().enumerate() {
            let codec_type = stream.codecpar().codec_type();
            if !codec_type.is_video() && !codec_type.is_audio() && !codec_type.is_subtitle() {
                stream_mapping[i] = None;
                continue;
            }
            stream_mapping[i] = Some(stream_index);
            stream_index += 1;

            let mut new_stream = output_format_context.new_stream();
            new_stream.set_codecpar(stream.codecpar().clone());
        }
        stream_mapping
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
        let output_stream_index = match stream_mapping[input_stream_index] {
            Some(x) => x,
            None => continue,
        };
        let output_stream_index = output_stream_index as usize;
        {
            let input_stream = input_format_context
                .streams()
                .get(input_stream_index)
                .unwrap();
            let output_stream = output_format_context
                .streams()
                .get(output_stream_index)
                .unwrap();
            packet.rescale_ts(input_stream.time_base, output_stream.time_base);
            packet.set_stream_index(output_stream_index as i32);
            packet.set_pos(-1);
        }
        output_format_context
            .interleaved_write_frame(&mut packet)
            .context("Interleaved write frame failed.")?;
    }

    output_format_context
        .write_trailer()
        .context("Write trailer failed.")?;
    Ok(())
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
