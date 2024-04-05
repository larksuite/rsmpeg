//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/decode_audio.c
use anyhow::{anyhow, Context, Result};
use rsmpeg::{
    avcodec::{AVCodecContext, AVCodecParserContext, AVPacket},
    avformat::AVFormatContextInput,
    avutil::{
        get_bytes_per_sample, get_packed_sample_fmt, get_sample_fmt_name, sample_fmt_is_planar,
        AVFrame, AVSampleFormat,
    },
    error::RsmpegError,
    ffi,
};
use rusty_ffmpeg::ffi::AV_INPUT_BUFFER_PADDING_SIZE;
use std::{
    ffi::CString,
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    slice::from_raw_parts,
};

fn get_format_from_sample_fmt(sample_fmt: AVSampleFormat) -> Option<&'static str> {
    let sample_fmt_entries = [
        (ffi::AV_SAMPLE_FMT_U8, "u8"),
        (ffi::AV_SAMPLE_FMT_S16, "s16le"),
        (ffi::AV_SAMPLE_FMT_S32, "s32le"),
        (ffi::AV_SAMPLE_FMT_FLT, "f32le"),
        (ffi::AV_SAMPLE_FMT_DBL, "f64le"),
    ];
    sample_fmt_entries
        .iter()
        .find(|(fmt, _)| *fmt == sample_fmt)
        .map(|(_, fmt)| *fmt)
}

fn frame_save(frame: &AVFrame, channels: usize, data_size: usize, mut file: &File) -> Result<()> {
    let nb_samples: usize = frame.nb_samples.try_into().context("nb_samples overflow")?;
    // ATTENTION: This is only valid for planar sample formats.
    for i in 0..nb_samples {
        for channel in 0..channels {
            let data = unsafe { from_raw_parts(frame.data[channel].add(data_size * i), data_size) };
            file.write_all(data).context("Write data failed.")?;
        }
    }
    Ok(())
}

fn decode(
    decode_context: &mut AVCodecContext,
    packet: Option<&AVPacket>,
    out_file: &File,
) -> Result<()> {
    decode_context
        .send_packet(packet)
        .context("Send packet failed.")?;
    let channels = decode_context
        .ch_layout
        .nb_channels
        .try_into()
        .context("channels overflow")?;
    let sample_fmt = decode_context.sample_fmt;
    loop {
        let frame = match decode_context.receive_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => break,
            Err(e) => return Err(e).context("Receive frame failed."),
        };
        let data_size = get_bytes_per_sample(sample_fmt).context("Unknown sample fmt")?;
        frame_save(&frame, channels, data_size, out_file)?;
    }
    Ok(())
}

fn decode_audio(audio_path: &str, out_file_path: &str) -> Result<()> {
    const AUDIO_INBUF_SIZE: usize = 20480;

    let (decoder, mut decode_context) = {
        // safety, &str ensures no internal null bytes.
        let audio_path = CString::new(audio_path).unwrap();
        let mut input_format_context = AVFormatContextInput::open(&audio_path, None, &mut None)
            .context("Open audio file failed.")?;
        let (stream_index, decoder) = input_format_context
            .find_best_stream(ffi::AVMEDIA_TYPE_AUDIO)
            .context("Find best stream failed.")?
            .context("Cannot find audio stream in this file.")?;
        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context
            .apply_codecpar(&input_format_context.streams()[stream_index].codecpar())
            .context("Apply codecpar failed.")?;
        decode_context.open(None).context("Could not open codec")?;
        input_format_context.dump(stream_index, &audio_path)?;
        (decoder, decode_context)
    };

    let mut inbuf = [0u8; AUDIO_INBUF_SIZE + AV_INPUT_BUFFER_PADDING_SIZE as usize];

    let mut audio_file =
        File::open(audio_path).with_context(|| anyhow!("Could not open {}", audio_path))?;
    fs::create_dir_all(Path::new(out_file_path).parent().unwrap()).unwrap();
    let out_file = File::create(out_file_path).context("Open out file failed.")?;

    let mut parser_context = AVCodecParserContext::init(decoder.id).context("Parser not found")?;
    let mut packet = AVPacket::new();

    loop {
        let len = audio_file
            .read(&mut inbuf[..AUDIO_INBUF_SIZE])
            .context("Read input file failed.")?;
        if len == 0 {
            break;
        }
        let mut parsed_offset = 0;
        while parsed_offset < len {
            let (get_packet, offset) = parser_context
                .parse_packet(&mut decode_context, &mut packet, &inbuf[parsed_offset..len])
                .context("Error while parsing")?;
            parsed_offset += offset;
            if get_packet {
                decode(&mut decode_context, Some(&packet), &out_file)?;
            }
        }
    }

    // Flush parser
    let (get_packet, _) = parser_context
        .parse_packet(&mut decode_context, &mut packet, &[])
        .context("Error while parsing")?;
    if get_packet {
        decode(&mut decode_context, Some(&packet), &out_file)?;
    }

    // Flush decoder
    decode(&mut decode_context, None, &out_file)?;

    let mut sample_fmt = decode_context.sample_fmt;

    if sample_fmt_is_planar(sample_fmt) {
        let name = get_sample_fmt_name(sample_fmt);
        println!(
            "Warning: the sample format the decoder produced is planar \
            ({}). This example will output the first channel only.",
            name.map(|x| x.to_str().unwrap()).unwrap_or("?")
        );
        sample_fmt = get_packed_sample_fmt(sample_fmt).context("Cannot get packed sample fmt")?;
    }

    let fmt = get_format_from_sample_fmt(sample_fmt).context("Unsupported sample fmt")?;

    println!("Play the output audio file with the command:");
    println!(
        "ffplay -f {} -ac {} -ar {} {}",
        fmt, decode_context.ch_layout.nb_channels, decode_context.sample_rate, out_file_path
    );
    Ok(())
}

#[test]
fn decode_audio_test() {
    decode_audio(
        "tests/assets/audios/sample1_short.aac",
        "tests/output/decode_audio/sample1_short.pcm",
    )
    .unwrap();
}
