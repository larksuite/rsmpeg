//! Everything related to `libavformat`.
mod avformat;
mod avio;

pub use avformat::*;
pub use avio::*;

crate::avutil::impl_version!(avformat);
