//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/decode_filter_video.c
use anyhow::{anyhow, Context, Result};
use rsmpeg::{
    avcodec::AVCodecContext,
    avfilter::{AVFilter, AVFilterGraph, AVFilterInOut},
    avformat::AVFormatContextInput,
    ffi,
};
use std::{ffi::CStr, io::Write};

static FILTER_DESCR: &CStr = c"scale=78:24,transpose=cclock";

struct FilterState {
    graph: AVFilterGraph,
}

fn open_input_file(filename: &CStr) -> Result<(AVFormatContextInput, AVCodecContext, usize)> {
    let fmt = AVFormatContextInput::open(filename).context("Cannot open input file")?;

    let (video_idx, dec) = fmt
        .find_best_stream(ffi::AVMEDIA_TYPE_VIDEO)
        .context("Cannot find a video stream in the input file")?
        .ok_or_else(|| anyhow!("Cannot find a video stream in the input file"))?;

    let mut ctx = AVCodecContext::new(&dec);
    ctx.apply_codecpar(&fmt.streams()[video_idx].codecpar())
        .context("Failed to copy codec parameters")?;
    ctx.open(None).context("Cannot open video decoder")?;
    Ok((fmt, ctx, video_idx))
}

fn init_filters(
    fmt: &AVFormatContextInput,
    dec_ctx: &AVCodecContext,
    video_stream_index: usize,
    filters_descr: &CStr,
) -> Result<FilterState> {
    let graph = AVFilterGraph::new();
    let buffer = AVFilter::get_by_name(c"buffer").context("Cannot find buffer")?;
    let buffersink = AVFilter::get_by_name(c"buffersink").context("Cannot find buffersink")?;

    let tb = fmt.streams()[video_stream_index].time_base;
    let args = format!(
        "video_size={}x{}:pix_fmt={}:time_base={}/{}:pixel_aspect={}/{}",
        dec_ctx.width,
        dec_ctx.height,
        dec_ctx.pix_fmt,
        tb.num,
        tb.den,
        dec_ctx.sample_aspect_ratio.num,
        dec_ctx.sample_aspect_ratio.den
    );
    let args_c = std::ffi::CString::new(args).unwrap();

    // Create endpoints and link through the filter graph
    {
        let mut buffersrc_ctx = graph
            .create_filter_context(&buffer, c"in", Some(&args_c))
            .context("Cannot create buffer source")?;
        let mut buffersink_ctx = graph
            .alloc_filter_context(&buffersink, c"out")
            .ok_or_else(|| anyhow!("Cannot create buffer sink"))?;
        buffersink_ctx
            .opt_set(c"pixel_formats", c"gray8")
            .context("Cannot set output pixel format")?;
        buffersink_ctx
            .init_str(None)
            .context("Cannot initialize buffer sink")?;

        let outputs = AVFilterInOut::new(c"in", &mut buffersrc_ctx, 0);
        let inputs = AVFilterInOut::new(c"out", &mut buffersink_ctx, 0);
        graph
            .parse_ptr(filters_descr, Some(inputs), Some(outputs))
            .context("avfilter_graph_parse_ptr failed")?;
        graph.config().context("avfilter_graph_config failed")?;
    }

    Ok(FilterState { graph })
}

fn display_frame(
    frame: &rsmpeg::avutil::AVFrame,
    out: &mut dyn Write,
    out_w: usize,
    out_h: usize,
) -> Result<()> {
    // write gray8 plane row by row to skip padding
    let linesize = frame.linesize[0] as usize;
    let data0 = frame.data[0];
    unsafe {
        for y in 0..out_h {
            let row = data0.add(y * linesize);
            let buf = std::slice::from_raw_parts(row, out_w);
            out.write_all(buf)?;
        }
    }
    Ok(())
}

pub fn decode_filter_video_run(input: &CStr, out_path: &str) -> Result<usize> {
    let (mut fmt, mut dec_ctx, video_idx) = open_input_file(input)?;
    let filt = init_filters(&fmt, &dec_ctx, video_idx, FILTER_DESCR)?;
    let mut frames = 0usize;
    let mut src = filt
        .graph
        .get_filter(c"in")
        .context("buffersrc not found")?;
    let mut sink = filt
        .graph
        .get_filter(c"out")
        .context("buffersink not found")?;
    let mut out = {
        if let Some(dir) = std::path::Path::new(out_path).parent() {
            std::fs::create_dir_all(dir).ok();
        }
        std::fs::File::create(out_path).with_context(|| format!("open {}", out_path))?
    };
    let out_w = sink.get_w() as usize;
    let out_h = sink.get_h() as usize;

    while let Some(packet) = fmt.read_packet()? {
        if packet.stream_index == video_idx as i32 {
            dec_ctx
                .send_packet(Some(&packet))
                .context("Error while sending a packet to the decoder")?;
            loop {
                match dec_ctx.receive_frame() {
                    Ok(mut frame) => {
                        frame.set_pts(frame.best_effort_timestamp);
                        // push
                        src.buffersrc_add_frame(
                            Some(frame),
                            Some(ffi::AV_BUFFERSRC_FLAG_KEEP_REF as i32),
                        )?;
                        loop {
                            match sink.buffersink_get_frame(None) {
                                Ok(f) => {
                                    display_frame(&f, &mut out, out_w, out_h)?;
                                    frames += 1;
                                }
                                Err(rsmpeg::error::RsmpegError::BufferSinkDrainError)
                                | Err(rsmpeg::error::RsmpegError::BufferSinkEofError) => break,
                                Err(e) => return Err(e.into()),
                            }
                        }
                    }
                    Err(rsmpeg::error::RsmpegError::DecoderDrainError)
                    | Err(rsmpeg::error::RsmpegError::DecoderFlushedError) => break,
                    Err(e) => return Err(e.into()),
                }
            }
        }
    }
    // flush filter graph
    src.buffersrc_add_frame(None, None)?;
    loop {
        match sink.buffersink_get_frame(None) {
            Ok(f) => {
                display_frame(&f, &mut out, out_w, out_h)?;
                frames += 1;
            }
            Err(rsmpeg::error::RsmpegError::BufferSinkDrainError)
            | Err(rsmpeg::error::RsmpegError::BufferSinkEofError) => break,
            Err(e) => return Err(e.into()),
        }
    }

    Ok(frames)
}

#[test]
fn decode_filter_video_test() {
    let input = c"tests/assets/vids/centaur.mpg";
    let out = "tests/output/decode_filter_video/out_gray8_78x24.raw";
    let n = decode_filter_video_run(input, out).unwrap();
    assert!(n > 0);
}
