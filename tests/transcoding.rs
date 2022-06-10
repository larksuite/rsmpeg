use anyhow::{anyhow, bail, Context, Result};
use cstr::cstr;
use rsmpeg::{
    self,
    avcodec::{AVCodec, AVCodecContext},
    avfilter::{AVFilter, AVFilterContextMut, AVFilterGraph, AVFilterInOut},
    avformat::{AVFormatContextInput, AVFormatContextOutput},
    avutil::{
        av_get_channel_layout_nb_channels, av_get_default_channel_layout, av_inv_q, av_mul_q,
        get_sample_fmt_name, ra, AVDictionary, AVFrame,
    },
    error::RsmpegError,
    ffi,
};
use std::ffi::{CStr, CString};

struct StreamContext {
    decode_context: AVCodecContext,
    encode_context: AVCodecContext,
    out_stream_index: usize,
}

struct FilterContext<'graph> {
    buffer_src_context: AVFilterContextMut<'graph>,
    buffer_sink_context: AVFilterContextMut<'graph>,
}

struct TranscodingContext<'graph> {
    decode_context: AVCodecContext,
    encode_context: AVCodecContext,
    out_stream_index: usize,
    buffer_src_context: AVFilterContextMut<'graph>,
    buffer_sink_context: AVFilterContextMut<'graph>,
}

/// Get `decode_contexts`, `input_format_context`, the length of
/// `decode_context` equals to the stream num of the input file. And each decode
/// context corresponds to each stream, if the stream is neither audio nor
/// audio, decode context at this index is set to `None`.
fn open_input_file(filename: &CStr) -> Result<(Vec<Option<AVCodecContext>>, AVFormatContextInput)> {
    let mut stream_contexts = vec![];
    let mut input_format_context = AVFormatContextInput::open(filename)?;

    for input_stream in input_format_context.streams().into_iter() {
        let codecpar = input_stream.codecpar();
        let codec_type = codecpar.codec_type;

        let decode_context = match codec_type {
            ffi::AVMediaType_AVMEDIA_TYPE_VIDEO => {
                let codec_id = codecpar.codec_id;
                let decoder = AVCodec::find_decoder(codec_id)
                    .with_context(|| anyhow!("video decoder ({}) not found.", codec_id))?;
                let mut decode_context = AVCodecContext::new(&decoder);
                decode_context.apply_codecpar(&codecpar)?;
                if let Some(framerate) = input_stream.guess_framerate() {
                    decode_context.set_framerate(framerate);
                }
                decode_context.open(None)?;
                Some(decode_context)
            }
            ffi::AVMediaType_AVMEDIA_TYPE_AUDIO => {
                let codec_id = codecpar.codec_id;
                let decoder = AVCodec::find_decoder(codec_id)
                    .with_context(|| anyhow!("audio decoder ({}) not found.", codec_id))?;
                let mut decode_context = AVCodecContext::new(&decoder);
                decode_context.apply_codecpar(&codecpar)?;
                decode_context.open(None)?;
                Some(decode_context)
            }
            _ => None,
        };

        stream_contexts.push(decode_context);
    }
    input_format_context.dump(0, filename)?;
    Ok((stream_contexts, input_format_context))
}

/// Accepts a output filename, attach `encode_context` to the corresponding
/// `decode_context` and wrap them into a `stream_context`. `stream_context` is
/// None when the given `decode_context` in the same index is None.
fn open_output_file(
    filename: &CStr,
    decode_contexts: Vec<Option<AVCodecContext>>,
    dict: &mut Option<AVDictionary>,
) -> Result<(Vec<Option<StreamContext>>, AVFormatContextOutput)> {
    let mut output_format_context = AVFormatContextOutput::create(filename, None)?;
    let mut stream_contexts = vec![];

    for decode_context in decode_contexts {
        let stream_context = if let Some(decode_context) = decode_context {
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
                    ra(decode_context.ticks_per_frame, 1),
                )));
            } else if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO {
                new_encode_context.set_sample_rate(decode_context.sample_rate);
                new_encode_context.set_channel_layout(decode_context.channel_layout);
                new_encode_context.set_channels(av_get_channel_layout_nb_channels(
                    decode_context.channel_layout,
                ));
                new_encode_context.set_sample_fmt(encoder.sample_fmts().unwrap()[0]);
                new_encode_context.set_time_base(ra(1, decode_context.sample_rate));
            } else {
                unreachable!("Shouldn't have decode_context when a codec is non-av!")
            }

            // Some formats want stream headers to be separate.
            if output_format_context.oformat().flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
                new_encode_context
                    .set_flags(new_encode_context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
            }

            new_encode_context.open(None)?;

            let mut out_stream = output_format_context.new_stream();
            out_stream.set_codecpar(new_encode_context.extract_codecpar());
            out_stream.set_time_base(new_encode_context.time_base);

            Some(StreamContext {
                encode_context: new_encode_context,
                decode_context,
                out_stream_index: out_stream.index as usize,
            })
        } else {
            None
        };
        stream_contexts.push(stream_context);
    }

    output_format_context.dump(0, filename)?;
    output_format_context.write_header(dict)?;

    Ok((stream_contexts, output_format_context))
}

/// Init a filter between a `decode_context` and a `encode_context`
/// corresponds to the given `filter_spec`.
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
    let (_inputs, _outputs) = filter_graph.parse_ptr(filter_spec, Some(inputs), Some(outputs))?;

    filter_graph.config()?;

    Ok(FilterContext {
        buffer_src_context,
        buffer_sink_context,
    })
}

/// Create transcoding context corresponding to the given `stream_contexts`, the
/// added filter contexts is mutable reference to objects stored in
/// `filter_graphs`.
fn init_filters<'graph>(
    filter_graphs: &'graph mut [AVFilterGraph],
    stream_contexts: Vec<Option<StreamContext>>,
) -> Result<Vec<Option<TranscodingContext<'graph>>>> {
    let mut filter_contexts = vec![];

    for (filter_graph, stream_context) in filter_graphs.iter_mut().zip(stream_contexts.into_iter())
    {
        let filter_context = if let Some(StreamContext {
            mut decode_context,
            mut encode_context,
            out_stream_index,
        }) = stream_context
        {
            // dummy filter
            let filter_spec = if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
                cstr!("null")
            } else {
                cstr!("anull")
            };

            let FilterContext {
                buffer_src_context,
                buffer_sink_context,
            } = init_filter(
                filter_graph,
                &mut decode_context,
                &mut encode_context,
                filter_spec,
            )?;

            Some(TranscodingContext {
                encode_context,
                decode_context,
                out_stream_index,
                buffer_src_context,
                buffer_sink_context,
            })
        } else {
            None
        };
        filter_contexts.push(filter_context);
    }

    Ok(filter_contexts)
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

/// filter -> encode -> write_frame
fn filter_encode_write_frame(
    frame_before: Option<AVFrame>,
    buffer_src_context: &mut AVFilterContextMut,
    buffer_sink_context: &mut AVFilterContextMut,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    out_stream_index: usize,
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
            out_stream_index,
        )?;
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
pub fn transcoding(
    input_file: &CStr,
    output_file: &CStr,
    dict: &mut Option<AVDictionary>,
) -> Result<()> {
    let (decode_contexts, mut input_format_context) = open_input_file(input_file)?;
    let (stream_contexts, mut output_format_context) =
        open_output_file(output_file, decode_contexts, dict)?;
    let mut filter_graphs: Vec<_> = (0..stream_contexts.len())
        .map(|_| AVFilterGraph::new())
        .collect();
    let mut transcoding_contexts = init_filters(&mut filter_graphs, stream_contexts)?;
    let mut last_timestamp = vec![-1; transcoding_contexts.len()];

    loop {
        let mut packet = match input_format_context.read_packet() {
            Ok(Some(x)) => x,
            // No more frames
            Ok(None) => break,
            Err(e) => bail!("Read frame error: {:?}", e),
        };

        let in_stream_index = packet.stream_index as usize;

        match transcoding_contexts[in_stream_index].as_mut() {
            Some(TranscodingContext {
                decode_context,
                encode_context,
                out_stream_index,
                buffer_src_context,
                buffer_sink_context,
            }) => {
                let input_stream = input_format_context.streams().get(in_stream_index).unwrap();
                packet.rescale_ts(input_stream.time_base, encode_context.time_base);

                decode_context.send_packet(Some(&packet)).unwrap();

                loop {
                    let mut frame = match decode_context.receive_frame() {
                        Ok(frame) => frame,
                        Err(RsmpegError::DecoderDrainError)
                        | Err(RsmpegError::DecoderFlushedError) => break,
                        Err(e) => bail!(e),
                    };

                    let mut best_effort_timestamp = frame.best_effort_timestamp;
                    if best_effort_timestamp == last_timestamp[in_stream_index] {
                        best_effort_timestamp += 1;
                        eprintln!(
                            "fix timestamp: {} -> {}",
                            last_timestamp[in_stream_index], best_effort_timestamp
                        );
                    }
                    last_timestamp[in_stream_index] = best_effort_timestamp;
                    frame.set_pts(best_effort_timestamp);
                    filter_encode_write_frame(
                        Some(frame),
                        buffer_src_context,
                        buffer_sink_context,
                        encode_context,
                        &mut output_format_context,
                        *out_stream_index,
                    )?;
                }
            }
            // Discard non-av video packets.
            None => (),
        }
    }

    // Flush the filter graph by pushing EOF packet to buffer_src_context.
    // Flush the encoder by pushing EOF frame to encode_context.
    for transcoding_context in transcoding_contexts.iter_mut() {
        match transcoding_context {
            Some(TranscodingContext {
                decode_context: _,
                encode_context,
                out_stream_index,
                buffer_src_context,
                buffer_sink_context,
            }) => {
                filter_encode_write_frame(
                    None,
                    buffer_src_context,
                    buffer_sink_context,
                    encode_context,
                    &mut output_format_context,
                    *out_stream_index,
                )?;
                flush_encoder(
                    encode_context,
                    &mut output_format_context,
                    *out_stream_index,
                )?;
            }
            None => (),
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
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcoding_test1() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/centaur.mpg"),
        cstr!("tests/output/transcoding/centaur.mpg"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcoding_test2() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/bear.mp4"),
        cstr!("tests/output/transcoding/bear.mp4"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcoding_test3() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/vp8.mp4"),
        cstr!("tests/output/transcoding/vp8.webm"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcoding_test4() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/big_buck_bunny.mp4"),
        cstr!("tests/output/transcoding/big_buck_bunny.mp4"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcoding_test5() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/with_pic.mp4"),
        cstr!("tests/output/transcoding/with_pic.mp4"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcoding_test6() {
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    transcoding(
        cstr!("tests/assets/vids/screen-fragment.mp4"),
        cstr!("tests/output/transcoding/screen-fragment.mp4"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcoding_test7() {
    // Fragmented MP4 transcoding.
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    let mut dict = Some(AVDictionary::new(
        cstr!("movflags"),
        cstr!("frag_keyframe+empty_moov"),
        0,
    ));

    transcoding(
        cstr!("tests/assets/vids/with_pic.mp4"),
        cstr!("tests/output/transcoding/with_pic_fragmented.mp4"),
        &mut dict,
    )
    .unwrap();

    // Ensure `dict` is consumed.
    assert!(dict.is_none());
}
