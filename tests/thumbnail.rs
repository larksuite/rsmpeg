use rsmpeg::avcodec::*;
use rsmpeg::avformat::*;
use rsmpeg::avutil::*;
use rsmpeg::error::RsmpegError;
use rsmpeg::ffi;
use rsmpeg::swscale::*;

use std::ffi::{CStr, CString, OsString};
use std::path::PathBuf;
use std::{ops::Deref, path::Path};

fn thumbnail(
    input_video_path: &CStr,
    output_image_path: &CStr,
    width: Option<i32>,
    height: Option<i32>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut input_format_context = AVFormatContextInput::open(&input_video_path)?;

    let (mut decode_codec_context, video_stream_index) = {
        let mut video_info = None;

        for stream in input_format_context.streams() {
            // TODO: Or directly from stream.codecpar().codec_type?
            let decoder =
                AVCodec::find_decoder(stream.codecpar().codec_id).ok_or("Decoder not found.")?;
            if decoder.type_ == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
                let mut decode_codec_context = AVCodecContext::new(&decoder);
                decode_codec_context.set_codecpar(stream.codecpar())?;
                decode_codec_context.open(None)?;
                video_info = Some((decode_codec_context, stream.index));
                break;
            }
        }

        video_info.ok_or("cannot find video stream for specific file")?
    };

    let cover_frame = {
        let cover_frame;
        loop {
            let cover_packet = {
                let mut cover_packet = None;
                while let Some(packet) = input_format_context.read_packet()? {
                    // Get first video packet.
                    if packet.stream_index == video_stream_index {
                        cover_packet = Some(packet);
                        break;
                    }
                }
                cover_packet.ok_or("Cannnot find video cover packet")?
            };

            decode_codec_context.send_packet(Some(&cover_packet))?;
            match decode_codec_context.receive_frame() {
                Ok(x) => {
                    // repeatedly send packet until a frame can be extracted
                    cover_frame = x;
                    break;
                }
                Err(RsmpegError::DecoderDrainError) => {}
                Err(e) => return Err(e.into()),
            }
        }
        cover_frame
    };
    println!("{:#?}", cover_frame);

    let mut encode_codec_context = {
        let encoder =
            AVCodec::find_encoder(ffi::AVCodecID_AV_CODEC_ID_MJPEG).ok_or("Encoder not found")?;
        let mut encode_codec_context = AVCodecContext::new(&encoder);

        encode_codec_context.set_bit_rate(decode_codec_context.bit_rate);
        encode_codec_context.set_width(width.unwrap_or(decode_codec_context.width));
        encode_codec_context.set_height(height.unwrap_or(decode_codec_context.height));
        encode_codec_context.set_time_base(av_inv_q(decode_codec_context.framerate));
        encode_codec_context.set_pix_fmt(if let Some(pix_fmts) = encoder.pix_fmts() {
            pix_fmts[0]
        } else {
            decode_codec_context.pix_fmt
        });
        encode_codec_context.open(None)?;

        encode_codec_context
    };

    let scaled_cover_packet = {
        let mut sws_context = SwsContext::get_context(
            decode_codec_context.width,
            decode_codec_context.height,
            decode_codec_context.pix_fmt,
            encode_codec_context.width,
            encode_codec_context.height,
            encode_codec_context.pix_fmt,
            ffi::SWS_FAST_BILINEAR | ffi::SWS_PRINT_INFO,
        )
        .ok_or("cannot create SwsContext!")?;

        let mut image_buffer = AVImage::new(
            encode_codec_context.pix_fmt,
            encode_codec_context.width,
            encode_codec_context.height,
            1,
        )
        .ok_or("cannot create AVImage buffer!")?;

        let mut scaled_cover_frame = AVFrameWithImageBuffer::new(
            &mut image_buffer,
            encode_codec_context.width,
            encode_codec_context.height,
            encode_codec_context.pix_fmt,
        );

        sws_context.scale_frame(
            &cover_frame,
            0,
            decode_codec_context.height,
            &mut scaled_cover_frame,
        )?;

        println!("{:#?}", scaled_cover_frame.deref());

        encode_codec_context.send_frame(Some(&scaled_cover_frame))?;
        encode_codec_context.receive_packet()?
    };

    use std::io::prelude::*;
    let mut file = std::fs::File::create(output_image_path.to_str().unwrap()).unwrap();
    let data = unsafe {
        std::slice::from_raw_parts(scaled_cover_packet.data, scaled_cover_packet.size as usize)
    };
    file.write_all(data)?;

    Ok(())
}

fn thumbnail_facade(
    video_path: &Path,
    width: Option<i32>,
    height: Option<i32>,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    let input_video_path = CString::new(video_path.to_str().ok_or("non utf8 path!")?)?;
    let output_image_path = {
        let output_image_path = {
            let mut image_dir = video_path
                .parent()
                .ok_or("image_path no parent!")?
                .to_owned();
            let resized_file_name = {
                let mut s = OsString::from("resized_");
                s.push(video_path.file_name().ok_or("image_path no file_name!")?);
                s.push(".jpg");
                s
            };
            image_dir.push(resized_file_name);
            image_dir
        };
        CString::new(output_image_path.to_str().ok_or("non utf8 path!")?)?
    };
    println!("From {:?}, to {:?}", input_video_path, output_image_path);
    thumbnail(&input_video_path, &output_image_path, width, height)?;
    Ok(())
}

#[test]
fn thumbnail_test() {
    thumbnail_facade(
        &PathBuf::from("tests/utils/thumbnail/bear.mp4"),
        Some(192),
        Some(108),
    )
    .unwrap();
}
