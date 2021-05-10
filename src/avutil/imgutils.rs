use crate::{avutil::AVPixelFormat, ffi, shared::*};
use std::{ptr, slice};

const AV_NUM_DATA_POINTERS: usize = ffi::AV_NUM_DATA_POINTERS as usize;

/// `AVImage` is a image buffer holder. It's a self referential structure.
pub struct AVImage {
    data: [*mut u8; AV_NUM_DATA_POINTERS],
    linesizes: [i32; AV_NUM_DATA_POINTERS],
    // Here we "pin" a vector.
    linear: *mut u8,
    linear_length: usize,
    linear_capacity: usize,
}

impl AVImage {
    /// Returns `None` when parameters are invalid, panic when no memory.
    pub fn new(pix_fmt: AVPixelFormat, width: i32, height: i32, align: i32) -> Option<Self> {
        let num_of_bytes = Self::get_buffer_size(pix_fmt, width, height, align)?;

        let mut data = [ptr::null_mut(); AV_NUM_DATA_POINTERS];
        let mut linesizes = [0; AV_NUM_DATA_POINTERS];
        let mut linear = vec![0u8; num_of_bytes as usize];

        match unsafe {
            ffi::av_image_fill_arrays(
                data.as_mut_ptr(),
                linesizes.as_mut_ptr(),
                linear.as_mut_ptr(),
                pix_fmt,
                width,
                height,
                align,
            )
        }
        .upgrade()
        {
            Ok(_) => {}
            Err(AVERROR_ENOMEM) => panic!(),
            // Won't leak memory here, since Self will be dropped
            Err(_) => return None,
        }

        let linear_length = linear.len();
        let linear_capacity = linear.capacity();
        // Here we leak a vector to "pin" it.
        // Enlarge range, the `as` is safe,
        let linear = Vec::leak(linear).as_mut_ptr();

        Some(Self {
            linear,
            linear_length,
            linear_capacity,
            data,
            linesizes,
        })
    }

    /// Return the size in bytes of the amount of data required to store an image
    /// with the given parameters.
    /// Return None when invalid.
    pub fn get_buffer_size(fmt: AVPixelFormat, width: i32, height: i32, align: i32) -> Option<i32> {
        unsafe { ffi::av_image_get_buffer_size(fmt, width, height, align) }
            .upgrade()
            .ok()
    }

    pub fn data(&self) -> &[*mut u8; AV_NUM_DATA_POINTERS] {
        &self.data
    }

    pub fn linesizes(&self) -> &[i32; AV_NUM_DATA_POINTERS] {
        &self.linesizes
    }

    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.linear, self.linear_length) }
    }
}

impl Drop for AVImage {
    fn drop(&mut self) {
        // Unpin the vector and drop it.
        let _ =
            unsafe { Vec::from_raw_parts(self.linear, self.linear_length, self.linear_capacity) };
    }
}

/// Setup the data pointers and linesizes based on the specified image parameters
/// and the provided array.
///
/// The fields of the given image are filled in by using the src address which
/// points to the image data buffer. Depending on the specified pixel format, one
/// or multiple image data pointers and line sizes will be set. If a planar
/// format is specified, several pointers will be set pointing to the different
/// picture planes and the line sizes of the different planes will be stored in
/// the lines_sizes array. Call with src == NULL to get the required size for the
/// src buffer.
///
/// To allocate the buffer and fill in the dst_data and dst_linesize in one call,
/// use av_image_alloc()
/// Hint: it doesn't copy the buffer, it just splits the buffer.
pub use ffi::av_image_fill_arrays;
