use crate::{
    avutil::{av_image_fill_arrays, AVChannelLayoutRef, AVImage, AVMotionVector, AVPixelFormat},
    error::*,
    ffi,
    shared::*,
};

use std::{fmt, mem::size_of, os::raw::c_int, ptr::NonNull, slice};

wrap!(AVFrame: ffi::AVFrame);
settable!(AVFrame {
    width: i32,
    height: i32,
    pts: i64,
    time_base: ffi::AVRational,
    pict_type: ffi::AVPictureType,
    nb_samples: i32,
    format: i32,
    ch_layout: ffi::AVChannelLayout,
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
            .field("ch_layout", &self.ch_layout().describe())
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
    /// Video:
    /// - format (pixel format)
    /// - width
    /// - height
    ///
    /// Audio:
    /// - format (sample format)
    /// - nb_samples
    /// - channel_layout or channels
    ///
    /// Return Error when the some of the frame settings are invalid, allocating
    /// buffer for an already initialized frame or allocation fails because of
    /// no memory.
    pub fn alloc_buffer(&mut self) -> Result<()> {
        // If frame has already been allocated, calling av_frame_get_buffer will
        // leak memory. So we do a check here.
        if self.is_allocated() {
            return Err(RsmpegError::AVFrameDoubleAllocatingError);
        }
        unsafe { ffi::av_frame_get_buffer(self.as_mut_ptr(), 0) }
            .upgrade()
            .map_err(RsmpegError::AVFrameInvalidAllocatingError)?;
        Ok(())
    }

    /// Allocate new buffer(s) for audio or video data.
    ///
    /// The following fields must be set on frame before calling this function:
    /// - format (pixel format for video, sample format for audio)
    /// - width and height for video
    /// - nb_samples and ch_layout for audio
    ///
    /// This function will fill AVFrame.data and AVFrame.buf arrays and, if
    /// necessary, allocate and fill AVFrame.extended_data and AVFrame.extended_buf.
    /// For planar formats, one buffer will be allocated for each plane.
    pub fn get_buffer(&mut self, align: i32) -> Result<()> {
        unsafe { ffi::av_frame_get_buffer(self.as_mut_ptr(), align) }.upgrade()?;
        Ok(())
    }

    pub fn data_mut(&mut self) -> &mut [*mut u8; 8] {
        unsafe { &mut self.deref_mut().data }
    }

    pub fn linesize_mut(&mut self) -> &mut [c_int; 8] {
        unsafe { &mut self.deref_mut().linesize }
    }

    /// Get channel layout
    pub fn ch_layout(&self) -> AVChannelLayoutRef {
        let inner = NonNull::new(&self.ch_layout as *const _ as *mut _).unwrap();
        unsafe { AVChannelLayoutRef::from_raw(inner) }
    }

    /// Setup the data pointers and linesizes based on the specified image
    /// parameters and the provided array.
    ///
    /// # Safety
    /// The buffer src points to cannot outlive the AVFrame. Recommend using
    /// fill_image_buffer() instead. And don't fill thread-local buffer in,
    /// since `AVFrame` is `Send`.
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
        .upgrade()?;
        Ok(())
    }

    /// Ensure that the frame data is writable, avoiding data copy if possible.
    ///
    /// Do nothing if the frame is writable, allocate new buffers and copy the
    /// data if it is not.
    pub fn make_writable(&mut self) -> Result<()> {
        unsafe { ffi::av_frame_make_writable(self.as_mut_ptr()) }.upgrade()?;
        Ok(())
    }

    /// Check if the frame data is writable.
    pub fn is_writable(&self) -> Result<bool> {
        match unsafe { ffi::av_frame_is_writable(self.as_ptr() as *mut _) }.upgrade() {
            Ok(1) => Ok(true),
            Ok(0) => Ok(false),
            Ok(_) => unreachable!(),
            Err(e) => Err(RsmpegError::AVError(e)),
        }
    }

    /// Copy data to or from a hw surface. At least one of self/src must have an
    /// AVHWFramesContext attached.
    ///
    /// If src has an AVHWFramesContext attached, then the format of dst (if set)
    /// must use one of the formats returned by av_hwframe_transfer_get_formats(src,
    /// AV_HWFRAME_TRANSFER_DIRECTION_FROM).
    /// If dst has an AVHWFramesContext attached, then the format of src must use one
    /// of the formats returned by av_hwframe_transfer_get_formats(dst,
    /// AV_HWFRAME_TRANSFER_DIRECTION_TO)
    ///
    /// dst may be "clean" (i.e. with data/buf pointers unset), in which case the
    /// data buffers will be allocated by this function using av_frame_get_buffer().
    /// If dst->format is set, then this format will be used, otherwise (when
    /// dst->format is AV_PIX_FMT_NONE) the first acceptable format will be chosen.
    ///
    /// The two frames must have matching allocated dimensions (i.e. equal to
    /// AVHWFramesContext.width/height), since not all device types support
    /// transferring a sub-rectangle of the whole surface. The display dimensions
    /// (i.e. AVFrame.width/height) may be smaller than the allocated dimensions, but
    /// also have to be equal for both frames. When the display dimensions are
    /// smaller than the allocated dimensions, the content of the padding in the
    /// destination frame is unspecified.
    pub fn hwframe_transfer_data(&mut self, src: &AVFrame) -> Result<()> {
        unsafe { ffi::av_hwframe_transfer_data(self.as_mut_ptr(), src.as_ptr(), 0) }.upgrade()?;
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
}

impl Drop for AVFrame {
    fn drop(&mut self) {
        let mut frame = self.as_mut_ptr();
        unsafe { ffi::av_frame_free(&mut frame) }
    }
}

/// It's a `AVFrame` bound with `AVImage`, the `AVFrame` references the buffer
/// owned by the `AVImage`.
pub struct AVFrameWithImage {
    frame: AVFrame,
    image: AVImage,
}

impl std::ops::Deref for AVFrameWithImage {
    type Target = AVFrame;
    fn deref(&self) -> &Self::Target {
        &self.frame
    }
}

impl std::ops::DerefMut for AVFrameWithImage {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.frame
    }
}

impl AVFrameWithImage {
    /// Create a [`AVFrame`] instance and wrap it with the given [`AVImage`]
    /// into a [`AVFrameWithImage`]. The created frame instance uses the buffer
    /// of given [`AVImage`], and is initialized with the parameter of the given
    /// [`AVImage`]. You can get the inner frame instance by derefenceing. You
    /// can get the inner image instance by [`Self::image()`].
    pub fn new(image: AVImage) -> Self {
        let mut frame = AVFrame::new();
        unsafe {
            // Borrow the image buffer.
            frame.deref_mut().data.clone_from(image.data());
            frame.deref_mut().linesize.clone_from(image.linesizes());
            frame.deref_mut().width = image.width;
            frame.deref_mut().height = image.height;
            frame.deref_mut().format = image.pix_fmt;
        }
        Self { frame, image }
    }

    /// Get reference to inner [`AVImage`] instance.
    pub fn image(&self) -> &AVImage {
        &self.image
    }

    /// Convert `self` into an [`AVImage`] instance.
    pub fn into_image(self) -> AVImage {
        self.image
    }
}

wrap_ref!(AVFrameSideData: ffi::AVFrameSideData);

impl<'frame> AVFrameSideDataRef<'frame> {
    /// # Safety
    ///
    /// You should only call this function when you ensure side data is motion vector.
    pub unsafe fn as_motion_vectors(&self) -> &'frame [AVMotionVector] {
        unsafe {
            slice::from_raw_parts(
                self.data as *const _ as *const ffi::AVMotionVector,
                self.size / size_of::<ffi::AVMotionVector>(),
            )
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{avcodec::AVCodec, avutil::AVChannelLayout};

    #[test]
    fn test_get_buffer() {
        let encoder = AVCodec::find_encoder(ffi::AV_CODEC_ID_AAC).unwrap();
        let mut frame = AVFrame::new();
        frame.set_nb_samples(2);
        frame.set_ch_layout(AVChannelLayout::from_nb_channels(2).into_inner());
        frame.set_format(encoder.sample_fmts().unwrap()[0]);
        assert!(frame.alloc_buffer().is_ok());
    }

    #[test]
    fn test_get_buffer_without_setting() {
        let mut frame = AVFrame::new();
        assert!(matches!(
            frame.alloc_buffer(),
            Err(RsmpegError::AVFrameInvalidAllocatingError(_))
        ));
    }

    #[test]
    fn test_get_buffer_double_alloc() {
        let encoder = AVCodec::find_encoder(ffi::AV_CODEC_ID_AAC).unwrap();
        let mut frame = AVFrame::new();
        frame.set_nb_samples(2);
        frame.set_ch_layout(AVChannelLayout::from_nb_channels(2).into_inner());
        frame.set_format(encoder.sample_fmts().unwrap()[0]);
        frame.alloc_buffer().unwrap();
        assert!(matches!(
            frame.alloc_buffer(),
            Err(RsmpegError::AVFrameDoubleAllocatingError)
        ));
    }

    #[test]
    fn test_frame_with_image_buffer() {
        let image = AVImage::new(ffi::AV_PIX_FMT_RGB24, 256, 256, 0).unwrap();
        let frame = AVFrameWithImage::new(image);
        let _: &Vec<u8> = &frame.image;
    }
}
