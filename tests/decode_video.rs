use anyhow::Result;
use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext, AVCodecParserContext, AVPacket},
    avutil::AVFrame,
    error::RsmpegError,
    ffi,
};
use std::{fs, io::prelude::*, slice};

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
    Ok(())
}

/// Push packet to `decode_context`, then save the output frames(fetched from the
/// `decode_context`) as pgm files.
fn decode(
    decode_context: &mut AVCodecContext,
    packet: Option<&AVPacket>,
    out_dir: &str,
) -> Result<()> {
    decode_context.send_packet(packet)?;
    loop {
        let frame = match decode_context.receive_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => break,
            Err(e) => return Err(e.into()),
        };
        pgm_save(
            &frame,
            &format!("{}/{}.pgm", out_dir, decode_context.frame_number),
        )?;
    }
    Ok(())
}

/// This function extracts frames from a MPEG1 video, then save them to `out_dir` as pgm.
fn decode_video(video_path: &str, out_dir: &str) {
    let decoder = AVCodec::find_decoder(ffi::AVCodecID_AV_CODEC_ID_MPEG1VIDEO).unwrap();
    let mut decode_context = AVCodecContext::new(&decoder);
    decode_context.open(None).unwrap();

    let video_data = fs::read(video_path).unwrap();
    // Create output dir
    fs::create_dir_all(out_dir).unwrap();

    let mut parsed_offset = 0;
    let mut parser_context = AVCodecParserContext::find(decoder.id).unwrap();
    let mut packet = AVPacket::new();

    while parsed_offset < video_data.len() {
        let (get_packet, offset) = parser_context
            .parse_packet(
                &mut decode_context,
                &mut packet,
                &video_data[parsed_offset..],
            )
            .unwrap();
        if get_packet {
            decode(&mut decode_context, Some(&packet), out_dir).unwrap();
        }
        parsed_offset += offset;
    }

    // Flush decoder
    decode(&mut decode_context, None, out_dir).unwrap();
}

#[test]
fn decode_video_test() {
    decode_video("tests/assets/vids/centaur.mpg", "tests/output/decode_video");
}
