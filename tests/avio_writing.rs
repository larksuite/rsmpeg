/// Simplified transcoding test, select the first video stream in given video file
/// and transcode it. Store the output in memory.
use anyhow::{anyhow, bail, Context, Result};
use cstr::cstr;
use rsmpeg::{
    self,
    avcodec::{AVCodec, AVCodecContext},
    avformat::{
        AVFormatContextInput, AVFormatContextOutput, AVIOContextContainer, AVIOContextCustom,
    },
    avutil::{av_inv_q, av_mul_q, AVFrame, AVMem, AVRational},
    error::RsmpegError,
    ffi::{self, AVCodecID_AV_CODEC_ID_H264},
};
use std::{
    ffi::CStr,
    fs::File,
    io::{Seek, SeekFrom, Write},
    sync::{Arc, Mutex},
};

/// Get `video_stream_index`, `input_format_context`, `decode_context`.
fn open_input_file(filename: &CStr) -> Result<(usize, AVFormatContextInput, AVCodecContext)> {
    let mut input_format_context = AVFormatContextInput::open(filename)?;
    input_format_context.dump(0, filename)?;

    let (video_index, decoder) = input_format_context
        .find_best_stream(ffi::AVMediaType_AVMEDIA_TYPE_VIDEO)
        .context("Failed to select a video stream")?
        .context("No video stream")?;

    let decode_context = {
        let input_stream = input_format_context.streams().get(video_index).unwrap();

        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context.apply_codecpar(&input_stream.codecpar())?;
        if let Some(framerate) = input_stream.guess_framerate() {
            decode_context.set_framerate(framerate);
        }
        decode_context.open(None)?;
        decode_context
    };

    Ok((video_index, input_format_context, decode_context))
}

/// Return output_format_context and encode_context
fn open_output_file(
    filename: &CStr,
    decode_context: &AVCodecContext,
) -> Result<(AVFormatContextOutput, AVCodecContext)> {
    let buffer = Arc::new(Mutex::new(File::create(filename.to_str()?)?));
    let buffer1 = buffer.clone();

    // Custom IO Context
    let io_context = AVIOContextCustom::alloc_context(
        AVMem::new(4096),
        true,
        vec![],
        None,
        Some(Box::new(move |_: &mut Vec<u8>, buf: &[u8]| {
            let mut buffer = buffer1.lock().unwrap();
            buffer.write_all(buf).unwrap();
            buf.len() as _
        })),
        Some(Box::new(
            move |_: &mut Vec<u8>, offset: i64, whence: i32| {
                println!("offset: {}, whence: {}", offset, whence);
                let mut buffer = match buffer.lock() {
                    Ok(x) => x,
                    Err(_) => return -1,
                };
                let mut seek_ = |offset: i64, whence: i32| -> Result<i64> {
                    Ok(match whence {
                        libc::SEEK_CUR => buffer.seek(SeekFrom::Current(offset))?,
                        libc::SEEK_SET => buffer.seek(SeekFrom::Start(offset as u64))?,
                        libc::SEEK_END => buffer.seek(SeekFrom::End(offset))?,
                        _ => return Err(anyhow!("Unsupported whence")),
                    } as i64)
                };
                seek_(offset, whence).unwrap_or(-1)
            },
        )),
    );

    let mut output_format_context =
        AVFormatContextOutput::create(filename, Some(AVIOContextContainer::Custom(io_context)))?;

    let encoder = AVCodec::find_encoder(AVCodecID_AV_CODEC_ID_H264)
        .with_context(|| anyhow!("encoder({}) not found.", AVCodecID_AV_CODEC_ID_H264))?;

    let mut encode_context = AVCodecContext::new(&encoder);
    encode_context.set_height(decode_context.height);
    encode_context.set_width(decode_context.width);
    encode_context.set_sample_aspect_ratio(decode_context.sample_aspect_ratio);
    encode_context.set_pix_fmt(if let Some(pix_fmts) = encoder.pix_fmts() {
        pix_fmts[0]
    } else {
        decode_context.pix_fmt
    });
    encode_context.set_time_base(av_inv_q(av_mul_q(
        decode_context.framerate,
        AVRational {
            num: decode_context.ticks_per_frame,
            den: 1,
        },
    )));

    // Some formats want stream headers to be separate.
    if output_format_context.oformat().flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
        encode_context.set_flags(encode_context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
    }

    encode_context.open(None)?;

    {
        let mut out_stream = output_format_context.new_stream();
        out_stream.set_codecpar(encode_context.extract_codecpar());
        out_stream.set_time_base(encode_context.time_base);
    }

    output_format_context.dump(0, filename)?;
    output_format_context.write_header(&mut None)?;

    Ok((output_format_context, encode_context))
}

/// encode -> write_frame
fn encode_write_frame(
    frame_after: Option<&AVFrame>,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    out_stream_index: usize,
) -> Result<()> {
    encode_context
        .send_frame(frame_after)
        .context("Encode frame failed.")?;

    loop {
        let mut packet = match encode_context.receive_packet() {
            Ok(packet) => packet,
            Err(RsmpegError::EncoderDrainError) | Err(RsmpegError::EncoderFlushedError) => break,
            Err(e) => bail!(e),
        };

        packet.set_stream_index(out_stream_index as i32);
        packet.rescale_ts(
            encode_context.time_base,
            output_format_context
                .streams()
                .get(out_stream_index)
                .unwrap()
                .time_base,
        );

        match output_format_context.interleaved_write_frame(&mut packet) {
            Ok(()) => Ok(()),
            Err(RsmpegError::InterleavedWriteFrameError(-22)) => Ok(()),
            Err(e) => Err(e),
        }
        .context("Interleaved write frame failed.")?;
    }

    Ok(())
}

/// Send an empty packet to the `encode_context` for packet flushing.
fn flush_encoder(
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    out_stream_index: usize,
) -> Result<()> {
    if encode_context.codec().capabilities & ffi::AV_CODEC_CAP_DELAY as i32 == 0 {
        return Ok(());
    }
    encode_write_frame(
        None,
        encode_context,
        output_format_context,
        out_stream_index,
    )?;
    Ok(())
}

/// Transcoding audio and video stream in a multi media file.
pub fn transcoding(input_file: &CStr, output_file: &CStr) -> Result<()> {
    let (video_stream_index, mut input_format_context, mut decode_context) =
        open_input_file(input_file)?;
    let (mut output_format_context, mut encode_context) =
        open_output_file(output_file, &decode_context)?;

    loop {
        let mut packet = match input_format_context.read_packet() {
            Ok(Some(x)) => x,
            // No more frames
            Ok(None) => break,
            Err(e) => bail!("Read frame error: {:?}", e),
        };

        if packet.stream_index as usize != video_stream_index {
            continue;
        }

        packet.rescale_ts(
            input_format_context
                .streams()
                .get(video_stream_index)
                .unwrap()
                .time_base,
            encode_context.time_base,
        );

        decode_context
            .send_packet(Some(&packet))
            .context("Send packet failed")?;

        loop {
            let mut frame = match decode_context.receive_frame() {
                Ok(frame) => frame,
                Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => {
                    break
                }
                Err(e) => bail!(e),
            };

            frame.set_pts(frame.best_effort_timestamp);
            encode_write_frame(
                Some(&frame),
                &mut encode_context,
                &mut output_format_context,
                0,
            )?;
        }
    }

    // Flush the encoder by pushing EOF frame to encode_context.
    flush_encoder(&mut encode_context, &mut output_format_context, 0)?;
    output_format_context.write_trailer()?;
    Ok(())
}

#[test]
fn avio_writing_test0() {
    std::fs::create_dir_all("tests/output/avio_writing/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/mov_sample.mov"),
        cstr!("tests/output/avio_writing/mov_sample.mp4"),
    )
    .unwrap();
}

#[test]
fn avio_writing_test1() {
    std::fs::create_dir_all("tests/output/avio_writing/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/centaur.mpg"),
        cstr!("tests/output/avio_writing/centaur.mp4"),
    )
    .unwrap();
}

#[test]
fn avio_writing_test2() {
    std::fs::create_dir_all("tests/output/avio_writing/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/bear.mp4"),
        cstr!("tests/output/avio_writing/bear.mp4"),
    )
    .unwrap();
}

#[test]
fn avio_writing_test3() {
    std::fs::create_dir_all("tests/output/avio_writing/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/vp8.mp4"),
        cstr!("tests/output/avio_writing/vp8.mp4"),
    )
    .unwrap();
}

#[test]
fn avio_writing_test4() {
    std::fs::create_dir_all("tests/output/avio_writing/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/big_buck_bunny.mp4"),
        cstr!("tests/output/avio_writing/big_buck_bunny.mp4"),
    )
    .unwrap();
}

#[test]
fn avio_writing_test5() {
    std::fs::create_dir_all("tests/output/avio_writing/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/with_pic.mp4"),
        cstr!("tests/output/avio_writing/with_pic.mp4"),
    )
    .unwrap();
}
