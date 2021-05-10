use crate::{avutil::AVPixelFormat, error::*, ffi, shared::*};
use std::ptr::{self, NonNull};
wrap!(AVPicture: ffi::AVPicture);

impl AVPicture {
    #[deprecated = "AVPicture is deprecated"]
    pub fn data_mut(&mut self) -> &mut [*mut u8; 8] {
        unsafe { &mut self.deref_mut().data }
    }

    #[deprecated = "AVPicture is deprecated"]
    pub fn linesize_mut(&mut self) -> &mut [libc::c_int; 8] {
        unsafe { &mut self.deref_mut().linesize }
    }

    #[deprecated = "AVPicture is deprecated"]
    pub fn new(pix_fmt: AVPixelFormat, width: i32, height: i32) -> Option<Self> {
        let mut picture = ffi::AVPicture {
            data: [ptr::null_mut(); 8],
            linesize: [0; 8],
        };
        // When pix_fmt or width or height is invalid, return None
        match unsafe { ffi::avpicture_alloc(&mut picture, pix_fmt, width, height) }.upgrade() {
            Ok(_) => {}
            Err(AVERROR_ENOMEM) => panic!(),
            Err(_) => return None,
        }
        unsafe {
            Some(Self::from_raw(
                NonNull::new(Box::into_raw(Box::new(picture))).unwrap(),
            ))
        }
    }

    #[deprecated = "Use av_image_get_buffer_size() instead."]
    pub fn get_size(pix_fmt: AVPixelFormat, width: i32, height: i32) -> Result<i32> {
        unsafe { ffi::avpicture_get_size(pix_fmt, width, height) }
            .upgrade()
            .map_err(|_| RsmpegError::AVPictureGetSizeError)
    }

    #[deprecated = "Use av_image_copy() instead"]
    pub fn copy(&self, dst: &mut AVPicture, pix_fmt: AVPixelFormat, width: i32, height: i32) {
        unsafe { ffi::av_picture_copy(dst.as_mut_ptr(), self.as_ptr(), pix_fmt, width, height) }
    }

    /// Copy [`AVPicture`]'s image data from an image into a buffer.
    ///
    /// # Safety
    /// The dest's size shouldn't smaller than dest_size.
    #[deprecated = "Use av_image_copy_to_buffer() instead"]
    pub unsafe fn layout(
        &self,
        pix_fmt: AVPixelFormat,
        width: i32,
        height: i32,
        dest: *mut u8,
        dest_size: i32,
    ) -> Result<()> {
        unsafe { ffi::avpicture_layout(self.as_ptr(), pix_fmt, width, height, dest, dest_size) }
            .upgrade()
            .map_err(|_| RsmpegError::AVPictureCopyToBufferError)?;
        Ok(())
    }

    /// Setup the [`AVPicture`]'s data pointers and linesizes based on the
    /// specified image parameters and the provided array.
    ///
    /// # Safety
    /// The dest's size shouldn't smaller than dest_size.
    #[allow(deprecated)]
    #[deprecated = "Use av_image_fill_arrays() instead."]
    pub unsafe fn fill(
        &mut self,
        src: *const u8,
        pix_fmt: AVPixelFormat,
        width: i32,
        height: i32,
    ) -> Result<()> {
        unsafe {
            ffi::av_image_fill_arrays(
                self.data_mut().as_mut_ptr(),
                self.linesize_mut().as_mut_ptr(),
                src,
                pix_fmt,
                width,
                height,
                0,
            )
        }
        .upgrade()
        .map_err(|_| RsmpegError::AVImageFillArrayError)?;
        Ok(())
    }
}

impl Drop for AVPicture {
    fn drop(&mut self) {
        unsafe {
            ffi::avpicture_free(self.as_mut_ptr());
        }
        let _ = unsafe { Box::from_raw(self.as_mut_ptr()) };
    }
}
