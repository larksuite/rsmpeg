#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::module_inception)]
#![allow(clippy::upper_case_acronyms)]

/// Raw and unsafe FFmpeg functions, structs and constants,
pub use rusty_ffmpeg::ffi;

#[macro_use]
mod macros;

mod shared;

pub mod avcodec;
pub mod avfilter;
pub mod avformat;
pub mod avutil;
pub mod swresample;
pub mod swscale;

pub mod error;

pub use shared::UnsafeDerefMut;
