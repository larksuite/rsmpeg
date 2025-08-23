//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/resample_audio.c
use rsmpeg::{
    avutil::{av_rescale_rnd, get_bytes_per_sample, AVChannelLayout, AVSamples},
    ffi::{self, AV_CHANNEL_LAYOUT_STEREO, AV_CHANNEL_LAYOUT_SURROUND},
    swresample::SwrContext,
};
use std::{f64::consts::PI, io::Write};

fn get_format_from_sample_fmt(sample_fmt: ffi::AVSampleFormat) -> Option<&'static str> {
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

fn fill_samples(
    buf: &mut [f64],
    nb_samples: usize,
    nb_channels: usize,
    sample_rate: usize,
    t: &mut f64,
) {
    let tincr = 1.0 / sample_rate as f64;
    let c = 2.0 * PI * 440.0;
    let mut dstp = buf.as_mut_ptr();
    for _ in 0..nb_samples {
        unsafe {
            *dstp = (c * *t).sin();
            for j in 1..nb_channels {
                *dstp.add(j) = *dstp;
            }
            dstp = dstp.add(nb_channels);
        }
        *t += tincr;
    }
}

pub fn resample_audio_run(out_path: &str) -> anyhow::Result<usize> {
    // Match C example: src=stereo 48kHz DBL, dst=surround 44.1kHz S16
    let src_ch_layout = unsafe { AVChannelLayout::new(AV_CHANNEL_LAYOUT_STEREO) }; // stereo
    let dst_ch_layout = unsafe { AVChannelLayout::new(AV_CHANNEL_LAYOUT_SURROUND) }; // surround
    let src_rate = 48000;
    let dst_rate = 44100;
    let src_sample_fmt = ffi::AV_SAMPLE_FMT_DBL;
    let dst_sample_fmt = ffi::AV_SAMPLE_FMT_S16;
    let src_nb_samples = 1024;
    let src_nb_channels = src_ch_layout.nb_channels;
    let dst_nb_channels = dst_ch_layout.nb_channels;

    let mut swr = SwrContext::new(
        &dst_ch_layout,
        dst_sample_fmt,
        dst_rate,
        &src_ch_layout,
        src_sample_fmt,
        src_rate,
    )
    .unwrap();
    swr.init().unwrap();

    // Allocate source samples buffer using AVSamples like C example
    let src_data = AVSamples::new(src_nb_channels, src_nb_samples, src_sample_fmt, 0).unwrap();

    // Calculate destination buffer size with some extra space for resampling
    let max_dst_nb_samples = av_rescale_rnd(
        src_nb_samples as i64,
        dst_rate as i64,
        src_rate as i64,
        ffi::AV_ROUND_UP,
    ) as i32;

    let mut dst_data =
        AVSamples::new(dst_nb_channels, max_dst_nb_samples, dst_sample_fmt, 1).unwrap();

    let mut f = std::fs::File::create(out_path)?;
    let mut t = 0.0f64;
    let mut total_bytes = 0usize;

    // Generate 10 seconds of audio like C example
    loop {
        // Generate synthetic audio (sine wave at 440Hz)
        let samples_mut = unsafe {
            std::slice::from_raw_parts_mut(
                src_data.audio_data[0] as *mut f64,
                (src_nb_samples * src_nb_channels) as usize,
            )
        };
        fill_samples(
            samples_mut,
            src_nb_samples as usize,
            src_nb_channels as usize,
            src_rate as usize,
            &mut t,
        );

        // Calculate actual destination samples needed including buffered samples
        let dst_nb_samples = av_rescale_rnd(
            swr.get_delay(src_rate as usize) as i64 + src_nb_samples as i64,
            dst_rate as i64,
            src_rate as i64,
            ffi::AV_ROUND_UP,
        ) as i32;

        // Reallocate destination buffer if needed
        if dst_nb_samples > max_dst_nb_samples {
            dst_data = AVSamples::new(dst_nb_channels, dst_nb_samples, dst_sample_fmt, 1).unwrap();
        }

        // Convert to destination format
        let ret = unsafe {
            swr.convert(
                dst_data.audio_data.as_mut_ptr(),
                dst_nb_samples,
                src_data.audio_data.as_ptr() as *const *const u8,
                src_nb_samples,
            )
        }
        .unwrap();

        let dst_bufsize = get_bytes_per_sample(dst_sample_fmt).unwrap() as usize
            * ret as usize
            * dst_nb_channels as usize;

        println!("t:{:.6} in:{} out:{}", t, src_nb_samples, ret);

        unsafe {
            let p = dst_data.audio_data[0] as *const u8;
            let buf = std::slice::from_raw_parts(p, dst_bufsize);
            f.write_all(buf)?;
        }
        total_bytes += dst_bufsize;

        if t >= 10.0 {
            break;
        }
    }

    let fmt = get_format_from_sample_fmt(dst_sample_fmt).unwrap_or("?");
    let layout_str = dst_ch_layout
        .describe()
        .map(|x| x.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "unknown".into());

    eprintln!(
        "Resampling succeeded. Play the output file with the command:\n\
         ffplay -f {} -channel_layout {} -channels {} -ar {} {}",
        fmt, layout_str, dst_nb_channels, dst_rate, out_path
    );

    Ok(total_bytes)
}

#[test]
fn resample_audio_test() {
    let out = "tests/output/resample_audio/out_s16le_44100_5_1.pcm";
    std::fs::create_dir_all("tests/output/resample_audio").unwrap();
    let nbytes = resample_audio_run(out).unwrap();
    assert!(nbytes > 0);
}
