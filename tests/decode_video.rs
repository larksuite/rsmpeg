use rsmpeg::{
    avcodec::{AVCodec, AVCodecContext, AVCodecParserContext, AVPacket},
    avutil::AVFrame,
    error::RsmpegError,
    ffi,
};
use std::{fs, io::prelude::*, slice};

fn pgm_save(frame: &AVFrame, filename: &str) {
    // here we only capture
    let data = frame.data[0];
    let linesize = frame.linesize[0] as usize;

    let width = frame.width as usize;
    let height = frame.height as usize;

    let mut pgm_file = fs::File::create(filename).unwrap();
    pgm_file
        .write_all(&format!("P5\n{} {}\n{}\n", width, height, 255).into_bytes())
        .unwrap();
    for i in 0..height {
        let buffer = unsafe { slice::from_raw_parts(data.add(i * linesize), width) };
        pgm_file.write_all(buffer).unwrap();
    }
}

fn decode(decode_context: &mut AVCodecContext, packet: Option<&AVPacket>, out_filename: &str) {
    decode_context
        .send_packet(packet)
        .expect("Send packet failed.");
    loop {
        let frame = match decode_context.receive_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => break,
            Err(e) => panic!("{}", e),
        };
        let out_filename = &format!("{}-{}.pgm", out_filename, decode_context.frame_number);
        pgm_save(&frame, out_filename);
    }
}

fn decode_video(filename: &str, out_filename: &str) {
    let decoder = AVCodec::find_decoder(ffi::AVCodecID_AV_CODEC_ID_MPEG1VIDEO).unwrap();
    let mut decode_context = AVCodecContext::new(&decoder);
    decode_context.open(None).unwrap();

    let file_data = fs::read(filename).unwrap();
    let file_len = file_data.len();
    let mut parsed_offset = 0;

    let mut parser_context = AVCodecParserContext::find(decoder.id).unwrap();
    let mut packet = AVPacket::new();

    while parsed_offset < file_len {
        let (get_packet, offset) = parser_context
            .parse_packet(
                &mut decode_context,
                &mut packet,
                &file_data[parsed_offset..],
            )
            .unwrap();
        parsed_offset += offset;
        if get_packet {
            decode(&mut decode_context, Some(&packet), out_filename);
        }
    }
}

#[test]
fn decode_video_test() {
    decode_video(
        "tests/utils/decode_video/centaur.mpg",
        "tests/utils/decode_video/decoded/centaur_decoded",
    );
}
