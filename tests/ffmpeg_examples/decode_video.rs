//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/decode_video.c
use anyhow::{anyhow, Context, Result};
use camino::Utf8Path as Path;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext, AVCodecParserContext, AVPacket},
    avutil::AVFrame,
    error::RsmpegError,
    ffi,
};
use rusty_ffmpeg::ffi::AV_INPUT_BUFFER_PADDING_SIZE;
use std::{
    fs::{self, File},
    io::prelude::*,
    slice,
};

/// Save a `AVFrame` as pgm file.
fn pgm_save(frame: &AVFrame, filename: &str) -> Result<()> {
    // Here we only capture the first layer of frame.
    let data = frame.data[0];
    let linesize = frame.linesize[0] as usize;

    let width = frame.width as usize;
    let height = frame.height as usize;

    let buffer = unsafe { slice::from_raw_parts(data, linesize * height) };

    // Create pgm file
    let mut pgm_file = fs::File::create(filename)?;

    // Write pgm header
    pgm_file.write_all(&format!("P5\n{} {}\n{}\n", width, height, 255).into_bytes())?;

    // Write pgm data
    for i in 0..height {
        pgm_file.write_all(&buffer[i * linesize..i * linesize + width])?;
    }

    pgm_file.flush()?;

    Ok(())
}

/// Push packet to `decode_context`, then save the output frames(fetched from the
/// `decode_context`) as pgm files.
fn decode(
    decode_context: &mut AVCodecContext,
    packet: Option<&AVPacket>,
    out_dir: &str,
    out_filename: &str,
) -> Result<()> {
    decode_context.send_packet(packet)?;
    loop {
        let frame = match decode_context.receive_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => break,
            Err(e) => Err(e).context("Error during decoding")?,
        };
        println!("saving frame {}", decode_context.frame_num);
        pgm_save(
            &frame,
            &format!(
                "{}/{}-{}.pgm",
                out_dir, out_filename, decode_context.frame_num
            ),
        )?;
    }
    Ok(())
}

/// This function extracts frames from a MPEG1 video, then save them to `out_dir` as pgm.
fn decode_video(video_path: &str, out_dir: &str) -> Result<()> {
    const INBUF_SIZE: usize = 4096;
    let video_path = Path::new(video_path);
    let out_filename = video_path.file_stem().unwrap();
    fs::create_dir_all(out_dir).unwrap();

    // set end of buffer to 0 (this ensures that no overreading happens for damaged MPEG streams)
    let mut inbuf = vec![0u8; INBUF_SIZE + AV_INPUT_BUFFER_PADDING_SIZE as usize];

    let decoder = AVCodec::find_decoder(ffi::AV_CODEC_ID_MPEG1VIDEO).context("Codec not found")?;
    let mut decode_context = AVCodecContext::new(&decoder);
    decode_context.open(None).context("Could not open codec")?;

    let mut video_file =
        File::open(video_path).with_context(|| anyhow!("Could not open {}", video_path))?;

    let mut parser_context = AVCodecParserContext::init(decoder.id).context("Parser not found")?;
    let mut packet = AVPacket::new();

    loop {
        let len = video_file
            .read(&mut inbuf[..INBUF_SIZE])
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
                decode(&mut decode_context, Some(&packet), out_dir, out_filename)?;
            }
        }
    }

    // Flush parser
    let (get_packet, _) = parser_context
        .parse_packet(&mut decode_context, &mut packet, &[])
        .context("Error while parsing")?;
    if get_packet {
        decode(&mut decode_context, Some(&packet), out_dir, out_filename)?;
    }

    // Flush decoder
    decode(&mut decode_context, None, out_dir, out_filename)?;

    Ok(())
}

#[test]
fn decode_video_test() {
    decode_video("tests/assets/vids/centaur.mpg", "tests/output/decode_video").unwrap();
}
