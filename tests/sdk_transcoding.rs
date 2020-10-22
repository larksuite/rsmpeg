#![feature(once_cell)]
// log.rs
use std::lazy::SyncLazy;
pub static LOG_INIT: SyncLazy<()> = SyncLazy::new(|| {
    env_logger::init();
    log::info!("env_logger initialized");
});

// error.rs
use rsmpeg::error::RsmpegError;
use thiserror::Error;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum LibMediaError {
    #[error("RsmpegError: {0}")]
    RsmpegError(#[from] rsmpeg::error::RsmpegError),
    #[error("CStringError: {0}")]
    CStringError(#[from] std::ffi::NulError),
    // #[error("ImageError: {0}")]
    // ImageError(#[from] image::ImageError),
    #[error("IoError: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Failed to get video metadata.")]
    MetadataGetError,
    #[error("No video stream.")]
    VideoStreamNotFound,
    #[error("Video parameter is invalid.")]
    InvalidVideoParameter,
    #[error("Packet info is invalid.")]
    InvalidPacketInfo,
    #[error("image crate meets invalid image.")]
    InvalidImageBuffer,
    #[error("Cannot found decoder for this video.")]
    DecoderNotFound,
    #[error("Cannot found encoder for this video.")]
    EncoderNotFound,
}

pub type Result<T> = std::result::Result<T, LibMediaError>;

// transcoding.rs
use log::{debug, error, info};
use rsmpeg::{
    self,
    avcodec::{AVCodec, AVCodecContext},
    avfilter::{AVFilter, AVFilterContextMut, AVFilterGraph, AVFilterInOut},
    avformat::{AVFormatContextInput, AVFormatContextOutput},
    avutil::{
        av_get_channel_layout_nb_channels, av_get_default_channel_layout, av_get_sample_fmt_name,
        av_inv_q, av_mul_q, AVFrame,
    },
    ffi,
};
use std::ffi::{CStr, CString};

struct StreamContext {
    decode_context: AVCodecContext,
    encode_context: Option<AVCodecContext>,
}

struct FilterContext<'graph> {
    buffer_src_context: AVFilterContextMut<'graph>,
    buffer_sink_context: AVFilterContextMut<'graph>,
}

macro_rules! cstr {
    ($s: literal) => {
        &CString::new($s).unwrap()
    };
    ($s: expr) => {
        &CString::new($s)?
    };
}

fn lib_media_is_transcoding_canceled(_: &str) -> bool {
    false
}

fn open_input_file(filename: &CStr) -> Result<(AVFormatContextInput, Vec<StreamContext>)> {
    let mut stream_contexts = vec![];
    let input_format_context = AVFormatContextInput::open(filename)?;
    let mut has_data_stream = false;

    for input_stream in input_format_context.streams().into_iter() {
        if input_stream.codecpar().codec_type == ffi::AVMediaType_AVMEDIA_TYPE_DATA {
            has_data_stream = true;
            continue;
        }
        if has_data_stream {
            // data stream要在video/audio等stream的后面，否则和output
            // file的stream_index对不上
            return Err(LibMediaError::DecoderNotFound);
        }
        let decoder = AVCodec::find_decoder(input_stream.codecpar().codec_id)
            .ok_or(LibMediaError::DecoderNotFound)?;
        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context.set_codecpar(input_stream.codecpar())?;

        if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            if let Some(framerate) = input_stream.guess_framerate() {
                decode_context.set_framerate(framerate);
            }
            decode_context.open(None)?;
        }
        stream_contexts.push(StreamContext {
            decode_context,
            encode_context: None,
        });
    }
    Ok((input_format_context, stream_contexts))
}

#[allow(clippy::unwrap_used, clippy::unwrap_used)]
fn open_output_file(
    filename: &CStr,
    input_format_context: &mut AVFormatContextInput,
    stream_contexts: &mut [StreamContext],
) -> Result<AVFormatContextOutput> {
    let mut output_format_context = AVFormatContextOutput::create(filename)?;

    for (i, in_stream) in input_format_context.streams().into_iter().enumerate() {
        let in_codecpar = in_stream.codecpar();
        if in_codecpar.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_DATA {
            continue;
        }

        let StreamContext {
            ref decode_context,
            ref mut encode_context,
        } = stream_contexts[i];

        let mut out_stream = output_format_context.new_stream(None);

        if in_codecpar.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            let encoder = AVCodec::find_encoder(ffi::AVCodecID_AV_CODEC_ID_H264)
                .ok_or(LibMediaError::EncoderNotFound)?;
            let mut new_encode_context = AVCodecContext::new(&encoder);

            new_encode_context.set_height(decode_context.height);
            new_encode_context.set_width(decode_context.width);
            new_encode_context.set_sample_aspect_ratio(decode_context.sample_aspect_ratio);
            new_encode_context.set_pix_fmt(if let Some(pix_fmts) = encoder.pix_fmts() {
                pix_fmts[0]
            } else {
                decode_context.pix_fmt
            });
            new_encode_context.set_bit_rate(decode_context.bit_rate);
            new_encode_context.set_time_base(decode_context.time_base);
            new_encode_context
                .set_flags(new_encode_context.flags | (ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32));
            // av_opt_set(enc_ctx->priv_data, "profile", "main", 0);
            new_encode_context.open(None)?;

            out_stream.set_codecpar(new_encode_context.extract_codecpar());

            out_stream.set_time_base(new_encode_context.time_base);

            *encode_context = Some(new_encode_context);
            // av_dict_copy(&out_stream->metadata, in_stream->metadata, AV_DICT_IGNORE_SUFFIX);
        } else if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_UNKNOWN {
            error!(
                "Elementary stream #{} is of unknown type, cannot proceed",
                i
            );
        } else {
            // when decode context is present, stream index should always be present.
            let in_stream = input_format_context.streams().get(i).unwrap();
            out_stream.set_codecpar(in_stream.codecpar().clone());
            out_stream.set_time_base(in_stream.time_base);
        }
    }

    /* already been placed in AVFormatContextOutput
    if (!((*ofmt_ctx)->oformat->flags & AVFMT_NOFILE)) {
        ret = avio_open(&(*ofmt_ctx)->pb, filename, AVIO_FLAG_WRITE);
        if (ret < 0) {
            av_log(NULL, AV_LOG_ERROR, "Could not open output file '%s'", filename);
            return ret;
        }
    }
    */

    // av_dict_copy(&(*ofmt_ctx)->metadata, ifmt_ctx->metadata, AV_DICT_IGNORE_SUFFIX);
    output_format_context.write_header()?;

    Ok(output_format_context)
}

#[allow(clippy::unwrap_used, clippy::unwrap_used)]
fn init_filter<'graph>(
    filter_graph: &'graph mut AVFilterGraph,
    decode_context: &mut AVCodecContext,
    encode_context: &mut AVCodecContext,
    filter_spec: &CStr,
) -> Result<FilterContext<'graph>> {
    let mut filter_graph = filter_graph;
    let (mut buffer_src_context, mut buffer_sink_context) =
        if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            // These two filters should always be present, so unwrap
            let buffer_src = AVFilter::get_by_name(cstr!("buffer")).unwrap();
            let buffer_sink = AVFilter::get_by_name(cstr!("buffersink")).unwrap();

            // Shouldn't have issue on convertion since it's ascii.
            let args = CString::new(format!(
                "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
                decode_context.width,
                decode_context.height,
                decode_context.pix_fmt,
                decode_context.time_base.num,
                decode_context.time_base.den,
                decode_context.sample_aspect_ratio.num,
                decode_context.sample_aspect_ratio.den,
            ))
            .unwrap();

            let (filter_graph_new, buffer_src_context) =
                filter_graph.create_filter_context(&buffer_src, cstr!("in"), Some(&args))?;

            filter_graph = filter_graph_new;

            let (filter_graph_new, mut buffer_sink_context) =
                filter_graph.create_filter_context(&buffer_sink, cstr!("out"), None)?;

            filter_graph = filter_graph_new;

            buffer_sink_context.set_property(cstr!("pix_fmts"), &encode_context.pix_fmt)?;
            (buffer_src_context, buffer_sink_context)
        } else {
            // Other stream type shouldn't call this function.
            panic!("Only video needs filter initialization")
        };

    // Yes the outputs' name is `in` -_-b
    let outputs = AVFilterInOut::new(cstr!("in"), &mut buffer_src_context);
    let inputs = AVFilterInOut::new(cstr!("out"), &mut buffer_sink_context);
    let (_inputs, _outputs) = filter_graph.parse_ptr(filter_spec, inputs, outputs)?;
    filter_graph.config()?;
    Ok(FilterContext {
        buffer_src_context,
        buffer_sink_context,
    })
}

#[allow(clippy::unwrap_used, clippy::unwrap_used)]
fn init_filters<'graph>(
    filter_graphs: &'graph mut [AVFilterGraph],
    stream_contexts: &mut [StreamContext],
) -> Result<Vec<Option<FilterContext<'graph>>>> {
    let mut filter_contexts = vec![];
    let filter_graphs_iter = filter_graphs.iter_mut();
    let stream_contexts_iter = stream_contexts.iter_mut();
    for (
        filter_graph,
        StreamContext {
            decode_context,
            encode_context,
        },
    ) in filter_graphs_iter.zip(stream_contexts_iter)
    {
        let media_type = decode_context.codec_type;

        let filter_context = if media_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            // We can ensure the encode_context is Some(_) when it's video stream.
            let encode_context = encode_context.as_mut().unwrap();

            // Scale filter
            // Shouldn't have convertion issue since it's pure ascii.
            let filter_spec = &CString::new(format!(
                "scale={}:{}",
                encode_context.width, encode_context.height
            ))
            .unwrap();

            Some(init_filter(
                filter_graph,
                decode_context,
                encode_context,
                filter_spec,
            )?)
        } else {
            None
        };
        filter_contexts.push(filter_context);
    }

    Ok(filter_contexts)
}

/// Returns if we get the packet
#[allow(deprecated)]
#[allow(clippy::unwrap_used, clippy::unwrap_used)]
fn encode_write_frame(
    frame_after: Option<&AVFrame>,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
) -> Result<bool> {
    let packet = encode_context.encode_frame(frame_after)?;
    if let Some(mut packet) = packet {
        packet.set_stream_index(stream_index as i32);
        // Trust the stream_index, so unwrap
        packet.rescale_ts(
            encode_context.time_base,
            output_format_context
                .streams()
                .get(stream_index)
                .unwrap()
                .time_base,
        );
        println!("{:?}", packet);
        match output_format_context.interleaved_write_frame(&mut packet) {
            Ok(()) => Ok(()),
            Err(RsmpegError::InterleavedWriteFrameError(-22)) => {
                log::warn!("Ignore a non mono-increasing time_stamp frame.");
                Ok(())
            }
            Err(e) => Err(e),
        }?;
        Ok(true)
    } else {
        Ok(false)
    }
}

fn filter_encode_write_frame(
    frame_before: Option<AVFrame>,
    buffer_src_context: &mut AVFilterContextMut,
    buffer_sink_context: &mut AVFilterContextMut,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
) -> Result<()> {
    buffer_src_context.buffersrc_add_frame_flags(frame_before, 0)?;
    loop {
        let mut frame_after = match buffer_sink_context.buffersink_get_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::BufferSinkDrainError) | Err(RsmpegError::BufferSinkEofError) => break,
            _ => panic!("Get frame from buffer sink failed."),
        };
        frame_after.set_pict_type(ffi::AVPictureType_AV_PICTURE_TYPE_NONE);
        // It doesn't matter if we don't get any packet, we can push more frame later.
        let _ = encode_write_frame(
            Some(&frame_after),
            encode_context,
            output_format_context,
            stream_index,
        )?;
    }
    Ok(())
}

fn flush_encoder(
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
) -> Result<()> {
    if encode_context.codec().capabilities & ffi::AV_CODEC_CAP_DELAY as i32 != 0 {
        loop {
            let got_packet =
                encode_write_frame(None, encode_context, output_format_context, stream_index)?;
            // When the encoder is drained
            if !got_packet {
                break;
            }
        }
    }
    Ok(())
}

#[allow(deprecated)]
/// Return `Result<is_cancelled>`.
#[allow(clippy::unwrap_used, clippy::unwrap_used)]
pub fn transcoding(key: &str, input_file: &CStr, output_file: &CStr) -> Result<bool> {
    let (mut input_format_context, mut stream_contexts) = open_input_file(input_file)?;
    let mut output_format_context =
        open_output_file(output_file, &mut input_format_context, &mut stream_contexts)?;

    let mut filter_graphs = (0..stream_contexts.len()).fold(vec![], |mut filter_graphs, _| {
        filter_graphs.push(AVFilterGraph::new());
        filter_graphs
    });
    let mut filter_contexts = init_filters(&mut filter_graphs, &mut stream_contexts)?;

    let mut checkpoint = 0;
    let mut is_cancelled = false;
    loop {
        checkpoint += 1;
        if checkpoint % 5 == 1 && lib_media_is_transcoding_canceled(key) {
            is_cancelled = true;
            break;
        }
        let mut packet = match input_format_context.read_packet() {
            Ok(Some(x)) => x,
            Ok(None) => {
                // No more frames
                break;
            }
            Err(e) => {
                error!("Read frame error: {:?}", e);
                break;
            }
        };

        let stream_index = packet.stream_index as usize;
        let input_stream = input_format_context
            .streams()
            .get(stream_index)
            .ok_or(LibMediaError::InvalidPacketInfo)?;
        let media_type = input_stream.codecpar().codec_type;
        if media_type == ffi::AVMediaType_AVMEDIA_TYPE_DATA {
            continue;
        }
        if media_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            // Video stream should already got the buffer filter context and encode context,
            // so unwrap()
            let FilterContext {
                buffer_src_context,
                buffer_sink_context,
            } = (&mut filter_contexts[stream_index]).as_mut().unwrap();
            let StreamContext {
                decode_context,
                encode_context,
            } = &mut stream_contexts[stream_index];
            let encode_context = encode_context.as_mut().unwrap();

            packet.rescale_ts(input_stream.time_base, encode_context.time_base);

            let decode_result = match decode_context.decode_packet(&packet) {
                Ok(x) => x,
                // fix(media): continue to decode when meeting bad frame
                Err(_) => continue,
            };

            if let Some(mut frame) = decode_result {
                frame.set_pts(frame.best_effort_timestamp);
                filter_encode_write_frame(
                    Some(frame),
                    buffer_src_context,
                    buffer_sink_context,
                    encode_context,
                    &mut output_format_context,
                    stream_index,
                )?;
            } else {
                // do nothing wait to another loop
            }
        } else {
            // Output stream should corresponds to input stream, since the input
            // stream is present, output stream should also be present. So unwrap.
            packet.rescale_ts(
                input_stream.time_base,
                output_format_context
                    .streams()
                    .get(stream_index)
                    .unwrap()
                    .time_base,
            );
            output_format_context.interleaved_write_frame(&mut packet)?;
        }
    }

    // Flush the filter graph by pushing EOF packet to buffer_src
    // Flush the encoder by pushing EOF frame to encoder_context
    for stream_index in 0..stream_contexts.len() {
        // Input stream should be present, so unwrap();
        let media_type = input_format_context
            .streams()
            .get(stream_index)
            .unwrap()
            .codecpar()
            .codec_type;
        if media_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            // Video stream got these things, so unwrap();
            let FilterContext {
                buffer_src_context,
                buffer_sink_context,
            } = (&mut filter_contexts[stream_index]).as_mut().unwrap();
            let StreamContext {
                decode_context: _,
                encode_context,
            } = &mut stream_contexts[stream_index];
            let encode_context = encode_context.as_mut().unwrap();

            filter_encode_write_frame(
                None,
                buffer_src_context,
                buffer_sink_context,
                encode_context,
                &mut output_format_context,
                stream_index,
            )?;
            flush_encoder(encode_context, &mut output_format_context, stream_index)?;
        }
    }
    output_format_context.write_trailer()?;
    Ok(is_cancelled)
}

#[test]
fn sdk_transcoding_test0() {
    transcoding(
        "",
        cstr!("tests/utils/transcoding/bear.mp4"),
        cstr!("tests/utils/transcoding/bear_transcoded.mp4"),
    )
    .unwrap();
}

#[test]
fn sdk_transcoding_test1() {
    transcoding(
        "",
        cstr!("tests/utils/transcoding/enlong.mp4"),
        cstr!("tests/utils/transcoding/enlong_transcoded.mp4"),
    )
    .unwrap();
}

#[test]
fn sdk_transcoding_test2() {
    transcoding(
        "",
        cstr!("tests/utils/sdk_transcoding/emm.mp4"),
        cstr!("tests/utils/sdk_transcoding/emm_transcoded.mp4"),
    )
    .unwrap();
}

#[test]
fn sdk_transcoding_test3() {
    transcoding(
        "",
        cstr!("tests/utils/sdk_transcoding/AR_FireFly_Demo_output.mp4"),
        cstr!("tests/utils/sdk_transcoding/AR_FireFly_Demo_output_transcoded.mp4"),
    )
    .unwrap();
}

