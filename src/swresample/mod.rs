//! Everything related to `libswresample`.
mod swresample;

pub use swresample::*;

crate::avutil::impl_version!(swresample);
