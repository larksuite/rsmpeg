use anyhow::{anyhow, bail, Context, Result};
use cstr::cstr;
use rsmpeg::{
    self,
    avcodec::{AVCodec, AVCodecContext},
    avfilter::{AVFilter, AVFilterContextMut, AVFilterGraph, AVFilterInOut},
    avformat::{AVFormatContextInput, AVFormatContextOutput},
    avutil::{
        av_inv_q, av_rescale_q, get_sample_fmt_name, ra, AVChannelLayout, AVDictionary, AVFrame,
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
    buffersrc_ctx: AVFilterContextMut<'graph>,
    buffersink_ctx: AVFilterContextMut<'graph>,
}

struct FilteringContext<'graph> {
    decode_context: AVCodecContext,
    encode_context: AVCodecContext,
    out_stream_index: usize,
    buffersrc_ctx: AVFilterContextMut<'graph>,
    buffersink_ctx: AVFilterContextMut<'graph>,
}

/// Get `decode_contexts`, `input_format_context`, the length of
/// `decode_context` equals to the stream num of the input file. And each decode
/// context corresponds to each stream, if the stream is neither audio nor
/// audio, decode context at this index is set to `None`.
fn open_input_file(filename: &CStr) -> Result<(Vec<Option<AVCodecContext>>, AVFormatContextInput)> {
    let mut input_format_context = AVFormatContextInput::open(filename, None, &mut None)?;
    let mut stream_contexts = Vec::with_capacity(input_format_context.nb_streams as usize);

    for (i, input_stream) in input_format_context.streams().into_iter().enumerate() {
        let codecpar = input_stream.codecpar();
        let codec_type = codecpar.codec_type();
        let decode_context = if codec_type.is_video() || codec_type.is_audio() {
            let decoder = AVCodec::find_decoder(codecpar.codec_id)
                .with_context(|| anyhow!("Failed to find decoder for stream #{}", i))?;

            let mut decode_context = AVCodecContext::new(&decoder);
            decode_context.apply_codecpar(&codecpar).with_context(|| {
                anyhow!(
                    "Failed to copy decoder parameters to input decoder context for stream #{}",
                    i
                )
            })?;
            decode_context.set_pkt_timebase(input_stream.time_base);
            if codec_type.is_video() {
                if let Some(framerate) = input_stream.guess_framerate() {
                    decode_context.set_framerate(framerate);
                }
            }
            decode_context
                .open(None)
                .with_context(|| anyhow!("Failed to open decoder for stream #{}", i))?;
            Some(decode_context)
        } else {
            None
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

    for (i, decode_context) in decode_contexts.into_iter().enumerate() {
        let Some(decode_context) = decode_context else {
            stream_contexts.push(None);
            continue;
        };
        let encoder = AVCodec::find_encoder(decode_context.codec_id)
            .with_context(|| anyhow!("encoder({}) not found.", decode_context.codec_id))?;

        let mut new_encode_context = AVCodecContext::new(&encoder);

        if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            new_encode_context.set_height(decode_context.height);
            new_encode_context.set_width(decode_context.width);
            new_encode_context.set_sample_aspect_ratio(decode_context.sample_aspect_ratio);
            // take first format from list of supported formats
            new_encode_context.set_pix_fmt(encoder.pix_fmts().unwrap()[0]);
            new_encode_context.set_time_base(av_inv_q(decode_context.framerate));
        } else if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO {
            new_encode_context.set_sample_rate(decode_context.sample_rate);
            new_encode_context.set_ch_layout(decode_context.ch_layout().clone().into_inner());
            // take first format from list of supported formats
            new_encode_context.set_sample_fmt(encoder.sample_fmts().unwrap()[0]);
            new_encode_context.set_time_base(ra(1, decode_context.sample_rate));
        } else {
            bail!(
                "Elementary stream #{} is of unknown type, cannot proceed",
                i
            );
        }

        // Some formats want stream headers to be separate.
        if output_format_context.oformat().flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
            new_encode_context
                .set_flags(new_encode_context.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
        }

        new_encode_context.open(None).with_context(|| {
            anyhow!(
                "Cannot open {} encoder for stream #{}",
                encoder.name().to_str().unwrap(),
                i
            )
        })?;

        let mut out_stream = output_format_context.new_stream();
        out_stream.set_codecpar(new_encode_context.extract_codecpar());
        out_stream.set_time_base(new_encode_context.time_base);

        stream_contexts.push(Some(StreamContext {
            encode_context: new_encode_context,
            decode_context,
            out_stream_index: out_stream.index as usize,
        }));
    }

    output_format_context.dump(0, filename)?;
    output_format_context
        .write_header(dict)
        .context("Error occurred when opening output file")?;

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
    let (mut buffersrc_ctx, mut buffersink_ctx) =
        if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            let buffersrc = AVFilter::get_by_name(cstr!("buffer")).unwrap();
            let buffersink = AVFilter::get_by_name(cstr!("buffersink")).unwrap();

            let args = format!(
                "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
                decode_context.width,
                decode_context.height,
                decode_context.pix_fmt,
                decode_context.pkt_timebase.num,
                decode_context.pkt_timebase.den,
                decode_context.sample_aspect_ratio.num,
                decode_context.sample_aspect_ratio.den,
            );

            let args = &CString::new(args).unwrap();

            let buffer_src_context = filter_graph
                .create_filter_context(&buffersrc, cstr!("in"), Some(args))
                .context("Cannot create buffer source")?;

            let mut buffer_sink_context = filter_graph
                .create_filter_context(&buffersink, cstr!("out"), None)
                .context("Cannot create buffer sink")?;

            buffer_sink_context
                .opt_set_bin(cstr!("pix_fmts"), &encode_context.pix_fmt)
                .context("Cannot set output pixel format")?;

            (buffer_src_context, buffer_sink_context)
        } else if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO {
            let buffersrc = AVFilter::get_by_name(cstr!("abuffer")).unwrap();
            let buffersink = AVFilter::get_by_name(cstr!("abuffersink")).unwrap();

            if decode_context.ch_layout.order == ffi::AVChannelOrder_AV_CHANNEL_ORDER_UNSPEC {
                decode_context.set_ch_layout(
                    AVChannelLayout::from_nb_channels(decode_context.ch_layout.nb_channels)
                        .into_inner(),
                );
            }

            let args = format!(
                "time_base={}/{}:sample_rate={}:sample_fmt={}:channel_layout={}",
                decode_context.pkt_timebase.num,
                decode_context.pkt_timebase.den,
                decode_context.sample_rate,
                // We can unwrap here, because we are sure that the given
                // sample_fmt is valid.
                get_sample_fmt_name(decode_context.sample_fmt)
                    .unwrap()
                    .to_string_lossy(),
                decode_context
                    .ch_layout()
                    .describe()
                    .unwrap()
                    .to_string_lossy(),
            );
            let args = &CString::new(args).unwrap();

            let buffersrc_ctx = filter_graph
                .create_filter_context(&buffersrc, cstr!("in"), Some(args))
                .context("Cannot create audio buffer source")?;

            let mut buffersink_ctx = filter_graph
                .create_filter_context(&buffersink, cstr!("out"), None)
                .context("Cannot create audio buffer sink")?;
            buffersink_ctx
                .opt_set_bin(cstr!("sample_fmts"), &encode_context.sample_fmt)
                .context("Cannot set output sample format")?;
            buffersink_ctx
                .opt_set(
                    cstr!("ch_layouts"),
                    &encode_context.ch_layout().describe().unwrap(),
                )
                .context("Cannot set output channel layout")?;
            buffersink_ctx
                .opt_set_bin(cstr!("sample_rates"), &encode_context.sample_rate)
                .context("Cannot set output sample rate")?;

            (buffersrc_ctx, buffersink_ctx)
        } else {
            bail!("Only video and audio needs filter initialization")
        };

    // Endpoints for the filter graph
    //
    // Yes the outputs' name is `in` -_-b
    let outputs = AVFilterInOut::new(cstr!("in"), &mut buffersrc_ctx, 0);
    let inputs = AVFilterInOut::new(cstr!("out"), &mut buffersink_ctx, 0);

    let (_inputs, _outputs) = filter_graph.parse_ptr(filter_spec, Some(inputs), Some(outputs))?;

    filter_graph.config()?;

    Ok(FilterContext {
        buffersrc_ctx,
        buffersink_ctx,
    })
}

/// Create transcoding context corresponding to the given `stream_contexts`, the
/// added filter contexts is mutable reference to objects stored in
/// `filter_graphs`.
fn init_filters(
    filter_graphs: &mut [AVFilterGraph],
    stream_contexts: Vec<Option<StreamContext>>,
) -> Result<Vec<Option<FilteringContext>>> {
    let mut filter_ctx = Vec::with_capacity(stream_contexts.len());

    for (filter_graph, stream_context) in filter_graphs.iter_mut().zip(stream_contexts.into_iter())
    {
        let Some(stream_context) = stream_context else {
            filter_ctx.push(None);
            continue;
        };

        let StreamContext {
            mut decode_context,
            mut encode_context,
            out_stream_index,
        } = stream_context;

        // dummy filter
        let filter_spec = if decode_context.codec_type == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            cstr!("null")
        } else {
            cstr!("anull")
        };

        let FilterContext {
            buffersrc_ctx,
            buffersink_ctx,
        } = init_filter(
            filter_graph,
            &mut decode_context,
            &mut encode_context,
            filter_spec,
        )?;

        filter_ctx.push(Some(FilteringContext {
            encode_context,
            decode_context,
            out_stream_index,
            buffersrc_ctx,
            buffersink_ctx,
        }));
    }

    Ok(filter_ctx)
}

/// encode -> write_frame
fn encode_write_frame(
    mut filt_frame: Option<AVFrame>,
    enc_ctx: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    stream_index: usize,
) -> Result<()> {
    if let Some(filt_frame) = filt_frame.as_mut() {
        if filt_frame.pts != ffi::AV_NOPTS_VALUE {
            filt_frame.set_pts(av_rescale_q(
                filt_frame.pts,
                filt_frame.time_base,
                enc_ctx.time_base,
            ));
        }
    }

    enc_ctx
        .send_frame(filt_frame.as_ref())
        .context("Encode frame failed.")?;

    loop {
        let mut enc_pkt = match enc_ctx.receive_packet() {
            Ok(packet) => packet,
            Err(RsmpegError::EncoderDrainError) | Err(RsmpegError::EncoderFlushedError) => break,
            Err(e) => bail!(e),
        };

        enc_pkt.set_stream_index(stream_index as i32);
        enc_pkt.rescale_ts(
            enc_ctx.time_base,
            output_format_context.streams()[stream_index].time_base,
        );

        match output_format_context.interleaved_write_frame(&mut enc_pkt) {
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
    frame: Option<AVFrame>,
    buffersrc_ctx: &mut AVFilterContextMut,
    buffersink_ctx: &mut AVFilterContextMut,
    encode_context: &mut AVCodecContext,
    output_format_context: &mut AVFormatContextOutput,
    out_stream_index: usize,
) -> Result<()> {
    buffersrc_ctx
        .buffersrc_add_frame(frame, None)
        .context("Error submitting the frame to the filtergraph:")?;
    loop {
        let mut filtered_frame = match buffersink_ctx.buffersink_get_frame(None) {
            Ok(frame) => frame,
            Err(RsmpegError::BufferSinkDrainError) | Err(RsmpegError::BufferSinkEofError) => break,
            Err(_) => bail!("Get frame from buffer sink failed."),
        };

        filtered_frame.set_time_base(buffersink_ctx.get_time_base());
        filtered_frame.set_pict_type(ffi::AVPictureType_AV_PICTURE_TYPE_NONE);

        encode_write_frame(
            Some(filtered_frame),
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
        let packet = match input_format_context.read_packet() {
            Ok(Some(x)) => x,
            // No more frames
            Ok(None) => break,
            Err(e) => bail!("Read frame error: {:?}", e),
        };

        let in_stream_index = packet.stream_index as usize;

        if let Some(FilteringContext {
            decode_context,
            encode_context,
            out_stream_index,
            buffersrc_ctx,
            buffersink_ctx,
        }) = transcoding_contexts[in_stream_index].as_mut()
        {
            decode_context.send_packet(Some(&packet)).unwrap();

            loop {
                let mut frame = match decode_context.receive_frame() {
                    Ok(frame) => frame,
                    Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => {
                        break
                    }
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
                    buffersrc_ctx,
                    buffersink_ctx,
                    encode_context,
                    &mut output_format_context,
                    *out_stream_index,
                )?;
            }
        }
    }

    // Flush the filter graph by pushing EOF packet to buffer_src_context.
    // Flush the encoder by pushing EOF frame to encode_context.
    for transcoding_context in transcoding_contexts.iter_mut() {
        match transcoding_context {
            Some(FilteringContext {
                decode_context: _,
                encode_context,
                out_stream_index,
                buffersrc_ctx: buffer_src_context,
                buffersink_ctx: buffer_sink_context,
            }) => {
                filter_encode_write_frame(
                    None,
                    buffer_src_context,
                    buffer_sink_context,
                    encode_context,
                    &mut output_format_context,
                    *out_stream_index,
                )
                .context("Flushing filter failed")?;
                flush_encoder(
                    encode_context,
                    &mut output_format_context,
                    *out_stream_index,
                )
                .context("Flushing encoder failed")?;
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
    // Fragmented MP4 transcoding.
    std::fs::create_dir_all("tests/output/transcoding/").unwrap();
    let mut dict = Some(AVDictionary::new(
        cstr!("movflags"),
        cstr!("frag_keyframe+empty_moov"),
        0,
    ));

    transcoding(
        cstr!("tests/assets/vids/big_buck_bunny.mp4"),
        cstr!("tests/output/transcoding/big_buck_bunny.fmp4.mp4"),
        &mut dict,
    )
    .unwrap();

    // Ensure `dict` is consumed.
    assert!(dict.is_none());
}
