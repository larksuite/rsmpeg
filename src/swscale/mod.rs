//! Everything related to `libswscale`.
mod swscale;
mod utils;

pub use swscale::*;
pub use utils::*;

crate::avutil::impl_version!(swscale);
