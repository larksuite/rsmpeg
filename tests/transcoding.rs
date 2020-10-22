#![feature(once_cell)]
// log.rs
use std::lazy::SyncLazy;
pub static LOG_INIT: SyncLazy<()> = SyncLazy::new(|| {
    env_logger::init();
    log::info!("env_logger initialized");
});

// error.rs
pub use rsmpeg::error::RsmpegError;
use std::cmp::{Eq, PartialEq};
#[derive(Debug, Eq, PartialEq)]
pub enum LibMediaError {
    RsmpegError(rsmpeg::error::RsmpegError),
    CStringError(std::ffi::NulError),
}

impl From<rsmpeg::error::RsmpegError> for LibMediaError {
    fn from(e: rsmpeg::error::RsmpegError) -> Self {
        LibMediaError::RsmpegError(e)
    }
}

impl From<std::ffi::NulError> for LibMediaError {
    fn from(e: std::ffi::NulError) -> Self {
        LibMediaError::CStringError(e)
    }
}

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
use std::ffi::CString;

pub type Result<T> = std::result::Result<T, LibMediaError>;

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

fn open_input_file<T: Into<Vec<u8>>>(
    filename: T,
) -> Result<(AVFormatContextInput, Vec<StreamContext>)> {
    let filename = cstr!(filename);
    let mut stream_contexts = vec![];
    let mut input_format_context = AVFormatContextInput::open(filename)?;

    for input_stream in input_format_context.streams().into_iter() {
        let decoder = AVCodec::find_decoder(input_stream.codecpar().codec_id).unwrap();
        let mut decode_context = AVCodecContext::new(&decoder);
        decode_context.set_codecpar(input_stream.codecpar())?;

        if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            if let Some(framerate) = input_stream.guess_framerate() {
                decode_context.set_framerate(framerate);
            }
            decode_context.open(None)?;
        } else if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO {
            decode_context.open(None)?;
        }
        stream_contexts.push(StreamContext {
            decode_context,
            encode_context: None,
        });
    }
    input_format_context.dump(0, filename)?;
    Ok((input_format_context, stream_contexts))
}

fn open_output_file<T: Into<Vec<u8>>>(
    filename: T,
    input_format_context: &mut AVFormatContextInput,
    stream_contexts: &mut [StreamContext],
) -> Result<AVFormatContextOutput> {
    let filename = cstr!(filename);
    let mut output_format_context = AVFormatContextOutput::create(filename.as_ref())?;

    for (
        i,
        StreamContext {
            decode_context,
            encode_context,
        },
    ) in stream_contexts.iter_mut().enumerate()
    {
        if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO
            || decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO
        {
            let encoder = AVCodec::find_encoder(decode_context.codec_id).expect(&format!(
                "Necessary encoder: {} not found.",
                decode_context.codec_id
            ));
            let mut new_encode_context = AVCodecContext::new(&encoder);

            if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
                new_encode_context.set_height(decode_context.height);
                new_encode_context.set_width(decode_context.width);
                new_encode_context.set_sample_aspect_ratio(decode_context.sample_aspect_ratio);
                new_encode_context.set_pix_fmt(if let Some(pix_fmts) = encoder.pix_fmts() {
                    pix_fmts[0]
                } else {
                    decode_context.pix_fmt
                });
                new_encode_context.set_time_base(av_inv_q(av_mul_q(
                    decode_context.framerate,
                    ffi::AVRational {
                        num: decode_context.ticks_per_frame,
                        den: 1,
                    },
                )));
            } else {
                new_encode_context.set_sample_rate(decode_context.sample_rate);
                new_encode_context.set_channel_layout(decode_context.channel_layout);
                new_encode_context.set_channels(av_get_channel_layout_nb_channels(
                    decode_context.channel_layout,
                ));
                /* take first format from list of supported formats */
                new_encode_context.set_sample_fmt(encoder.sample_fmts().unwrap()[0]);
                new_encode_context.set_time_base(ffi::AVRational {
                    num: 1,
                    den: decode_context.sample_rate,
                });
            }
            if output_format_context.oformat().flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
                new_encode_context
                    .set_flags(new_encode_context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
            }

            new_encode_context.open(None)?;

            let mut out_stream = output_format_context.new_stream(None);

            out_stream.set_codecpar(new_encode_context.extract_codecpar());

            out_stream.set_time_base(new_encode_context.time_base);

            *encode_context = Some(new_encode_context)
        } else if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_UNKNOWN {
            panic!(
                "Elementary stream #{} is of unknown type, cannot proceed",
                i
            )
        } else {
            let in_stream = input_format_context.streams().get(i).unwrap();
            let mut out_stream = output_format_context.new_stream(None);
            out_stream.set_codecpar(in_stream.codecpar().clone());

            out_stream.set_time_base(in_stream.time_base);
        }
    }

    output_format_context.dump(0, filename.as_ref())?;

    output_format_context.write_header()?;

    Ok(output_format_context)
}

fn init_filter<'graph, T: Into<Vec<u8>>>(
    filter_graph: &'graph mut AVFilterGraph,
    decode_context: &mut AVCodecContext,
    encode_context: &mut AVCodecContext,
    filter_spec: T,
) -> Result<FilterContext<'graph>> {
    let mut filter_graph = filter_graph;
    let (mut buffer_src_context, mut buffer_sink_context) =
        if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            let buffer_src = AVFilter::get_by_name(cstr!("buffer")).unwrap();
            let buffer_sink = AVFilter::get_by_name(cstr!("buffersink")).unwrap();

            let args = format!(
                "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
                decode_context.width,
                decode_context.height,
                decode_context.pix_fmt,
                decode_context.time_base.num,
                decode_context.time_base.den,
                decode_context.sample_aspect_ratio.num,
                decode_context.sample_aspect_ratio.den,
            );

            let (filter_graph_new, buffer_src_context) =
                filter_graph.create_filter_context(&buffer_src, cstr!("in"), Some(cstr!(args)))?;

            filter_graph = filter_graph_new;

            let (filter_graph_new, mut buffer_sink_context) =
                filter_graph.create_filter_context(&buffer_sink, cstr!("out"), None)?;

            filter_graph = filter_graph_new;

            buffer_sink_context.set_property(cstr!("pix_fmts"), &encode_context.pix_fmt)?;
            (buffer_src_context, buffer_sink_context)
        } else if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO {
            let buffer_src = AVFilter::get_by_name(cstr!("abuffer")).unwrap();
            let buffer_sink = AVFilter::get_by_name(cstr!("abuffersink")).unwrap();

            if decode_context.channel_layout == 0 {
                let channel_layout = av_get_default_channel_layout(decode_context.channels);
                decode_context.set_channel_layout(channel_layout);
            }

            let args = format!(
                "time_base={}/{}:sample_rate={}:sample_fmt={}:channel_layout=0x{}",
                decode_context.time_base.num,
                decode_context.time_base.den,
                decode_context.sample_rate,
                av_get_sample_fmt_name(decode_context.sample_fmt).to_string_lossy(),
                decode_context.channel_layout,
            );

            let (filter_graph_new, buffer_src_context) =
                filter_graph.create_filter_context(&buffer_src, cstr!("in"), Some(cstr!(args)))?;

            filter_graph = filter_graph_new;

            let (filter_graph_new, mut buffer_sink_context) =
                filter_graph.create_filter_context(&buffer_sink, cstr!("out"), None)?;

            filter_graph = filter_graph_new;

            buffer_sink_context.set_property(cstr!("sample_fmts"), &encode_context.sample_fmt)?;
            buffer_sink_context
                .set_property(cstr!("channel_layouts"), &encode_context.channel_layout)?;
            buffer_sink_context.set_property(cstr!("sample_rates"), &encode_context.sample_rate)?;
            (buffer_src_context, buffer_sink_context)
        } else {
            panic!("Only video and audio needs filter initialization")
        };

    // Yes the outputs' name is `in` -_-b
    let outputs = AVFilterInOut::new(cstr!("in"), &mut buffer_src_context);
    let inputs = AVFilterInOut::new(cstr!("out"), &mut buffer_sink_context);
    let (_inputs, _outputs) = filter_graph.parse_ptr(cstr!(filter_spec), inputs, outputs)?;
    filter_graph.config()?;
    Ok(FilterContext {
        buffer_src_context,
        buffer_sink_context,
    })
}

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

        let filter_context = if media_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO
            || media_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO
        {
            // dummy filter
            let filter_spec = if media_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
                "null"
            } else {
                "anull"
            };

            // We can ensure the encode_context is Some(_) here.
            let encode_context = encode_context.as_mut().unwrap();
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
fn encode_write_frame(
    frame_after: Option<&AVFrame>,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
) -> bool {
    info!("Encoding frame");
    let packet = encode_context
        .encode_frame(frame_after)
        .expect("Encode frame failed.");
    if let Some(mut packet) = packet {
        packet.set_stream_index(stream_index as i32);
        packet.rescale_ts(
            encode_context.time_base,
            output_format_context
                .streams()
                .get(stream_index)
                .unwrap()
                .time_base,
        );
        match output_format_context.interleaved_write_frame(&mut packet) {
            Ok(()) => Ok(()),
            Err(RsmpegError::InterleavedWriteFrameError(-22)) => {
                log::warn!("Ignore a non mono-increasing time_stamp frame.");
                Ok(())
            }
            Err(e) => Err(e),
        }
        .expect("Interleaved write frame failed.");
        true
    } else {
        false
    }
}

fn filter_encode_write_frame(
    frame_before: Option<AVFrame>,
    buffer_src_context: &mut AVFilterContextMut,
    buffer_sink_context: &mut AVFilterContextMut,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
) {
    info!("Pushing decoded frame to filters");
    buffer_src_context
        .buffersrc_add_frame_flags(frame_before, 0)
        .expect("Error while feeding the filtergraph");
    loop {
        info!("Pulling filtered frame from filters");
        let mut frame_after = match buffer_sink_context.buffersink_get_frame() {
            Ok(frame) => frame,
            Err(RsmpegError::BufferSinkDrainError) | Err(RsmpegError::BufferSinkEofError) => break,
            Err(_) => panic!("Get frame from buffer sink failed."),
        };
        frame_after.set_pict_type(ffi::AVPictureType_AV_PICTURE_TYPE_NONE);
        // It doesn't matter if we don't get any packet, we can push more frame later.
        let _ = encode_write_frame(
            Some(&frame_after),
            encode_context,
            output_format_context,
            stream_index,
        );
    }
}

fn flush_encoder(
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
) {
    if encode_context.codec().capabilities & ffi::AV_CODEC_CAP_DELAY as i32 != 0 {
        loop {
            info!("Flushing stream #{} encoder", stream_index);
            let got_packet =
                encode_write_frame(None, encode_context, output_format_context, stream_index);
            // When the encoder is drained
            if !got_packet {
                break;
            }
        }
    }
}

#[allow(deprecated)]
pub fn transcoding(input_file: &str, output_file: &str) -> Result<()> {
    let _ = *LOG_INIT;

    let (mut input_format_context, mut stream_contexts) = open_input_file(input_file)?;
    let mut output_format_context =
        open_output_file(output_file, &mut input_format_context, &mut stream_contexts)?;

    let mut filter_graphs = (0..stream_contexts.len()).fold(vec![], |mut filter_graphs, _| {
        filter_graphs.push(AVFilterGraph::new());
        filter_graphs
    });
    let mut filter_contexts = init_filters(&mut filter_graphs, &mut stream_contexts)?;

    loop {
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
        let input_stream = input_format_context.streams().get(stream_index).unwrap();
        let media_type = input_stream.codecpar().codec_type;
        debug!("Demuxer gave frame of stream_index {}", stream_index);
        if media_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO
            || media_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO
        {
            debug!("Going to reencode&filter the frame.");
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

            let decode_result = decode_context
                .decode_packet(&packet)
                .expect("Decoding failed.");

            if let Some(mut frame) = decode_result {
                frame.set_pts(frame.best_effort_timestamp);
                filter_encode_write_frame(
                    Some(frame),
                    buffer_src_context,
                    buffer_sink_context,
                    encode_context,
                    &mut output_format_context,
                    stream_index,
                );
            } else {
                // do nothing wait to another loop
            }
        } else {
            packet.rescale_ts(
                input_stream.time_base,
                output_format_context
                    .streams()
                    .get(stream_index)
                    .unwrap()
                    .time_base,
            );
            output_format_context
                .interleaved_write_frame(&mut packet)
                .expect("Interleaved write frame failed.");
        }
    }

    // Flush the filter graph by pushing EOF packet to buffer_src
    // Flush the encoder by pushing EOF frame to encoder_context
    for stream_index in 0..stream_contexts.len() {
        let media_type = input_format_context
            .streams()
            .get(stream_index)
            .unwrap()
            .codecpar()
            .codec_type;
        if media_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO
            || media_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO
        {
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
            );
            flush_encoder(encode_context, &mut output_format_context, stream_index);
        }
    }
    output_format_context.write_trailer()?;
    Ok(())
}

#[test]
fn transcoding_test() {
    transcoding(
        "tests/utils/transcoding/bear.mp4",
        "tests/utils/transcoding/bear_transcoded.mp4",
    )
    .unwrap();
    transcoding(
        "tests/utils/transcoding/enlong.mp4",
        "tests/utils/transcoding/enlong_transcoded.mp4",
    )
    .unwrap();
    transcoding(
        "tests/utils/transcoding/emm.mp4",
        "tests/utils/transcoding/emm_transcoded.mp4",
    )
    .unwrap();
}

#[test]
fn transcoding_unpresent_file() {
    assert_eq!(
        transcoding("asd;fklasdlfkadsfads/bear.mp4", "tests/bear_transcoded.mp4"),
        Err(LibMediaError::RsmpegError(RsmpegError::OpenInputError))
    );
}
