//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/decode_filter_audio.c
use anyhow::{anyhow, Context, Result};
use rsmpeg::{
    avcodec::AVCodecContext,
    avfilter::{AVFilter, AVFilterGraph, AVFilterInOut},
    avformat::AVFormatContextInput,
    avutil::{get_sample_fmt_name, AVChannelLayout, AVFrame},
    ffi,
};
use std::{ffi::CStr, fs::File, io::Write, path::Path};

static FILTER_DESCR: &CStr = c"aresample=8000,aformat=sample_fmts=s16:channel_layouts=mono";

struct FilterState {
    graph: AVFilterGraph,
}

fn open_input_file(filename: &CStr) -> Result<(AVFormatContextInput, AVCodecContext, usize)> {
    let fmt = AVFormatContextInput::open(filename).context("Cannot open input file")?;

    let (audio_idx, dec) = fmt
        .find_best_stream(ffi::AVMEDIA_TYPE_AUDIO)
        .context("Cannot find an audio stream in the input file")?
        .ok_or_else(|| anyhow!("Cannot find an audio stream in the input file"))?;

    let mut ctx = AVCodecContext::new(&dec);
    ctx.apply_codecpar(&fmt.streams()[audio_idx].codecpar())
        .context("Failed to copy codec parameters")?;
    ctx.open(None).context("Cannot open audio decoder")?;
    Ok((fmt, ctx, audio_idx))
}

fn init_filters(
    fmt: &AVFormatContextInput,
    dec_ctx: &mut AVCodecContext,
    audio_stream_index: usize,
    filters_descr: &CStr,
) -> Result<FilterState> {
    let graph = AVFilterGraph::new();
    let abuffer = AVFilter::get_by_name(c"abuffer").context("Cannot find abuffer")?;
    let abuffersink = AVFilter::get_by_name(c"abuffersink").context("Cannot find abuffersink")?;

    // Build args like C: time_base=..:sample_rate=..:sample_fmt=..:channel_layout=..
    let tb = fmt.streams()[audio_stream_index].time_base;
    if dec_ctx.ch_layout.order == ffi::AV_CHANNEL_ORDER_UNSPEC {
        // default by channel count if unspecified
        dec_ctx.set_ch_layout(
            AVChannelLayout::from_nb_channels(dec_ctx.ch_layout.nb_channels).into_inner(),
        );
    }
    let sample_fmt_name = get_sample_fmt_name(dec_ctx.sample_fmt)
        .and_then(|s| s.to_str().ok())
        .unwrap_or("");

    // Prepare channel_layout description (safe wrapper)
    let ch_layout_str = dec_ctx
        .ch_layout()
        .describe()
        .ok()
        .and_then(|s| s.into_string().ok())
        .unwrap_or_else(|| "?".to_string());

    let args = format!(
        "time_base={}/{}:sample_rate={}:sample_fmt={}:channel_layout={}",
        tb.num,
        tb.den,
        dec_ctx.sample_rate,
        if sample_fmt_name.is_empty() {
            "fltp"
        } else {
            sample_fmt_name
        },
        ch_layout_str
    );

    // Create source and sink filters
    {
        let args_c = std::ffi::CString::new(args).unwrap();
        let mut buffersrc_ctx = graph
            .create_filter_context(&abuffer, c"in", Some(&args_c))
            .context("Cannot create audio buffer source")?;
        let mut buffersink_ctx = graph
            .alloc_filter_context(&abuffersink, c"out")
            .ok_or_else(|| anyhow!("Cannot create audio buffer sink"))?;

        buffersink_ctx
            .opt_set(c"sample_formats", c"s16")
            .context("Cannot set output sample format")?;
        buffersink_ctx
            .opt_set(c"channel_layouts", c"mono")
            .context("Cannot set output channel layout")?;
        buffersink_ctx
            .opt_set_array(c"samplerates", 0, Some(&[8000]), ffi::AV_OPT_TYPE_INT)
            .context("Cannot set output sample rate")?;
        buffersink_ctx
            .init_str(None)
            .context("Cannot initialize audio buffer sink")?;

        // Link endpoints through the filter graph described by filters_descr
        let outputs = AVFilterInOut::new(c"in", &mut buffersrc_ctx, 0);
        let inputs = AVFilterInOut::new(c"out", &mut buffersink_ctx, 0);
        graph
            .parse_ptr(filters_descr, Some(inputs), Some(outputs))
            .context("avfilter_graph_parse_ptr failed")?;
        graph.config().context("avfilter_graph_config failed")?;

        let out_srate = buffersink_ctx.get_sample_rate();
        let out_fmt = get_sample_fmt_name(buffersink_ctx.get_format())
            .and_then(|s| s.to_str().ok())
            .unwrap_or("?");
        let ch_layout = buffersink_ctx.get_ch_layout();
        let ch_desc = ch_layout
            .describe()
            .ok()
            .and_then(|s| s.into_string().ok())
            .unwrap_or_else(|| "?".to_string());
        eprintln!(
            "Output: srate:{}Hz fmt:{} chlayout:{}",
            out_srate, out_fmt, ch_desc
        );
    }

    Ok(FilterState { graph })
}

fn print_and_write_frame(mut out: &File, frame: &AVFrame) -> Result<()> {
    // s16le interleaved mono: dump to file
    let samples = (frame.nb_samples * frame.ch_layout.nb_channels) as usize;
    unsafe {
        let p = frame.data[0] as *const u8;
        let bytes = std::slice::from_raw_parts(p, samples * 2);
        out.write_all(bytes)?;
    }
    Ok(())
}

fn decode_filter_audio(input: &CStr, out_path: &str) -> Result<()> {
    let (mut fmt, mut dec_ctx, audio_idx) = open_input_file(input)?;
    let filt = init_filters(&fmt, &mut dec_ctx, audio_idx, FILTER_DESCR)?;

    if let Some(dir) = Path::new(out_path).parent() {
        std::fs::create_dir_all(dir).ok();
    }
    let out = File::create(out_path).with_context(|| format!("open {}", out_path))?;

    let mut src = filt
        .graph
        .get_filter(c"in")
        .context("buffersrc not found")?;
    let mut sink = filt
        .graph
        .get_filter(c"out")
        .context("buffersink not found")?;

    while let Some(packet) = fmt.read_packet()? {
        if packet.stream_index == audio_idx as i32 {
            dec_ctx
                .send_packet(Some(&packet))
                .context("Error while sending a packet to the decoder")?;
            loop {
                match dec_ctx.receive_frame() {
                    Ok(frame) => {
                        // push decoded frame into graph
                        {
                            src.buffersrc_add_frame(
                                Some(frame),
                                Some(ffi::AV_BUFFERSRC_FLAG_KEEP_REF as i32),
                            )
                            .context("Error while feeding the audio filtergraph")?;
                        }
                        // pull all available filtered frames
                        loop {
                            match sink.buffersink_get_frame(None) {
                                Ok(f) => {
                                    print_and_write_frame(&out, &f)?;
                                }
                                Err(rsmpeg::error::RsmpegError::BufferSinkDrainError)
                                | Err(rsmpeg::error::RsmpegError::BufferSinkEofError) => break,
                                Err(e) => return Err(e).context("buffersink_get_frame failed"),
                            }
                        }
                    }
                    Err(rsmpeg::error::RsmpegError::DecoderDrainError)
                    | Err(rsmpeg::error::RsmpegError::DecoderFlushedError) => break,
                    Err(e) => {
                        return Err(e).context("Error while receiving a frame from the decoder")
                    }
                }
            }
        }
    }
    // EOF: signal to the filter graph
    {
        let mut src = filt
            .graph
            .get_filter(c"in")
            .ok_or_else(|| anyhow!("buffersrc not found"))?;
        src.buffersrc_add_frame(None, None)
            .context("Error while closing the filtergraph")?;
    }
    loop {
        let mut sink = filt
            .graph
            .get_filter(c"out")
            .ok_or_else(|| anyhow!("buffersink not found"))?;
        match sink.buffersink_get_frame(None) {
            Ok(f) => {
                print_and_write_frame(&out, &f)?;
            }
            Err(rsmpeg::error::RsmpegError::BufferSinkDrainError)
            | Err(rsmpeg::error::RsmpegError::BufferSinkEofError) => break,
            Err(e) => return Err(e).context("buffersink_get_frame failed on flush"),
        }
    }

    Ok(())
}

#[test]
fn decode_filter_audio_test() {
    let input = c"tests/assets/audios/sample1_short.aac";
    let out = "tests/output/decode_filter_audio/out_s16le_8k_mono.pcm";
    decode_filter_audio(input, out).unwrap();
}
