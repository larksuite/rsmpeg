use std::{
    ffi::CStr,
    mem,
    ops::Drop,
    ptr::{self, NonNull},
    slice,
};

use crate::{
    avcodec::{AVCodecID, AVCodecParameters, AVCodecParametersRef, AVPacket},
    avutil::{AVDictionary, AVFrame, AVPixelFormat, AVRational},
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

wrap_ref!(AVCodec: ffi::AVCodec);

impl AVCodec {
    /// Find a static decoder instance with [`AVCodecID`]
    pub fn find_decoder(id: AVCodecID) -> Option<AVCodecRef<'static>> {
        unsafe { ffi::avcodec_find_decoder(id) }
            .upgrade()
            .map(|x| unsafe { AVCodecRef::from_raw(x) })
    }

    /// Find a static encoder instance with [`AVCodecID`]
    pub fn find_encoder(id: AVCodecID) -> Option<AVCodecRef<'static>> {
        unsafe { ffi::avcodec_find_encoder(id) }
            .upgrade()
            .map(|x| unsafe { AVCodecRef::from_raw(x) })
    }

    /// Find a static decoder instance with it short name.
    pub fn find_decoder_by_name(name: &CStr) -> Option<AVCodecRef<'static>> {
        unsafe { ffi::avcodec_find_decoder_by_name(name.as_ptr()) }
            .upgrade()
            .map(|x| unsafe { AVCodecRef::from_raw(x) })
    }

    /// Find a static encoder instance with it short name.
    pub fn find_encoder_by_name(name: &CStr) -> Option<AVCodecRef<'static>> {
        unsafe { ffi::avcodec_find_encoder_by_name(name.as_ptr()) }
            .upgrade()
            .map(|x| unsafe { AVCodecRef::from_raw(x) })
    }

    /// Get name of the codec.
    pub fn name(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.name) }
    }

    /// Get descriptive name for the codec.
    pub fn long_name(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.long_name) }
    }
}

impl<'codec> AVCodec {
    /// Convenient function for probing a pointer until met specific memory
    /// pattern.
    fn probe_len<T>(mut ptr: *const T, tail: T) -> usize {
        for len in 0.. {
            if unsafe { libc::memcmp(ptr as _, &tail as *const _ as _, mem::size_of::<T>()) } == 0 {
                return len;
            }
            unsafe {
                ptr = ptr.add(1);
            }
        }
        unreachable!()
    }

    /// Convenient function for building a memory slice.
    fn build_array<'a, T>(ptr: *const T, tail: T) -> Option<&'a [T]> {
        if ptr.is_null() {
            None
        } else {
            let len = Self::probe_len(ptr, tail);
            Some(unsafe { slice::from_raw_parts(ptr, len) })
        }
    }

    /// Return supported framerates of this [`AVCodec`].
    pub fn supported_framerates(&'codec self) -> Option<&'codec [AVRational]> {
        // terminates with AVRational{0, 0}
        Self::build_array(self.supported_framerates, AVRational { den: 0, num: 0 })
    }

    /// Return supported pix_fmts of this [`AVCodec`].
    pub fn pix_fmts(&'codec self) -> Option<&'codec [AVPixelFormat]> {
        // terminates with -1
        Self::build_array(self.pix_fmts, -1)
    }

    /// Return supported samplerates of this [`AVCodec`].
    pub fn supported_samplerates(&'codec self) -> Option<&'codec [i32]> {
        // terminates with 0
        Self::build_array(self.supported_samplerates, 0)
    }

    /// Return supported sample_fmts of this [`AVCodec`].
    pub fn sample_fmts(&'codec self) -> Option<&'codec [ffi::AVSampleFormat]> {
        // terminates with -1
        Self::build_array(self.sample_fmts, -1)
    }
}

impl Drop for AVCodec {
    fn drop(&mut self) {
        // Do nothing since the encoder and decoder is finded.(The Codec list is
        // constructed staticly)
    }
}

wrap_ref!(AVCodecContext: ffi::AVCodecContext);
settable!(AVCodecContext {
    framerate: AVRational,
    channel_layout: u64,
    height: i32,
    width: i32,
    sample_aspect_ratio: AVRational,
    pix_fmt: i32,
    time_base: AVRational,
    sample_rate: i32,
    channels: i32,
    sample_fmt: i32,
    flags: i32,
    bit_rate: i64,
    strict_std_compliance: i32,
    gop_size: i32,
    max_b_frames: i32,
});

impl AVCodecContext {
    /// Create a new [`AVCodecContext`] instance, allocate private data and
    /// initialize defaults for the given [`AVCodec`].
    pub fn new(codec: &AVCodec) -> Self {
        // ATTENTION here we restrict the usage of avcodec_alloc_context3() by only put in non-null pointers.
        let codec_context = unsafe { ffi::avcodec_alloc_context3(codec.as_ptr()) }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(codec_context) }
    }

    /// Initialize the [`AVCodecContext`].
    ///
    /// dict: A [`AVDictionary`] filled with [`AVCodecContext`] and [`AVCodec`]
    /// private options.  Function returns a [`AVDictionary`] filled with
    /// options that were not found if given dictionary. It can usually be
    /// ignored.
    pub fn open(&mut self, dict: Option<AVDictionary>) -> Result<Option<AVDictionary>> {
        if let Some(mut dict) = dict {
            let dict_ptr = {
                // Doesn't use into_raw or we will drop the dict when error occurs.
                let mut dict_ptr = dict.as_mut_ptr();
                unsafe {
                    ffi::avcodec_open2(self.as_mut_ptr(), ptr::null_mut(), &mut dict_ptr as *mut _)
                }
                .upgrade()
                .map_err(RsmpegError::CodecOpenError)?;
                dict_ptr
            };
            // If no error, dict's inner pointer is dangling, here we manually drop it by using into_raw().
            let _ = dict.into_raw();
            Ok(dict_ptr
                .upgrade()
                .map(|dict_ptr| unsafe { AVDictionary::from_raw(dict_ptr) }))
        } else {
            unsafe { ffi::avcodec_open2(self.as_mut_ptr(), ptr::null_mut(), ptr::null_mut()) }
                .upgrade()
                .map_err(RsmpegError::CodecOpenError)?;
            Ok(None)
        }
    }

    /// Trying to push a packet to current decoding_context([`AVCodecContext`]).
    pub fn send_packet(&mut self, packet: Option<&AVPacket>) -> Result<()> {
        let packet_ptr = match packet {
            Some(packet) => packet.as_ptr(),
            None => ptr::null(),
        };
        match unsafe { ffi::avcodec_send_packet(self.as_mut_ptr(), packet_ptr) }.upgrade() {
            Ok(_) => Ok(()),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::DecoderFullError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::DecoderFlushedError),
            Err(x) => Err(RsmpegError::SendPacketError(x)),
        }
    }

    /// Trying to pull a frame from current decoding_context([`AVCodecContext`]).
    pub fn receive_frame(&mut self) -> Result<AVFrame> {
        let mut frame = AVFrame::new();
        match unsafe { ffi::avcodec_receive_frame(self.as_mut_ptr(), frame.as_mut_ptr()) }.upgrade()
        {
            Ok(_) => Ok(frame),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::DecoderDrainError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::DecoderFlushedError),
            Err(x) => Err(RsmpegError::ReceiveFrameError(x)),
        }
    }

    /// Trying to push a frame to current encoding_context([`AVCodecContext`]).
    pub fn send_frame(&mut self, frame: Option<&AVFrame>) -> Result<()> {
        let frame_ptr = match frame {
            Some(frame) => frame.as_ptr(),
            None => ptr::null(),
        };
        match unsafe { ffi::avcodec_send_frame(self.as_mut_ptr(), frame_ptr) }.upgrade() {
            Ok(_) => Ok(()),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::SendFrameAgainError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::EncoderFlushedError),
            Err(x) => Err(RsmpegError::SendFrameError(x)),
        }
    }

    /// Trying to pull a packet from current encoding_context([`AVCodecContext`]).
    pub fn receive_packet(&mut self) -> Result<AVPacket> {
        let mut packet = AVPacket::new();
        match unsafe { ffi::avcodec_receive_packet(self.as_mut_ptr(), packet.as_mut_ptr()) }
            .upgrade()
        {
            Ok(_) => Ok(packet),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::EncoderDrainError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::EncoderFlushedError),
            Err(x) => Err(RsmpegError::ReceivePacketError(x)),
        }
    }

    /// Fill the codec context based on the values from the supplied codec parameters.
    pub fn set_codecpar(&mut self, codecpar: AVCodecParametersRef) -> Result<()> {
        unsafe { ffi::avcodec_parameters_to_context(self.as_mut_ptr(), codecpar.as_ptr()) }
            .upgrade()
            .map_err(|_| RsmpegError::CodecSetParameterError)?;
        Ok(())
    }

    /// Get a filled [`AVCodecParameters`] based on the values from current [`AVCodecContext`].
    pub fn extract_codecpar(&self) -> AVCodecParameters {
        let mut parameters = AVCodecParameters::new();
        // Only fails on no memory, so unwrap.
        unsafe { ffi::avcodec_parameters_from_context(parameters.as_mut_ptr(), self.as_ptr()) }
            .upgrade()
            .unwrap();
        parameters
    }

    /// Is hardware accelaration enabled in this codec context.
    pub fn is_hwaccel(&self) -> bool {
        // We doesn't expose the `AVHWAccel` because the documentationstates:
        //
        // Nothing in this structure should be accessed by the user. At some
        // point in future it will not be externally visible at all.
        !self.hwaccel.is_null()
    }
}

impl<'ctx> AVCodecContext {
    /// Get a reference to the [`AVCodec`] in current codec context.
    pub fn codec(&'ctx self) -> AVCodecRef<'ctx> {
        unsafe { AVCodecRef::from_raw(NonNull::new(self.codec as *mut _).unwrap()) }
    }
}

impl Drop for AVCodecContext {
    fn drop(&mut self) {
        // A pointer holder
        let mut context = self.as_mut_ptr();
        unsafe {
            ffi::avcodec_free_context(&mut context);
        }
    }
}
