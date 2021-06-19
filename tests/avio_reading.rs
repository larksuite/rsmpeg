//! Demo of custom IO using `AVIOContextCustom`.
use anyhow::Result;
use cstr::cstr;
use rsmpeg::{
    avformat::{AVFormatContextInput, AVIOContextContainer, AVIOContextCustom},
    avutil::{AVMem, AVMmap},
    ffi,
};
use std::ffi::CStr;

fn avio_reading(filename: &CStr) -> Result<()> {
    let mmap = AVMmap::new(filename)?;
    let mut current = 0;

    let io_context = AVIOContextCustom::alloc_context(
        AVMem::new(4096),
        false,
        vec![],
        Some(Box::new(move |_, buf| {
            let right = mmap.len().min(current + buf.len());
            if right <= current {
                return ffi::AVERROR_EOF;
            }
            let read_len = right - current;
            buf[0..read_len].copy_from_slice(&mmap[current..right]);
            current = right;
            read_len as i32
        })),
        None,
        None,
    );

    let mut input_format_context =
        AVFormatContextInput::from_io_context(AVIOContextContainer::Custom(io_context))?;
    input_format_context.dump(0, filename)?;

    Ok(())
}

#[test]
fn test_avio_reading0() {
    avio_reading(cstr!("tests/assets/vids/bear.mp4")).unwrap();
}

#[test]
fn test_avio_reading1() {
    avio_reading(cstr!("tests/assets/vids/centaur.mpg")).unwrap();
}
