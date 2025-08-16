//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/resample_audio.c
use rsmpeg::{
    avutil::{AVChannelLayout, AVFrame},
    ffi,
    swresample::SwrContext,
};
use std::io::Write;

fn fill_samples(
    buf: &mut [f64],
    nb_samples: usize,
    nb_channels: usize,
    sample_rate: usize,
    t: &mut f64,
) {
    let tincr = 1.0 / sample_rate as f64;
    let c = 2.0 * std::f64::consts::PI * 440.0;
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
    // src: stereo dbl 48k, dst: 5.1 s16 44.1k
    let src_ch = AVChannelLayout::from_nb_channels(2);
    let dst_ch = AVChannelLayout::from_nb_channels(6);
    let mut swr = SwrContext::new(
        &dst_ch.into_inner(),
        ffi::AV_SAMPLE_FMT_S16,
        44100,
        &src_ch.into_inner(),
        ffi::AV_SAMPLE_FMT_DBL,
        48000,
    )
    .unwrap();
    swr.init().unwrap();

    // build src frame
    let nb = 1024;
    let mut src = AVFrame::new();
    src.set_nb_samples(nb);
    src.set_format(ffi::AV_SAMPLE_FMT_DBL);
    src.set_sample_rate(48000);
    src.set_ch_layout(AVChannelLayout::from_nb_channels(2).into_inner());
    src.get_buffer(0).unwrap();
    // For packed formats (non-planar), samples are interleaved per channel in data[0].
    let mut t = 0.0f64;
    let samples_mut =
        unsafe { std::slice::from_raw_parts_mut(src.data[0] as *mut f64, (nb * 2) as usize) };
    fill_samples(samples_mut, nb as usize, 2, 48000usize, &mut t);

    // dst frame (let swr allocate)
    let mut dst = AVFrame::new();
    dst.set_nb_samples(0);
    dst.set_format(ffi::AV_SAMPLE_FMT_S16);
    dst.set_sample_rate(44100);
    dst.set_ch_layout(AVChannelLayout::from_nb_channels(6).into_inner());

    swr.convert_frame(Some(&src), &mut dst).unwrap();

    // Write interleaved s16 to file: nb_samples * channels * 2 bytes
    let bytes = (dst.nb_samples as usize) * (dst.ch_layout.nb_channels as usize) * 2usize;
    let mut f = std::fs::File::create(out_path)?;
    unsafe {
        let p = dst.data[0] as *const u8;
        let buf = std::slice::from_raw_parts(p, bytes);
        f.write_all(buf)?;
    }
    Ok(bytes)
}

#[test]
fn resample_audio_test() {
    let out = "tests/output/resample_audio/out_s16le_44100_5_1.pcm";
    std::fs::create_dir_all("tests/output/resample_audio").unwrap();
    let nbytes = resample_audio_run(out).unwrap();
    assert!(nbytes > 0);
}
