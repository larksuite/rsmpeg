//! RIIR: https://github.com/FFmpeg/FFmpeg/blob/master/doc/examples/scale_video.c
use anyhow::Result;
use rsmpeg::{
    avutil::{AVFrame, AVImage},
    ffi,
    swscale::SwsContext,
};
use std::io::Write;

fn fill_yuv_image(frame: &mut AVFrame, w: i32, h: i32, idx: i32) {
    unsafe {
        // Y
        for y in 0..h {
            for x in 0..w {
                *frame.data[0].offset((y * frame.linesize[0] + x) as isize) =
                    (x + y + 3 * idx) as u8;
            }
        }
        // Cb, Cr
        for y in 0..(h / 2) {
            for x in 0..(w / 2) {
                *frame.data[1].offset((y * frame.linesize[1] + x) as isize) =
                    (128 + y + 2 * idx) as u8;
                *frame.data[2].offset((y * frame.linesize[2] + x) as isize) =
                    (64 + x + 5 * idx) as u8;
            }
        }
    }
}

pub fn scale_video_run(out_path: &str, dst_w: i32, dst_h: i32, nframes: i32) -> Result<usize> {
    let src_w = 320;
    let src_h = 240;

    let src_img = AVImage::new(ffi::AV_PIX_FMT_YUV420P, src_w, src_h, 16).unwrap();
    let mut src = AVFrame::new();
    src.data_mut().clone_from(src_img.data());
    src.linesize_mut().clone_from(src_img.linesizes());
    src.set_format(ffi::AV_PIX_FMT_YUV420P);
    src.set_width(src_w);
    src.set_height(src_h);

    let dst_img = AVImage::new(ffi::AV_PIX_FMT_RGB24, dst_w, dst_h, 1).unwrap();
    let mut dst = AVFrame::new();
    dst.data_mut().clone_from(dst_img.data());
    dst.linesize_mut().clone_from(dst_img.linesizes());
    dst.set_format(ffi::AV_PIX_FMT_RGB24);
    dst.set_width(dst_w);
    dst.set_height(dst_h);

    let mut sws = SwsContext::get_context(
        src_w,
        src_h,
        ffi::AV_PIX_FMT_YUV420P,
        dst_w,
        dst_h,
        ffi::AV_PIX_FMT_RGB24,
        ffi::SWS_BILINEAR as u32,
        None,
        None,
        None,
    )
    .unwrap();

    let mut file = std::fs::File::create(out_path)?;
    let mut total = 0usize;
    for i in 0..nframes {
        fill_yuv_image(&mut src, src_w, src_h, i);
        sws.scale_frame(&src, 0, src_h, &mut dst)?;
        let bytes = dst.image_get_buffer_size(1).unwrap() as usize;
        let buf = unsafe { std::slice::from_raw_parts(dst.data[0], bytes) };
        file.write_all(buf)?;
        total += bytes;
    }
    Ok(total)
}

#[test]
fn scale_video_test() {
    let out = "tests/output/scale_video/out_rgb24_160x120.raw";
    std::fs::create_dir_all("tests/output/scale_video").unwrap();
    let total = scale_video_run(out, 160, 120, 100).unwrap();
    assert!(total > 0);
}
