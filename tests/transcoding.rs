use anyhow::{anyhow, bail, Context, Result};
use cstr::cstr;
use rsmpeg::{
    self,
    avcodec::{AVCodec, AVCodecContext},
    avfilter::{AVFilter, AVFilterContextMut, AVFilterGraph, AVFilterInOut},
    avformat::{AVFormatContextInput, AVFormatContextOutput},
    avutil::{
        av_get_channel_layout_nb_channels, av_get_default_channel_layout, av_inv_q, av_mul_q,
        get_sample_fmt_name, AVFrame, AVRational,
    },
    error::RsmpegError,
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

fn open_input_file(filename: &CStr) -> Result<(AVFormatContextInput, Vec<StreamContext>)> {
    let mut stream_contexts = vec![];
    let mut input_format_context = AVFormatContextInput::open(filename)?;

    for input_stream in input_format_context.streams().into_iter() {
        let codec_id = input_stream.codecpar().codec_id;
        let decoder = AVCodec::find_decoder(codec_id)
            .with_context(|| anyhow!("decoder ({}) not found.", codec_id))?;

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

fn open_output_file(
    filename: &CStr,
    input_format_context: &mut AVFormatContextInput,
    stream_contexts: &mut [StreamContext],
) -> Result<AVFormatContextOutput> {
    let mut output_format_context = AVFormatContextOutput::create(filename)?;

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
            let encoder = AVCodec::find_encoder(decode_context.codec_id)
                .with_context(|| anyhow!("encoder({}) not found.", decode_context.codec_id))?;
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
                    AVRational {
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
                new_encode_context.set_sample_fmt(encoder.sample_fmts().unwrap()[0]);
                new_encode_context.set_time_base(AVRational {
                    num: 1,
                    den: decode_context.sample_rate,
                });
            }
            if output_format_context.oformat().flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
                new_encode_context
                    .set_flags(new_encode_context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
            }

            new_encode_context.open(None)?;

            let mut out_stream = output_format_context.new_stream();
            out_stream.set_codecpar(new_encode_context.extract_codecpar());
            out_stream.set_time_base(new_encode_context.time_base);

            *encode_context = Some(new_encode_context)
        } else if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_UNKNOWN {
            bail!("Stream #{} is of unknown type.", i);
        } else {
            let in_stream = input_format_context.streams().get(i).unwrap();
            let mut out_stream = output_format_context.new_stream();
            out_stream.set_codecpar(in_stream.codecpar().clone());
            out_stream.set_time_base(in_stream.time_base);
        }
    }

    output_format_context.dump(0, filename)?;
    output_format_context.write_header()?;

    Ok(output_format_context)
}

fn init_filter<'graph>(
    filter_graph: &'graph mut AVFilterGraph,
    decode_context: &mut AVCodecContext,
    encode_context: &mut AVCodecContext,
    filter_spec: &CStr,
) -> Result<FilterContext<'graph>> {
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

            let args = &CString::new(args).unwrap();

            let buffer_src_context =
                filter_graph.create_filter_context(&buffer_src, cstr!("in"), Some(args))?;

            let mut buffer_sink_context =
                filter_graph.create_filter_context(&buffer_sink, cstr!("out"), None)?;
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
                // We can unwrap here, because we are sure that the given
                // sample_fmt is valid.
                get_sample_fmt_name(decode_context.sample_fmt)
                    .unwrap()
                    .to_string_lossy(),
                decode_context.channel_layout,
            );
            let args = &CString::new(args).unwrap();

            let buffer_src_context =
                filter_graph.create_filter_context(&buffer_src, cstr!("in"), Some(args))?;

            let mut buffer_sink_context =
                filter_graph.create_filter_context(&buffer_sink, cstr!("out"), None)?;
            buffer_sink_context.set_property(cstr!("sample_fmts"), &encode_context.sample_fmt)?;
            buffer_sink_context
                .set_property(cstr!("channel_layouts"), &encode_context.channel_layout)?;
            buffer_sink_context.set_property(cstr!("sample_rates"), &encode_context.sample_rate)?;

            (buffer_src_context, buffer_sink_context)
        } else {
            bail!("Only video and audio needs filter initialization")
        };

    // Yes the outputs' name is `in` -_-b
    let outputs = AVFilterInOut::new(cstr!("in"), &mut buffer_src_context, 0);
    let inputs = AVFilterInOut::new(cstr!("out"), &mut buffer_sink_context, 0);
    let (_inputs, _outputs) = filter_graph.parse_ptr(filter_spec, inputs, outputs)?;

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
                cstr!("null")
            } else {
                cstr!("anull")
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

fn encode_write_frame(
    frame_after: Option<&AVFrame>,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
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
            Err(RsmpegError::InterleavedWriteFrameError(-22)) => Ok(()),
            Err(e) => Err(e),
        }
        .context("Interleaved write frame failed.")?;
    }

    Ok(())
}

fn filter_encode_write_frame(
    frame_before: Option<AVFrame>,
    buffer_src_context: &mut AVFilterContextMut,
    buffer_sink_context: &mut AVFilterContextMut,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
) -> Result<()> {
    buffer_src_context
        .buffersrc_add_frame(frame_before, None)
        .context("Error while feeding the filtergraph")?;
    loop {
        let mut frame_after = match buffer_sink_context.buffersink_get_frame(None) {
            Ok(frame) => frame,
            Err(RsmpegError::BufferSinkDrainError) | Err(RsmpegError::BufferSinkEofError) => break,
            Err(_) => bail!("Get frame from buffer sink failed."),
        };
        frame_after.set_pict_type(ffi::AVPictureType_AV_PICTURE_TYPE_NONE);

        encode_write_frame(
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
    if encode_context.codec().capabilities & ffi::AV_CODEC_CAP_DELAY as i32 == 0 {
        return Ok(());
    }
    encode_write_frame(None, encode_context, output_format_context, stream_index)?;
    Ok(())
}

pub fn transcoding(input_file: &CStr, output_file: &CStr) -> Result<()> {
    let (mut input_format_context, mut stream_contexts) = open_input_file(input_file)?;
    let mut output_format_context =
        open_output_file(output_file, &mut input_format_context, &mut stream_contexts)?;

    let mut filter_graphs: Vec<_> = (0..stream_contexts.len())
        .map(|_| AVFilterGraph::new())
        .collect();
    let mut filter_contexts = init_filters(&mut filter_graphs, &mut stream_contexts)?;

    loop {
        let mut packet = match input_format_context.read_packet() {
            Ok(Some(x)) => x,
            // No more frames
            Ok(None) => break,
            Err(e) => bail!("Read frame error: {:?}", e),
        };

        let stream_index = packet.stream_index as usize;
        let input_stream = input_format_context.streams().get(stream_index).unwrap();
        let media_type = input_stream.codecpar().codec_type;
        if media_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO
            || media_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO
        {
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

            decode_context.send_packet(Some(&packet)).unwrap();

            loop {
                let mut frame = match decode_context.receive_frame() {
                    Ok(frame) => frame,
                    Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => {
                        break
                    }
                    Err(e) => bail!(e),
                };

                frame.set_pts(frame.best_effort_timestamp);
                filter_encode_write_frame(
                    Some(frame),
                    buffer_src_context,
                    buffer_sink_context,
                    encode_context,
                    &mut output_format_context,
                    stream_index,
                )?;
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
                .context("Interleaved write frame failed.")?;
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
            )?;
            flush_encoder(encode_context, &mut output_format_context, stream_index)?;
        }
    }
    output_format_context.write_trailer()?;
    Ok(())
}

#[test]
fn transcoding_test0() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/mov_sample.mov"),
        cstr!("tests/output/transcoding/mov_sample.mov"),
    )
    .unwrap();
}

#[test]
fn transcoding_test1() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/centaur.mpg"),
        cstr!("tests/output/transcoding/centaur.mpg"),
    )
    .unwrap();
}

#[test]
fn transcoding_test2() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/bear.mp4"),
        cstr!("tests/output/transcoding/bear.mp4"),
    )
    .unwrap();
}

#[test]
fn transcoding_test3() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/vp8.mp4"),
        cstr!("tests/output/transcoding/vp8.webm"),
    )
    .unwrap();
}
