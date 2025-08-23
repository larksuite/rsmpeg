//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/demux_decode.c
use anyhow::{anyhow, Context, Result};
use rsmpeg::{
    avcodec::{AVCodecContext, AVPacket},
    avformat::AVFormatContextInput,
    avutil::{
        get_bytes_per_sample, get_media_type_string, get_packed_sample_fmt, get_pix_fmt_name,
        get_sample_fmt_name, sample_fmt_is_planar, ts2timestr, AVChannelLayout, AVFrame,
    },
    error::RsmpegError,
    ffi::{self, AV_CHANNEL_LAYOUT_MONO},
};
use std::{ffi::CStr, fs, io::Write, path::Path};

struct DemuxState {
    // video
    vout: Option<fs::File>,
    width: i32,
    height: i32,
    pix_fmt: ffi::AVPixelFormat,
    video_frame_count: i32,
    // audio
    aout: Option<fs::File>,
    audio_frame_count: i32,
    audio_time_base: ffi::AVRational,
}

impl DemuxState {
    fn new() -> Self {
        Self {
            vout: None,
            width: 0,
            height: 0,
            pix_fmt: ffi::AV_PIX_FMT_NONE,
            video_frame_count: 0,
            aout: None,
            audio_frame_count: 0,
            audio_time_base: ffi::AVRational { num: 0, den: 1 },
        }
    }
}

// (moved below to mirror C example ordering)

fn output_video_frame(state: &mut DemuxState, frame: &AVFrame) -> Result<()> {
    // Ensure format is constant and rawvideo-friendly. The C example errors if it changes.
    let align = 1; // rawvideo expects tightly packed by default
    if (frame.width != state.width)
        || (frame.height != state.height)
        || (frame.format != state.pix_fmt as i32)
    {
        let old_fmt = get_pix_fmt_name(state.pix_fmt)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());
        let new_fmt = get_pix_fmt_name(frame.format as ffi::AVPixelFormat)
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "unknown".to_string());
        eprintln!("Error: Width, height and pixel format have to be constant in a rawvideo file, but the width, height or pixel format of the input video changed:\nold: width = {}, height = {}, format = {}\nnew: width = {}, height = {}, format = {}",
            state.width, state.height, old_fmt, frame.width, frame.height, new_fmt);
        return Err(anyhow!("video params changed"));
    }

    println!("video_frame n:{}", state.video_frame_count);
    state.video_frame_count += 1;

    let size = frame.image_get_buffer_size(align)?;
    let mut buf = vec![0u8; size];
    let n = frame.image_copy_to_buffer(&mut buf, align)?;
    if let Some(out) = state.vout.as_mut() {
        out.write_all(&buf[..n])?;
    }
    Ok(())
}

fn output_audio_frame(state: &mut DemuxState, frame: &AVFrame) -> Result<()> {
    // Match C example: print info, then write only the first plane.
    let pts_str = ts2timestr(frame.pts, state.audio_time_base);
    println!(
        "audio_frame n:{} nb_samples:{} pts:{}",
        state.audio_frame_count, frame.nb_samples, pts_str
    );
    state.audio_frame_count += 1;

    let sample_fmt = frame.format as ffi::AVSampleFormat;
    let data_size = get_bytes_per_sample(sample_fmt).context("Unknown sample fmt")?;
    let nb_samples: usize = frame.nb_samples.try_into().context("nb_samples overflow")?;
    let unpadded = nb_samples * data_size;

    unsafe {
        let ptr = frame.data[0];
        if let Some(out) = state.aout.as_mut() {
            out.write_all(std::slice::from_raw_parts(ptr, unpadded))?;
        }
    }

    Ok(())
}

fn decode_packet(
    dec: &mut AVCodecContext,
    pkt: Option<&AVPacket>,
    state: &mut DemuxState,
) -> Result<()> {
    // submit the packet to the decoder
    dec.send_packet(pkt)
        .context("Error submitting a packet for decoding")?;
    // receive available frames
    loop {
        match dec.receive_frame() {
            Ok(frame) => {
                if dec.codec_type == ffi::AVMEDIA_TYPE_VIDEO {
                    output_video_frame(state, &frame)?;
                } else {
                    output_audio_frame(state, &frame)?;
                }
            }
            Err(RsmpegError::DecoderDrainError) | Err(RsmpegError::DecoderFlushedError) => break,
            Err(e) => return Err(e).context("Error during decoding"),
        }
    }
    Ok(())
}

fn open_codec_context(
    fmt_ctx: &AVFormatContextInput,
    media_type: ffi::AVMediaType,
    src_filename: &str,
) -> Result<(AVCodecContext, usize)> {
    let media_type_str = get_media_type_string(media_type as i32)
        .and_then(|s| s.to_str().ok())
        .unwrap_or("unknown");

    let (idx, decoder) = fmt_ctx
        .find_best_stream(media_type)
        .with_context(|| {
            format!(
                "Could not find {} stream in input file '{}'",
                media_type_str, src_filename
            )
        })?
        .ok_or_else(|| {
            anyhow!(
                "Could not find {} stream in input file '{}'",
                media_type_str,
                src_filename
            )
        })?;

    let mut ctx = AVCodecContext::new(&decoder);
    ctx.apply_codecpar(&fmt_ctx.streams()[idx].codecpar())
        .with_context(|| {
            format!(
                "Failed to copy {} codec parameters to decoder context",
                media_type_str
            )
        })?;
    ctx.open(None)
        .with_context(|| format!("Failed to open {} codec", media_type_str))?;
    Ok((ctx, idx))
}

fn get_format_from_sample_fmt(sample_fmt: ffi::AVSampleFormat) -> Option<&'static str> {
    // Mirror C example's AV_NE(fmt_be, fmt_le) using Rust cfg on target endianness
    #[cfg(target_endian = "big")]
    let sample_fmt_entries = [
        (ffi::AV_SAMPLE_FMT_U8, "u8"),
        (ffi::AV_SAMPLE_FMT_S16, "s16be"),
        (ffi::AV_SAMPLE_FMT_S32, "s32be"),
        (ffi::AV_SAMPLE_FMT_FLT, "f32be"),
        (ffi::AV_SAMPLE_FMT_DBL, "f64be"),
    ];
    #[cfg(target_endian = "little")]
    let sample_fmt_entries = [
        (ffi::AV_SAMPLE_FMT_U8, "u8"),
        (ffi::AV_SAMPLE_FMT_S16, "s16le"),
        (ffi::AV_SAMPLE_FMT_S32, "s32le"),
        (ffi::AV_SAMPLE_FMT_FLT, "f32le"),
        (ffi::AV_SAMPLE_FMT_DBL, "f64le"),
    ];
    sample_fmt_entries
        .iter()
        .find(|(fmt, _)| *fmt == sample_fmt)
        .map(|(_, fmt)| *fmt)
}

fn demux_decode(input_raw: &CStr, video_out: &str, audio_out: &str) -> Result<()> {
    let input = input_raw.to_str().unwrap();
    // Open input and find stream info
    let mut ictx = AVFormatContextInput::open(input_raw)
        .with_context(|| anyhow!("Could not open source file {}", input_raw.to_str().unwrap()))?;

    // Open best video and audio streams via helper like C example
    let (mut video_ctx, video_stream_index) =
        match open_codec_context(&ictx, ffi::AVMEDIA_TYPE_VIDEO, input) {
            Ok((ctx, idx)) => (Some(ctx), Some(idx as i32)),
            Err(_) => (None, None),
        };

    let (mut audio_ctx, audio_stream_index) =
        match open_codec_context(&ictx, ffi::AVMEDIA_TYPE_AUDIO, input) {
            Ok((ctx, idx)) => (Some(ctx), Some(idx as i32)),
            Err(_) => (None, None),
        };

    if video_ctx.is_none() && audio_ctx.is_none() {
        return Err(anyhow!("Could not find audio or video stream in the input"));
    }

    // Dump input information to stderr
    ictx.dump(0, &input_raw)?;

    // Prepare outputs
    if let Some(dir) = Path::new(video_out).parent() {
        fs::create_dir_all(dir).ok();
    }
    if let Some(dir) = Path::new(audio_out).parent() {
        fs::create_dir_all(dir).ok();
    }

    let mut state = DemuxState::new();
    state.vout = if video_ctx.is_some() {
        Some(
            fs::File::create(video_out)
                .with_context(|| format!("Could not open destination file {}", video_out))?,
        )
    } else {
        None
    };
    state.aout = if audio_ctx.is_some() {
        Some(
            fs::File::create(audio_out)
                .with_context(|| format!("Could not open destination file {}", audio_out))?,
        )
    } else {
        None
    };

    // For video raw frames, ensure we know dimensions and pixel format for the ffplay hint later
    if let Some(vc) = video_ctx.as_ref() {
        state.width = vc.width;
        state.height = vc.height;
        state.pix_fmt = vc.pix_fmt;
    }

    if video_ctx.is_some() {
        println!("Demuxing video from file '{}' into '{}'", input, video_out);
    }
    if audio_ctx.is_some() {
        println!("Demuxing audio from file '{}' into '{}'", input, audio_out);
    }

    if let Some(ac) = audio_ctx.as_ref() {
        state.audio_time_base = ac.time_base;
    }

    // Read packets and decode
    while let Some(pkt) = ictx.read_packet()? {
        if let (Some(ref mut vc), Some(vsi)) = (&mut video_ctx, video_stream_index) {
            if pkt.stream_index == vsi {
                decode_packet(vc, Some(&pkt), &mut state)?;
            }
        }
        if let (Some(ref mut ac), Some(asi)) = (&mut audio_ctx, audio_stream_index) {
            if pkt.stream_index == asi {
                decode_packet(ac, Some(&pkt), &mut state)?;
            }
        }
    }

    // Flush decoders
    if let Some(ref mut vc) = video_ctx {
        decode_packet(vc, None, &mut state)?;
    }
    if let Some(ref mut ac) = audio_ctx {
        decode_packet(ac, None, &mut state)?;
    }

    println!("Demuxing succeeded.");

    // Print ffplay hints like the C example
    if let Some(vc) = video_ctx.as_ref() {
        let pix_name = get_pix_fmt_name(state.pix_fmt)
            .and_then(|s| s.to_str().ok())
            .unwrap_or("unknown");
        println!("Play the output video file with the command:");
        println!(
            "ffplay -f rawvideo -pixel_format {} -video_size {}x{} {}",
            pix_name, vc.width, vc.height, video_out
        );
    }
    if let Some(ac) = audio_ctx.as_ref() {
        let mut sfmt = ac.sample_fmt;
        let mut ch_layout = ac.ch_layout().clone();
        if sample_fmt_is_planar(sfmt) {
            let planar_name = get_sample_fmt_name(sfmt)
                .and_then(|x| x.to_str().ok())
                .unwrap_or("?");
            println!(
                "Warning: the sample format the decoder produced is planar ({}). This example will output the first channel only.",
                planar_name
            );
            sfmt = get_packed_sample_fmt(sfmt).context("Cannot get packed sample fmt")?;
            ch_layout = unsafe { AVChannelLayout::new(AV_CHANNEL_LAYOUT_MONO) };
        }
        let fmt = get_format_from_sample_fmt(sfmt).ok_or_else(|| {
            let name = get_sample_fmt_name(sfmt)
                .and_then(|x| x.to_str().ok())
                .unwrap_or("?");
            anyhow!("sample format {} is not supported as output format", name)
        })?;
        println!("Play the output audio file with the command:");
        println!(
            "ffplay -f {} -ch_layout {} -sample_rate {} {}",
            fmt,
            ch_layout.describe().unwrap().to_string_lossy(),
            ac.sample_rate,
            audio_out
        );
    }

    Ok(())
}

#[test]
fn demux_decode_test() {
    // Pick a test asset that most likely has both audio and video
    let input = c"tests/assets/vids/big_buck_bunny.mp4";
    let vout = "tests/output/demux_decode/bbb.rawvideo";
    let aout = "tests/output/demux_decode/bbb.pcm";
    demux_decode(input, vout, aout).unwrap();
}
