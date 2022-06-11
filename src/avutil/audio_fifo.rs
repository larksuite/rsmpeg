use crate::{error::*, ffi, shared::*};
use std::ops::Drop;
wrap!(AVAudioFifo: ffi::AVAudioFifo);

impl AVAudioFifo {
    pub fn new(sample_fmt: ffi::AVSampleFormat, channels: i32, nb_samples: i32) -> Self {
        let fifo = unsafe { ffi::av_audio_fifo_alloc(sample_fmt, channels, nb_samples) }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(fifo) }
    }

    /// Get the current number of samples in the [`AVAudioFifo`] available for
    /// reading.
    pub fn size(&self) -> i32 {
        unsafe {
            // function doesn't modify self, casting safe
            ffi::av_audio_fifo_size(self.as_ptr() as *mut _)
        }
    }

    /// Get the current number of samples in the [`AVAudioFifo`] available for
    /// writing.
    pub fn space(&self) -> i32 {
        unsafe {
            // function doesn't modify self, casting safe
            ffi::av_audio_fifo_space(self.as_ptr() as *mut _)
        }
    }

    pub fn reset(&mut self) {
        unsafe { ffi::av_audio_fifo_reset(self.as_mut_ptr()) }
    }

    pub fn drain(&mut self, nb_samples: i32) {
        // FFI function only error when the nb_samples is negative.
        unsafe { ffi::av_audio_fifo_drain(self.as_mut_ptr(), nb_samples) }
            .upgrade()
            .unwrap();
    }

    pub fn realloc(&mut self, nb_samples: i32) {
        // Almost only panic on no memory, in other cases panic on invalid
        // parameters which is not possible with current good API.
        unsafe { ffi::av_audio_fifo_realloc(self.as_mut_ptr(), nb_samples) }
            .upgrade()
            .unwrap();
    }

    /// Write data to an AVAudioFifo. If successful, the number of samples
    /// actually written will always be nb_samples.
    ///
    /// The AVAudioFifo will be reallocated automatically if the available space
    /// is less than nb_samples.
    ///
    /// # Safety
    /// Function is safe when the `data` points to valid samples.
    pub unsafe fn write(&mut self, data: *const *mut u8, nb_samples: i32) -> Result<i32> {
        unsafe { ffi::av_audio_fifo_write(self.as_mut_ptr(), data as *mut _, nb_samples) }
            .upgrade()
            .map_err(RsmpegError::AudioFifoWriteError)
    }

    /// Return actually read size if success.
    ///
    /// # Safety
    /// Function is safe when the `data` points to valid array such as AVFrame::data.
    pub unsafe fn read(&mut self, data: *mut *mut u8, nb_samples: i32) -> Result<i32> {
        unsafe { ffi::av_audio_fifo_read(self.as_mut_ptr(), data as _, nb_samples) }
            .upgrade()
            .map_err(RsmpegError::AudioFifoReadError)
    }
}

impl Drop for AVAudioFifo {
    fn drop(&mut self) {
        unsafe { ffi::av_audio_fifo_free(self.as_mut_ptr()) }
    }
}
