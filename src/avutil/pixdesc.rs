use crate::{ffi, shared::PointerUpgrade};

use std::{ffi::CStr, ops::Deref, ptr::NonNull};

pub use ffi::AVComponentDescriptor;

/// Descriptor that unambiguously describes how the bits of a pixel are
/// stored in the up to 4 data planes of an image. It also stores the
/// subsampling factors and number of components.
pub struct AVPixFmtDescriptorRef(NonNull<ffi::AVPixFmtDescriptor>);

impl Deref for AVPixFmtDescriptorRef {
    type Target = ffi::AVPixFmtDescriptor;
    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }
    }
}

impl AVPixFmtDescriptorRef {
    /// Return a pixel format descriptor for provided pixel format or `None` if
    /// this pixel format is unknown.
    pub fn get(pix_fmt: i32) -> Option<Self> {
        unsafe { ffi::av_pix_fmt_desc_get(pix_fmt).upgrade().map(Self) }
    }

    /// Iterate over all pixel format descriptors known to libavutil.
    ///
    /// You can started with [`ffi::AV_PIX_FMT_YUV420P`]
    ///
    /// Return next descriptor or None after the last descriptor
    pub fn next(&self) -> Option<Self> {
        unsafe {
            ffi::av_pix_fmt_desc_next(self.0.as_ptr())
                .upgrade()
                .map(Self)
        }
    }

    /// Return an AVPixelFormat id described by desc, or
    /// [`ffi::AV_PIX_FMT_NONE`] if desc is not a valid pointer to
    /// a pixel format descriptor.
    pub fn get_id(&self) -> i32 {
        unsafe { ffi::av_pix_fmt_desc_get_id(self.0.as_ptr()) }
    }

    /// Get name of the AVPixFmtDescriptor.
    pub fn name(&self) -> &CStr {
        // FFmpeg's implementation: name is always non-null
        let name = self.name.upgrade().unwrap();
        unsafe { CStr::from_ptr(name.as_ptr()) }
    }

    pub fn alias(&self) -> Option<&CStr> {
        // FFmpeg's implementation: alias can be null, always valid UTF-8.
        self.alias
            .upgrade()
            .map(|x| unsafe { CStr::from_ptr(x.as_ptr()) })
    }
}

/// Return the name of the given pixel format, or `None` if `pix_fmt` is not recognized.
///
/// This is a thin safe wrapper over `av_get_pix_fmt_name` from libavutil/pixdesc.h.
///
/// Examples
/// ```rust
/// # use rsmpeg::avutil::get_pix_fmt_name;
/// # use rsmpeg::ffi::AV_PIX_FMT_YUV420P;
/// # use std::ffi::CString;
/// let name = get_pix_fmt_name(AV_PIX_FMT_YUV420P)
///     .map(|s| s.to_string_lossy().into_owned());
/// assert_eq!(name.as_deref(), Some("yuv420p"));
/// ```
pub fn get_pix_fmt_name(pix_fmt: ffi::AVPixelFormat) -> Option<&'static CStr> {
    unsafe {
        ffi::av_get_pix_fmt_name(pix_fmt)
            .upgrade()
            .map(|x| CStr::from_ptr(x.as_ptr()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_pix_fmt_getter() {
        let pix_fmt_desc = AVPixFmtDescriptorRef::get(ffi::AV_PIX_FMT_YUV420P).unwrap();
        assert_eq!(pix_fmt_desc.name(), c"yuv420p");
        assert_eq!(pix_fmt_desc.alias(), None);

        assert_eq!(pix_fmt_desc.nb_components, 3);
        assert_eq!(pix_fmt_desc.log2_chroma_w, 1);
        assert_eq!(pix_fmt_desc.log2_chroma_h, 1);

        assert_eq!(pix_fmt_desc.comp[0].plane, 0);
        assert_eq!(pix_fmt_desc.comp[1].plane, 1);
        assert_eq!(pix_fmt_desc.comp[2].plane, 2);

        assert_eq!(pix_fmt_desc.comp[0].step, 1);
        assert_eq!(pix_fmt_desc.comp[1].step, 1);
        assert_eq!(pix_fmt_desc.comp[2].step, 1);

        assert_eq!(pix_fmt_desc.comp[0].offset, 0);
        assert_eq!(pix_fmt_desc.comp[1].offset, 0);
        assert_eq!(pix_fmt_desc.comp[2].offset, 0);

        assert_eq!(pix_fmt_desc.comp[0].depth, 8);
        assert_eq!(pix_fmt_desc.comp[1].depth, 8);
        assert_eq!(pix_fmt_desc.comp[2].depth, 8);

        let pix_fmt_desc = AVPixFmtDescriptorRef::get(ffi::AV_PIX_FMT_GRAY9LE).unwrap();
        assert_eq!(pix_fmt_desc.name(), c"gray9le");
        assert_eq!(pix_fmt_desc.alias(), Some(c"y9le"));
    }

    #[test]
    fn test_pix_fmt_desc_next() {
        let pix_fmt_desc = AVPixFmtDescriptorRef::get(ffi::AV_PIX_FMT_GRAYF32BE).unwrap();
        assert_eq!(pix_fmt_desc.name(), c"grayf32be");
        assert_eq!(pix_fmt_desc.alias(), Some(c"yf32be"));

        let next = pix_fmt_desc.next().unwrap();
        assert_eq!(next.name(), c"grayf32le");
        assert_eq!(next.alias(), Some(c"yf32le"));
    }

    #[test]
    fn test_pix_fmt_get_id() {
        let pix_fmt = ffi::AV_PIX_FMT_YUVA444P12LE;
        let pix_fmt_desc = AVPixFmtDescriptorRef::get(pix_fmt).unwrap();
        assert_eq!(pix_fmt_desc.get_id(), pix_fmt);
    }

    #[test]
    fn test_get_pix_fmt_name() {
        let s = get_pix_fmt_name(ffi::AV_PIX_FMT_YUV420P).unwrap();
        assert_eq!(s, c"yuv420p");
        assert!(get_pix_fmt_name(ffi::AV_PIX_FMT_NONE).is_none());
    }
}
