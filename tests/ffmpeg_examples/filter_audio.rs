//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/filter_audio.c
use anyhow::{Context, Result};
use rsmpeg::{
    avfilter::{AVFilter, AVFilterGraph},
    avutil::{
        get_bytes_per_sample, get_sample_fmt_name, sample_fmt_is_planar, AVChannelLayout,
        AVDictionary, AVFrame, AVMD5,
    },
    ffi,
};

const INPUT_SAMPLERATE: i32 = 48_000;
const FRAME_SIZE: i32 = 1024;

fn init_filter_graph() -> Result<AVFilterGraph> {
    let graph = AVFilterGraph::new();
    let abuffer = AVFilter::get_by_name(c"abuffer").context("abuffer not found")?;
    let abuffersink = AVFilter::get_by_name(c"abuffersink").context("abuffersink not found")?;
    let volume = AVFilter::get_by_name(c"volume").context("volume not found")?;
    let aformat = AVFilter::get_by_name(c"aformat").context("aformat not found")?;

    // abuffer options via AVOptions, like the C example
    let ch_layout = AVChannelLayout::from_nb_channels(5).describe()?;
    let sample_fmt = get_sample_fmt_name(ffi::AV_SAMPLE_FMT_FLTP).unwrap_or(c"fltp");

    // Create abuffer as "src"
    let mut src = graph
        .alloc_filter_context(&abuffer, c"src")
        .expect("alloc abuffer src");
    src.opt_set(c"channel_layout", &ch_layout)?;
    src.opt_set(c"sample_fmt", sample_fmt)?;
    src.opt_set_q(
        c"time_base",
        ffi::AVRational {
            num: 1,
            den: INPUT_SAMPLERATE,
        },
    )?;
    src.opt_set_int(c"sample_rate", INPUT_SAMPLERATE as i64)?;
    src.init_str(None)
        .context("Could not initialize the abuffer filter")?;

    // Create abuffersink as "sink"
    let mut sink = graph
        .alloc_filter_context(&abuffersink, c"sink")
        .expect("alloc sink");
    sink.init_str(None)
        .context("Could not initialize the abuffersink instance")?;

    // Create volume filter
    let mut vol = graph
        .alloc_filter_context(&volume, c"volume")
        .expect("alloc volume");
    // Set volume via dict equivalent: key=volume, value=0.90
    let mut vol_opts = Some(AVDictionary::new(c"volume", c"0.90", 0));
    vol.init_dict(&mut vol_opts)
        .context("Could not initialize the volume filter")?;

    // Create aformat filter
    let mut fmt = graph
        .alloc_filter_context(&aformat, c"aformat")
        .expect("alloc aformat");
    let fmt_opts = c"sample_fmts=s16:sample_rates=44100:channel_layouts=stereo";
    fmt.init_str(Some(&fmt_opts))
        .context("Could not initialize the aformat filter")?;

    // Link: src -> volume -> aformat -> sink
    src.link(0, &mut vol, 0)
        .context("Error connecting filters")?;
    vol.link(0, &mut fmt, 0)
        .context("Error connecting filters")?;
    fmt.link(0, &mut sink, 0)
        .context("Error connecting filters")?;
    graph
        .config()
        .context("Error configuring the filter graph")?;
    // drop local filter context borrows before returning graph
    drop(src);
    drop(vol);
    drop(fmt);
    drop(sink);
    Ok(graph)
}

// Do something useful with the filtered data: this simple
// example just prints the MD5 checksum of each plane to stdout.
fn process_output(md5: &mut AVMD5, frame: &AVFrame) -> Result<()> {
    let planar = sample_fmt_is_planar(frame.format);
    let channels = frame.ch_layout.nb_channels;
    let planes = if planar { channels } else { 1 } as usize;
    let bps = get_bytes_per_sample(frame.format).unwrap_or(0);
    let plane_size = bps
        .saturating_mul(frame.nb_samples as usize)
        .saturating_mul(if planar { 1 } else { channels as usize });

    for i in 0..planes {
        let data_ptr = unsafe {
            if frame.extended_data.is_null() {
                frame.data[i]
            } else {
                *frame.extended_data.add(i)
            }
        };
        if !data_ptr.is_null() && plane_size > 0 {
            let data = unsafe { std::slice::from_raw_parts(data_ptr, plane_size) };
            // Initialize and compute using the context to match intent
            md5.init();
            md5.update(data);
            let digest = md5.finalize();
            print!("plane {}: 0x", i);
            for b in &digest {
                print!("{:02X}", b);
            }
            println!();
        }
    }
    println!();
    Ok(())
}

fn get_input(frame_num: i64) -> Result<AVFrame> {
    let mut f = AVFrame::new();
    f.set_sample_rate(INPUT_SAMPLERATE);
    f.set_format(ffi::AV_SAMPLE_FMT_FLTP);
    f.set_ch_layout(rsmpeg::avutil::AVChannelLayout::from_nb_channels(5).into_inner());
    f.set_nb_samples(FRAME_SIZE);
    f.set_pts(frame_num * FRAME_SIZE as i64);
    f.get_buffer(0)?;

    // fill planar float samples: 5 channels
    unsafe {
        for ch in 0..5 {
            let ptr = f.data[ch] as *mut f32;
            for i in 0..FRAME_SIZE as isize {
                let val =
                    ((frame_num as f32 + i as f32) * (ch as f32 + 1.0) / FRAME_SIZE as f32).sin();
                *ptr.offset(i) = val;
            }
        }
    }
    Ok(f)
}

pub fn filter_audio_process(duration: f32) -> Result<usize> {
    let nframes = (duration * INPUT_SAMPLERATE as f32 / FRAME_SIZE as f32) as u32;
    let graph = init_filter_graph()?;
    let mut total_out = 0usize;
    let mut src = graph.get_filter(c"src").context("buffersrc not found")?;
    let mut sink = graph.get_filter(c"sink").context("buffersink not found")?;
    let mut md5 = AVMD5::new();
    for n in 0..nframes {
        let frame = get_input(n.into()).context("Error generating input frame")?;
        src.buffersrc_add_frame(Some(frame), None)
            .context("Error submitting the frame to the filtergraph")?;
        loop {
            match sink.buffersink_get_frame(None) {
                Ok(f) => {
                    process_output(&mut md5, &f).context("Error processing the filtered frame")?;
                    total_out += f.nb_samples as usize
                }
                Err(rsmpeg::error::RsmpegError::BufferSinkDrainError)
                | Err(rsmpeg::error::RsmpegError::BufferSinkEofError) => break,
                Err(e) => return Err(anyhow::anyhow!("Error filtering the data: {}", e)),
            }
        }
    }
    // flush
    src.buffersrc_add_frame(None, None)
        .context("Error submitting the frame to the filtergraph")?;
    loop {
        match sink.buffersink_get_frame(None) {
            Ok(f) => {
                process_output(&mut md5, &f).context("Error processing the filtered frame")?;
                total_out += f.nb_samples as usize
            }
            Err(rsmpeg::error::RsmpegError::BufferSinkDrainError)
            | Err(rsmpeg::error::RsmpegError::BufferSinkEofError) => break,
            Err(e) => return Err(anyhow::anyhow!("Error filtering the data: {}", e)),
        }
    }
    Ok(total_out)
}

#[test]
fn filter_audio_test() {
    let total_out = filter_audio_process(5.).unwrap();
    assert!(total_out > 0);
}
