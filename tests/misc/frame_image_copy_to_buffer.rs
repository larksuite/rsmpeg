use anyhow::{bail, Context, Result};
use image::RgbImage;
use rsmpeg::{
    avcodec::*,
    avfilter::{AVFilter, AVFilterInOut},
    avformat::*,
    avutil::*,
    error::RsmpegError,
    ffi,
    swscale::*,
};
use std::{
    ffi::CStr,
    fs::{self},
    path::Path,
};

fn frame_copy_to_buffer(filter_spec: &CStr, output_image_path: impl AsRef<Path>) -> Result<()> {
    let frame = get_libav_allocated_frame(filter_spec)?;
    debug_assert_eq!(frame.format, ffi::AV_PIX_FMT_RGB24);

    let buffer_size = frame.image_get_buffer_size(1)?;
    let mut buffer = vec![0u8; buffer_size];
    let written = frame.image_copy_to_buffer(&mut buffer, 1)?;
    assert_eq!(buffer_size, written);

    write_out_rgb24(
        buffer,
        frame.width as u32,
        frame.height as u32,
        output_image_path,
    )?;

    Ok(())
}

// Use AVFilter to generate test frames (i.e. we cannot create an AVImage to access the data that way)
fn get_libav_allocated_frame(filter_spec: &CStr) -> Result<AVFrame> {
    let testsrc2_filter =
        AVFilter::get_by_name(c"testsrc2").context("could not find testsrc2 filter")?;
    let buffersink_filter =
        AVFilter::get_by_name(c"buffersink").context("could not find buffersink filter")?;

    let filter_graph = rsmpeg::avfilter::AVFilterGraph::new();

    let mut testsrc2_ctx = filter_graph.create_filter_context(
        &testsrc2_filter,
        c"in",
        Some(c"size=800x600:rate=30"),
    )?;

    let mut buffersink_ctx = filter_graph
        .alloc_filter_context(&buffersink_filter, c"out")
        .context("could not allocate buffersink context")?;
    buffersink_ctx.opt_set_bin(c"pix_fmts", &rsmpeg::ffi::AV_PIX_FMT_RGB24)?;
    buffersink_ctx.init_dict(&mut None)?;

    let outputs = AVFilterInOut::new(c"in", &mut testsrc2_ctx, 0);
    let inputs = AVFilterInOut::new(c"out", &mut buffersink_ctx, 0);

    let (_inputs, _outputs) = filter_graph.parse_ptr(filter_spec, Some(inputs), Some(outputs))?;

    filter_graph.config()?;

    let frame = buffersink_ctx.buffersink_get_frame(None)?;
    println!("Frame info: {:#?}", frame);

    Ok(frame)
}

fn write_out_rgb24(
    pixel_values: Vec<u8>,
    width: u32,
    height: u32,
    output_image_path: impl AsRef<Path>,
) -> Result<()> {
    let image = RgbImage::from_raw(width, height, pixel_values)
        .context("Can't create rgb image from buffer")?;

    fs::create_dir_all(
        output_image_path
            .as_ref()
            .parent()
            .context("could not get output parent dir")?,
    )?;
    image.save(output_image_path)?;

    Ok(())
}

#[test]
fn test_frame_copy_to_buffer0() {
    frame_copy_to_buffer(c"null", "tests/output/frame_copy_to_buffer/0.png").unwrap();
}
