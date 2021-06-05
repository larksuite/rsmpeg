use crate::{
    avutil::{av_image_fill_arrays, AVImage, AVMotionVector, AVPixelFormat},
    error::*,
    ffi,
    shared::*,
};

use std::{fmt, mem::size_of, ops::Drop, slice};

wrap!(AVFrame: ffi::AVFrame);
settable!(AVFrame {
    width: i32,
    height: i32,
    pts: i64,
    pict_type: ffi::AVPictureType,
    nb_samples: i32,
    format: i32,
    channel_layout: u64,
    sample_rate: i32,
});

impl fmt::Debug for AVFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AVFrame")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("pts", &self.pts)
            .field("pict_type", &self.pict_type)
            .field("nb_samples", &self.nb_samples)
            .field("format", &self.format)
            .field("channel_layout", &self.channel_layout)
            .field("sample_rate", &self.sample_rate)
            .finish()
    }
}

impl AVFrame {
    pub fn new() -> Self {
        let frame = unsafe { ffi::av_frame_alloc() }.upgrade().unwrap();
        unsafe { Self::from_raw(frame) }
    }

    /// Return true if the data and buffer of current frame is allocated.
    pub fn is_allocated(&self) -> bool {
        !(self.data[0].is_null() && self.buf[0].is_null())
    }

    /// Allocate new buffer(s) for audio or video data.
    /// The following fields must be set on frame before calling this function:
    ///
    /// - format (pixel format for video, sample format for audio)
    /// - width and height for video
    /// - nb_samples and channel_layout for audio
    ///
    /// Return Error when the some of the frame settings are invalid, allocating
    /// buffer for an already initialized frame or allocation fails because of
    /// no memory.
    pub fn alloc_buffer(&mut self) -> Result<()> {
        // If frame already has been allocated, calling av_frame_get_buffer will
        // leak memory. So we do a check here.
        if self.is_allocated() {
            return Err(RsmpegError::AVFrameDoubleAllocatingError);
        }
        unsafe { ffi::av_frame_get_buffer(self.as_mut_ptr(), 0) }
            .upgrade()
            .map_err(|_| RsmpegError::AVFrameInvalidAllocatingError)?;
        Ok(())
    }

    pub fn data_mut(&mut self) -> &mut [*mut u8; 8] {
        unsafe { &mut self.deref_mut().data }
    }

    pub fn linesize_mut(&mut self) -> &mut [libc::c_int; 8] {
        unsafe { &mut self.deref_mut().linesize }
    }

    /// Setup the data pointers and linesizes based on the specified image
    /// parameters and the provided array.
    ///
    /// # Safety
    /// The buffer src points to cannot outlive the AVFrame. Recommend using
    /// fill_image_buffer() instead.
    pub unsafe fn fill_arrays(
        &mut self,
        src: *const u8,
        pix_fmt: AVPixelFormat,
        width: i32,
        height: i32,
    ) -> Result<()> {
        unsafe {
            av_image_fill_arrays(
                self.data_mut().as_mut_ptr(),
                self.linesize_mut().as_mut_ptr(),
                src,
                pix_fmt,
                width,
                height,
                1,
            )
        }
        .upgrade()
        .map_err(|_| RsmpegError::AVImageFillArrayError)?;
        Ok(())
    }
}

impl Clone for AVFrame {
    fn clone(&self) -> Self {
        let new_frame = unsafe { ffi::av_frame_clone(self.as_ptr()) }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(new_frame) }
    }
}

impl Default for AVFrame {
    fn default() -> Self {
        Self::new()
    }
}

impl<'frame> AVFrame {
    pub fn get_side_data(
        &'frame self,
        side_data_type: ffi::AVFrameSideDataType,
    ) -> Option<AVFrameSideDataRef<'frame>> {
        unsafe { ffi::av_frame_get_side_data(self.as_ptr(), side_data_type) }
            .upgrade()
            .map(|side_data_ptr| unsafe { AVFrameSideDataRef::from_raw(side_data_ptr) })
    }

    pub fn get_motion_vectors(&'frame self) -> Option<&'frame [AVMotionVector]> {
        let side_data =
            self.get_side_data(ffi::AVFrameSideDataType_AV_FRAME_DATA_MOTION_VECTORS)?;
        Some(unsafe { side_data.as_motion_vectors() })
    }
}

impl Drop for AVFrame {
    fn drop(&mut self) {
        let mut frame = self.as_mut_ptr();
        unsafe { ffi::av_frame_free(&mut frame) }
    }
}

/// It's a `AVFrame` binded with `AVImage`, the AVFrame copies the buffer
/// pointer from the `AVImage`.
pub struct AVFrameWithImageBuffer<'img> {
    inner: AVFrame,

    _image: &'img mut AVImage,
}

impl<'img> std::ops::Deref for AVFrameWithImageBuffer<'img> {
    type Target = AVFrame;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<'img> std::ops::DerefMut for AVFrameWithImageBuffer<'img> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<'img> AVFrameWithImageBuffer<'img> {
    pub fn new(image: &'img mut AVImage, width: i32, height: i32, format: i32) -> Self {
        let mut frame = AVFrame::new();
        unsafe {
            // Borrow the image buffer.
            frame.deref_mut().data.clone_from(image.data());
            frame.deref_mut().linesize.clone_from(image.linesizes());
            frame.deref_mut().width = width;
            frame.deref_mut().height = height;
            frame.deref_mut().format = format;
        }
        Self {
            inner: frame,
            _image: image,
        }
    }
}

wrap_ref!(AVFrameSideData: ffi::AVFrameSideData);

impl<'frame> AVFrameSideDataRef<'frame> {
    unsafe fn as_motion_vectors(&self) -> &'frame [AVMotionVector] {
        unsafe {
            slice::from_raw_parts(
                self.data as *const _ as *const ffi::AVMotionVector,
                self.size as usize / size_of::<ffi::AVMotionVector>(),
            )
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{avcodec::AVCodec, avutil::av_get_default_channel_layout};

    #[test]
    fn test_get_buffer() {
        let encoder = AVCodec::find_encoder(ffi::AVCodecID_AV_CODEC_ID_AAC).unwrap();
        let mut frame = AVFrame::new();
        frame.set_nb_samples(2);
        frame.set_channel_layout(av_get_default_channel_layout(2));
        frame.set_format(encoder.sample_fmts().unwrap()[0]);
        assert!(frame.alloc_buffer().is_ok());
    }

    #[test]
    fn test_get_buffer_without_setting() {
        let mut frame = AVFrame::new();
        assert!(matches!(
            frame.alloc_buffer(),
            Err(RsmpegError::AVFrameInvalidAllocatingError)
        ));
    }

    #[test]
    fn test_get_buffer_double_alloc() {
        let encoder = AVCodec::find_encoder(ffi::AVCodecID_AV_CODEC_ID_AAC).unwrap();
        let mut frame = AVFrame::new();
        frame.set_nb_samples(2);
        frame.set_channel_layout(av_get_default_channel_layout(2));
        frame.set_format(encoder.sample_fmts().unwrap()[0]);
        frame.alloc_buffer().unwrap();
        assert!(matches!(
            frame.alloc_buffer(),
            Err(RsmpegError::AVFrameDoubleAllocatingError)
        ));
    }
}
