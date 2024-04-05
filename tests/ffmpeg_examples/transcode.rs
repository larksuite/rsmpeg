//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/transcode.c
use anyhow::{anyhow, bail, Context, Result};
use cstr::cstr;
use rsmpeg::{
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

struct FilteringContext<'graph> {
    dec_ctx: AVCodecContext,
    enc_ctx: AVCodecContext,
    stream_index: usize,
    buffersrc_ctx: AVFilterContextMut<'graph>,
    buffersink_ctx: AVFilterContextMut<'graph>,
}

struct StreamContext {
    dec_ctx: AVCodecContext,
    enc_ctx: AVCodecContext,
    stream_index: usize,
}

struct FilterContext<'graph> {
    buffersrc_ctx: AVFilterContextMut<'graph>,
    buffersink_ctx: AVFilterContextMut<'graph>,
}

/// Get `decode_contexts`, `input_format_context`, the length of
/// `decode_context` equals to the stream num of the input file. And each decode
/// context corresponds to each stream, if the stream is neither audio nor
/// audio, decode context at this index is set to `None`.
fn open_input_file(filename: &CStr) -> Result<(Vec<Option<AVCodecContext>>, AVFormatContextInput)> {
    let mut ifmt_ctx = AVFormatContextInput::open(filename, None, &mut None)?;
    let mut stream_ctx = Vec::with_capacity(ifmt_ctx.nb_streams as usize);

    for (i, input_stream) in ifmt_ctx.streams().into_iter().enumerate() {
        let codecpar = input_stream.codecpar();
        let codec_type = codecpar.codec_type();
        let dec_ctx = if codec_type.is_video() || codec_type.is_audio() {
            let decoder = AVCodec::find_decoder(codecpar.codec_id)
                .with_context(|| anyhow!("Failed to find decoder for stream #{}", i))?;

            let mut dec_ctx = AVCodecContext::new(&decoder);
            dec_ctx.apply_codecpar(&codecpar).with_context(|| {
                anyhow!(
                    "Failed to copy decoder parameters to input decoder context for stream #{}",
                    i
                )
            })?;
            dec_ctx.set_pkt_timebase(input_stream.time_base);
            if codec_type.is_video() {
                if let Some(framerate) = input_stream.guess_framerate() {
                    dec_ctx.set_framerate(framerate);
                }
            }
            dec_ctx
                .open(None)
                .with_context(|| anyhow!("Failed to open decoder for stream #{}", i))?;
            Some(dec_ctx)
        } else {
            None
        };

        stream_ctx.push(dec_ctx);
    }
    ifmt_ctx.dump(0, filename)?;
    Ok((stream_ctx, ifmt_ctx))
}

/// Accepts a output filename, attach `encode_context` to the corresponding
/// `decode_context` and wrap them into a `stream_context`. `stream_context` is
/// None when the given `decode_context` in the same index is None.
fn open_output_file(
    filename: &CStr,
    dec_ctx: Vec<Option<AVCodecContext>>,
    dict: &mut Option<AVDictionary>,
) -> Result<(Vec<Option<StreamContext>>, AVFormatContextOutput)> {
    let mut ofmt_ctx = AVFormatContextOutput::create(filename, None)?;
    let mut stream_ctx = vec![];

    for (i, dec_ctx) in dec_ctx.into_iter().enumerate() {
        let Some(dec_ctx) = dec_ctx else {
            stream_ctx.push(None);
            continue;
        };
        let encoder = AVCodec::find_encoder(dec_ctx.codec_id)
            .with_context(|| anyhow!("encoder({}) not found.", dec_ctx.codec_id))?;

        let mut enc_ctx = AVCodecContext::new(&encoder);

        if dec_ctx.codec_type == ffi::AVMEDIA_TYPE_VIDEO {
            enc_ctx.set_height(dec_ctx.height);
            enc_ctx.set_width(dec_ctx.width);
            enc_ctx.set_sample_aspect_ratio(dec_ctx.sample_aspect_ratio);
            // take first format from list of supported formats
            enc_ctx.set_pix_fmt(encoder.pix_fmts().unwrap()[0]);
            enc_ctx.set_time_base(av_inv_q(dec_ctx.framerate));
        } else if dec_ctx.codec_type == ffi::AVMEDIA_TYPE_AUDIO {
            enc_ctx.set_sample_rate(dec_ctx.sample_rate);
            enc_ctx.set_ch_layout(dec_ctx.ch_layout().clone().into_inner());
            // take first format from list of supported formats
            enc_ctx.set_sample_fmt(encoder.sample_fmts().unwrap()[0]);
            enc_ctx.set_time_base(ra(1, dec_ctx.sample_rate));
        } else {
            bail!(
                "Elementary stream #{} is of unknown type, cannot proceed",
                i
            );
        }

        // Some formats want stream headers to be separate.
        if ofmt_ctx.oformat().flags & ffi::AVFMT_GLOBALHEADER as i32 != 0 {
            enc_ctx.set_flags(enc_ctx.flags | ffi::AV_CODEC_FLAG_GLOBAL_HEADER as i32);
        }

        enc_ctx.open(None).with_context(|| {
            anyhow!(
                "Cannot open {} encoder for stream #{}",
                encoder.name().to_str().unwrap(),
                i
            )
        })?;

        let mut out_stream = ofmt_ctx.new_stream();
        out_stream.set_codecpar(enc_ctx.extract_codecpar());
        out_stream.set_time_base(enc_ctx.time_base);

        stream_ctx.push(Some(StreamContext {
            enc_ctx,
            dec_ctx,
            stream_index: out_stream.index as usize,
        }));
    }

    ofmt_ctx.dump(0, filename)?;
    ofmt_ctx
        .write_header(dict)
        .context("Error occurred when opening output file")?;

    Ok((stream_ctx, ofmt_ctx))
}

/// Init a filter between a `decode_context` and a `encode_context`
/// corresponds to the given `filter_spec`.
fn init_filter<'graph>(
    filter_graph: &'graph mut AVFilterGraph,
    dec_ctx: &mut AVCodecContext,
    enc_ctx: &mut AVCodecContext,
    filter_spec: &CStr,
) -> Result<FilterContext<'graph>> {
    let (mut buffersrc_ctx, mut buffersink_ctx) = if dec_ctx.codec_type == ffi::AVMEDIA_TYPE_VIDEO {
        let buffersrc = AVFilter::get_by_name(cstr!("buffer")).unwrap();
        let buffersink = AVFilter::get_by_name(cstr!("buffersink")).unwrap();

        let args = format!(
            "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
            dec_ctx.width,
            dec_ctx.height,
            dec_ctx.pix_fmt,
            dec_ctx.pkt_timebase.num,
            dec_ctx.pkt_timebase.den,
            dec_ctx.sample_aspect_ratio.num,
            dec_ctx.sample_aspect_ratio.den,
        );

        let args = &CString::new(args).unwrap();

        let buffer_src_context = filter_graph
            .create_filter_context(&buffersrc, cstr!("in"), Some(args))
            .context("Cannot create buffer source")?;

        let mut buffer_sink_context = filter_graph
            .create_filter_context(&buffersink, cstr!("out"), None)
            .context("Cannot create buffer sink")?;

        buffer_sink_context
            .opt_set_bin(cstr!("pix_fmts"), &enc_ctx.pix_fmt)
            .context("Cannot set output pixel format")?;

        (buffer_src_context, buffer_sink_context)
    } else if dec_ctx.codec_type == ffi::AVMEDIA_TYPE_AUDIO {
        let buffersrc = AVFilter::get_by_name(cstr!("abuffer")).unwrap();
        let buffersink = AVFilter::get_by_name(cstr!("abuffersink")).unwrap();

        if dec_ctx.ch_layout.order == ffi::AV_CHANNEL_ORDER_UNSPEC {
            dec_ctx.set_ch_layout(
                AVChannelLayout::from_nb_channels(dec_ctx.ch_layout.nb_channels).into_inner(),
            );
        }

        let args = format!(
            "time_base={}/{}:sample_rate={}:sample_fmt={}:channel_layout={}",
            dec_ctx.pkt_timebase.num,
            dec_ctx.pkt_timebase.den,
            dec_ctx.sample_rate,
            // We can unwrap here, because we are sure that the given
            // sample_fmt is valid.
            get_sample_fmt_name(dec_ctx.sample_fmt)
                .unwrap()
                .to_string_lossy(),
            dec_ctx.ch_layout().describe().unwrap().to_string_lossy(),
        );
        let args = &CString::new(args).unwrap();

        let buffersrc_ctx = filter_graph
            .create_filter_context(&buffersrc, cstr!("in"), Some(args))
            .context("Cannot create audio buffer source")?;

        let mut buffersink_ctx = filter_graph
            .create_filter_context(&buffersink, cstr!("out"), None)
            .context("Cannot create audio buffer sink")?;
        buffersink_ctx
            .opt_set_bin(cstr!("sample_fmts"), &enc_ctx.sample_fmt)
            .context("Cannot set output sample format")?;
        buffersink_ctx
            .opt_set(
                cstr!("ch_layouts"),
                &enc_ctx.ch_layout().describe().unwrap(),
            )
            .context("Cannot set output channel layout")?;
        buffersink_ctx
            .opt_set_bin(cstr!("sample_rates"), &enc_ctx.sample_rate)
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
            mut dec_ctx,
            mut enc_ctx,
            stream_index,
        } = stream_context;

        // dummy filter
        let filter_spec = if dec_ctx.codec_type == ffi::AVMEDIA_TYPE_VIDEO {
            cstr!("null")
        } else {
            cstr!("anull")
        };

        let FilterContext {
            buffersrc_ctx,
            buffersink_ctx,
        } = init_filter(filter_graph, &mut dec_ctx, &mut enc_ctx, filter_spec)?;

        filter_ctx.push(Some(FilteringContext {
            enc_ctx,
            dec_ctx,
            stream_index,
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
    ofmt_ctx: &mut AVFormatContextOutput,
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
            ofmt_ctx.streams()[stream_index].time_base,
        );

        ofmt_ctx
            .interleaved_write_frame(&mut enc_pkt)
            .context("Interleaved write frame failed.")?;
    }

    Ok(())
}

/// filter -> encode -> write_frame
fn filter_encode_write_frame(
    frame: Option<AVFrame>,
    buffersrc_ctx: &mut AVFilterContextMut,
    buffersink_ctx: &mut AVFilterContextMut,
    enc_ctx: &mut AVCodecContext,
    ofmt_ctx: &mut AVFormatContextOutput,
    stream_index: usize,
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
        filtered_frame.set_pict_type(ffi::AV_PICTURE_TYPE_NONE);

        encode_write_frame(Some(filtered_frame), enc_ctx, ofmt_ctx, stream_index)?;
    }
    Ok(())
}

/// Send an empty packet to the `encode_context` for packet flushing.
fn flush_encoder(
    enc_ctx: &mut AVCodecContext,
    ofmt_ctx: &mut AVFormatContextOutput,
    stream_index: usize,
) -> Result<()> {
    if enc_ctx.codec().capabilities & ffi::AV_CODEC_CAP_DELAY as i32 == 0 {
        return Ok(());
    }
    encode_write_frame(None, enc_ctx, ofmt_ctx, stream_index)?;
    Ok(())
}

/// Transcoding audio and video stream in a multi media file.
pub fn transcode(
    input_file: &CStr,
    output_file: &CStr,
    dict: &mut Option<AVDictionary>,
) -> Result<()> {
    let (dec_ctx, mut ifmt_ctx) = open_input_file(input_file)?;
    let (stream_ctx, mut ofmt_ctx) = open_output_file(output_file, dec_ctx, dict)?;
    let mut filter_graphs: Vec<_> = (0..stream_ctx.len())
        .map(|_| AVFilterGraph::new())
        .collect();
    let mut filter_ctx = init_filters(&mut filter_graphs, stream_ctx)?;

    loop {
        let packet = match ifmt_ctx.read_packet() {
            Ok(Some(x)) => x,
            // No more frames
            Ok(None) => break,
            Err(e) => bail!("Read frame error: {:?}", e),
        };

        let in_stream_index = packet.stream_index as usize;

        if let Some(FilteringContext {
            dec_ctx: decode_context,
            enc_ctx: encode_context,
            stream_index,
            buffersrc_ctx,
            buffersink_ctx,
        }) = filter_ctx[in_stream_index].as_mut()
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

                frame.set_pts(frame.best_effort_timestamp);
                filter_encode_write_frame(
                    Some(frame),
                    buffersrc_ctx,
                    buffersink_ctx,
                    encode_context,
                    &mut ofmt_ctx,
                    *stream_index,
                )?;
            }
        }
    }

    // Flush the filter graph by pushing EOF packet to buffer_src_context.
    // Flush the encoder by pushing EOF frame to encode_context.
    for filter_ctx in filter_ctx.iter_mut() {
        match filter_ctx {
            Some(FilteringContext {
                dec_ctx: _,
                enc_ctx,
                stream_index,
                buffersrc_ctx,
                buffersink_ctx,
            }) => {
                filter_encode_write_frame(
                    None,
                    buffersrc_ctx,
                    buffersink_ctx,
                    enc_ctx,
                    &mut ofmt_ctx,
                    *stream_index,
                )
                .context("Flushing filter failed")?;
                flush_encoder(enc_ctx, &mut ofmt_ctx, *stream_index)
                    .context("Flushing encoder failed")?;
            }
            None => (),
        }
    }
    ofmt_ctx.write_trailer()?;
    Ok(())
}

#[test]
fn transcode_test0() {
    std::fs::create_dir_all("tests/output/transcode/").unwrap();
    transcode(
        cstr!("tests/assets/vids/mov_sample.mov"),
        cstr!("tests/output/transcode/mov_sample.mov"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcode_test1() {
    std::fs::create_dir_all("tests/output/transcode/").unwrap();
    transcode(
        cstr!("tests/assets/vids/centaur.mpg"),
        cstr!("tests/output/transcode/centaur.mpg"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcode_test2() {
    std::fs::create_dir_all("tests/output/transcode/").unwrap();
    transcode(
        cstr!("tests/assets/vids/bear.mp4"),
        cstr!("tests/output/transcode/bear.mp4"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcode_test3() {
    std::fs::create_dir_all("tests/output/transcode/").unwrap();
    transcode(
        cstr!("tests/assets/vids/vp8.mp4"),
        cstr!("tests/output/transcode/vp8.webm"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcode_test4() {
    std::fs::create_dir_all("tests/output/transcode/").unwrap();
    transcode(
        cstr!("tests/assets/vids/big_buck_bunny.mp4"),
        cstr!("tests/output/transcode/big_buck_bunny.mp4"),
        &mut None,
    )
    .unwrap();
}

#[test]
fn transcode_test5() {
    // Fragmented MP4 transcode.
    std::fs::create_dir_all("tests/output/transcode/").unwrap();
    let mut dict = Some(AVDictionary::new(
        cstr!("movflags"),
        cstr!("frag_keyframe+empty_moov"),
        0,
    ));

    transcode(
        cstr!("tests/assets/vids/big_buck_bunny.mp4"),
        cstr!("tests/output/transcode/big_buck_bunny.fmp4.mp4"),
        &mut dict,
    )
    .unwrap();

    // Ensure `dict` is consumed.
    assert!(dict.is_none());
}
