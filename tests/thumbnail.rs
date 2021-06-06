use rsmpeg::avcodec::*;
use rsmpeg::avformat::*;
use rsmpeg::avutil::*;
use rsmpeg::error::RsmpegError;
use rsmpeg::ffi;
use rsmpeg::swscale::*;

use anyhow::{Context, Result};
use cstr::cstr;
use std::ffi::CStr;
use std::fs::{self, File};
use std::io::prelude::*;
use std::ops::Deref;
use std::slice;

fn thumbnail(
    input_video_path: &CStr,
    output_image_path: &CStr,
    width: Option<i32>,
    height: Option<i32>,
) -> Result<()> {
    let mut input_format_context = AVFormatContextInput::open(&input_video_path)?;

    let (video_stream_index, mut decode_context) = {
        let (stream_index, decoder) = input_format_context
            .find_best_stream(ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)?
            .context("Failed to find the best stream")?;

        let stream = input_format_context.streams().get(stream_index).unwrap();

        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context.apply_codecpar(stream.codecpar())?;
        decode_context.open(None)?;

        (stream_index, decode_context)
    };

    let cover_frame = loop {
        let cover_packet = {
            let mut cover_packet = None;
            while let Some(packet) = input_format_context.read_packet()? {
                // Get first video packet.
                if packet.stream_index == video_stream_index as i32 {
                    cover_packet = Some(packet);
                    break;
                }
            }
            cover_packet.context("Cannnot find video cover packet")?
        };

        decode_context.send_packet(Some(&cover_packet))?;
        // repeatedly send packet until a frame can be extracted
        match decode_context.receive_frame() {
            Ok(x) => break x,
            Err(RsmpegError::DecoderDrainError) => {}
            Err(e) => return Err(e.into()),
        }
    };

    println!("Cover frame info: {:#?}", cover_frame);

    let mut encode_context = {
        let encoder =
            AVCodec::find_encoder(ffi::AVCodecID_AV_CODEC_ID_MJPEG).context("Encoder not found")?;
        let mut encode_context = AVCodecContext::new(&encoder);

        encode_context.set_bit_rate(decode_context.bit_rate);
        encode_context.set_width(width.unwrap_or(decode_context.width));
        encode_context.set_height(height.unwrap_or(decode_context.height));
        encode_context.set_time_base(av_inv_q(decode_context.framerate));
        encode_context.set_pix_fmt(if let Some(pix_fmts) = encoder.pix_fmts() {
            pix_fmts[0]
        } else {
            decode_context.pix_fmt
        });
        encode_context.open(None)?;

        encode_context
    };

    let scaled_cover_packet = {
        let mut sws_context = SwsContext::get_context(
            decode_context.width,
            decode_context.height,
            decode_context.pix_fmt,
            encode_context.width,
            encode_context.height,
            encode_context.pix_fmt,
            ffi::SWS_FAST_BILINEAR | ffi::SWS_PRINT_INFO,
        )
        .context("Invalid swscontext parameter.")?;

        let mut image_buffer = AVImage::new(
            encode_context.pix_fmt,
            encode_context.width,
            encode_context.height,
            1,
        )
        .context("Image buffer parameter invalid.")?;

        let mut scaled_cover_frame = AVFrameWithImageBuffer::new(
            &mut image_buffer,
            encode_context.width,
            encode_context.height,
            encode_context.pix_fmt,
        );

        sws_context.scale_frame(
            &cover_frame,
            0,
            decode_context.height,
            &mut scaled_cover_frame,
        )?;

        println!("{:#?}", scaled_cover_frame.deref());

        encode_context.send_frame(Some(&scaled_cover_frame))?;
        encode_context.receive_packet()?
    };

    let mut file = File::create(output_image_path.to_str().unwrap()).unwrap();
    let data = unsafe {
        slice::from_raw_parts(scaled_cover_packet.data, scaled_cover_packet.size as usize)
    };
    file.write_all(data)?;

    Ok(())
}

#[test]
fn thumbnail_test() {
    fs::create_dir_all("tests/output/thumbnail").unwrap();

    thumbnail(
        cstr!("tests/assets/vids/bear.mp4"),
        cstr!("tests/output/thumbnail/bear.jpg"),
        Some(192),
        Some(108),
    )
    .unwrap();
}
