use std::{
    ffi::CStr,
    mem,
    ops::Drop,
    ptr::{self, NonNull},
    slice,
};

use crate::{
    avcodec::{AVCodecParameters, AVCodecParametersRef, AVPacket},
    avutil::{AVDictionary, AVFrame, AVRational},
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

wrap_ref!(AVCodec: ffi::AVCodec);

impl AVCodec {
    pub fn find_decoder(id: ffi::AVCodecID) -> Option<Self> {
        unsafe { ffi::avcodec_find_decoder(id) }
            .upgrade()
            .map(|x| unsafe { Self::from_raw(x) })
    }

    pub fn find_encoder(id: ffi::AVCodecID) -> Option<Self> {
        unsafe { ffi::avcodec_find_encoder(id) }
            .upgrade()
            .map(|x| unsafe { Self::from_raw(x) })
    }

    pub fn find_decoder_by_name(name: &CStr) -> Option<Self> {
        unsafe { ffi::avcodec_find_decoder_by_name(name.as_ptr()) }
            .upgrade()
            .map(|x| unsafe { Self::from_raw(x) })
    }

    pub fn find_encoder_by_name(name: &CStr) -> Option<Self> {
        unsafe { ffi::avcodec_find_encoder_by_name(name.as_ptr()) }
            .upgrade()
            .map(|x| unsafe { Self::from_raw(x) })
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

    fn build_array<'a, T>(ptr: *const T, tail: T) -> Option<&'a [T]> {
        if ptr.is_null() {
            None
        } else {
            let len = Self::probe_len(ptr, tail);
            Some(unsafe { slice::from_raw_parts(ptr, len) })
        }
    }

    pub fn supported_framerates(&'codec self) -> Option<&'codec [AVRational]> {
        // terminates with AVRational{0, 0}
        Self::build_array(self.supported_framerates, AVRational { den: 0, num: 0 })
    }

    pub fn pix_fmts(&'codec self) -> Option<&'codec [ffi::AVPixelFormat]> {
        // terminates with -1
        Self::build_array(self.pix_fmts, -1)
    }

    pub fn supported_samplerates(&'codec self) -> Option<&'codec [i32]> {
        // terminates with 0
        Self::build_array(self.supported_samplerates, 0)
    }

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
    pub fn new(codec: &AVCodec) -> Self {
        let codec_context = unsafe { ffi::avcodec_alloc_context3(codec.as_ptr()) }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(codec_context) }
    }

    /// Initialize the AVCodecContext.
    ///
    /// dict: A dictionary filled with AVCodecContext and codec-private options.
    /// Function returns a dictionary filled with options that were not found if
    /// given dictionary. It's can usually be ignored.
    pub fn open(&mut self, dict: Option<AVDictionary>) -> Result<Option<AVDictionary>> {
        if let Some(mut dict) = dict {
            let dict_ptr = {
                // Doesn't use into_raw because we can drop the dict when error occurs.
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

    /// This is a wrapper around the deprecated api `avcodec_decode_video2()` and
    /// `avcodec_decode_audio4()`.
    ///
    /// Return Some(frame) on getting frame, return None on not getting frame(or
    /// say frame decoding haven't finished), Return Err on decoding error.
    #[deprecated = "This is a wrapper around the deprecated api `avcodec_decode_video2()` and `avcodec_decode_audio4()`."]
    pub fn decode_packet(&mut self, packet: &AVPacket) -> Result<Option<AVFrame>> {
        let mut frame = AVFrame::new();
        let mut got_frame = 0;
        if self.codec().type_ == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            unsafe {
                ffi::avcodec_decode_video2(
                    self.as_mut_ptr(),
                    frame.as_mut_ptr(),
                    &mut got_frame,
                    packet.as_ptr(),
                )
            }
        } else if self.codec().type_ == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO {
            unsafe {
                ffi::avcodec_decode_audio4(
                    self.as_mut_ptr(),
                    frame.as_mut_ptr(),
                    &mut got_frame,
                    packet.as_ptr(),
                )
            }
        } else {
            panic!("Decode in strange codec context.");
        }
        .upgrade()?;
        Ok(if got_frame != 0 { Some(frame) } else { None })
    }

    /// This is a wrapper around deprecated api: `avcodec_encode_video2` and `avcodec_encode_audio2`.
    #[deprecated = "This is a wrapper around deprecated api: `avcodec_encode_video2` and `avcodec_encode_audio2`."]
    pub fn encode_frame(&mut self, frame: Option<&AVFrame>) -> Result<Option<AVPacket>> {
        let frame_ptr = match frame {
            Some(frame) => frame.as_ptr(),
            None => ptr::null(),
        };
        let mut packet = AVPacket::new();
        let mut got_packet = 0;
        if self.codec().type_ == ffi::AVMediaType_AVMEDIA_TYPE_VIDEO {
            unsafe {
                ffi::avcodec_encode_video2(
                    self.as_mut_ptr(),
                    packet.as_mut_ptr(),
                    frame_ptr,
                    &mut got_packet,
                )
            }
        } else if self.codec().type_ == ffi::AVMediaType_AVMEDIA_TYPE_AUDIO {
            unsafe {
                ffi::avcodec_encode_audio2(
                    self.as_mut_ptr(),
                    packet.as_mut_ptr(),
                    frame_ptr,
                    &mut got_packet,
                )
            }
        } else {
            panic!("Decode in strange codec context.");
        }
        .upgrade()?;
        Ok(if got_packet != 0 { Some(packet) } else { None })
    }

    pub fn send_packet(&mut self, packet: Option<&AVPacket>) -> Result<()> {
        let packet_ptr = match packet {
            Some(packet) => packet.as_ptr(),
            None => ptr::null(),
        };
        match unsafe { ffi::avcodec_send_packet(self.as_mut_ptr(), packet_ptr) }.upgrade() {
            Ok(_) => Ok(()),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::SendPacketAgainError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::DecoderFlushedError),
            Err(x) => Err(RsmpegError::SendPacketError(x)),
        }
    }

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

    pub fn set_codecpar(&mut self, codecpar: AVCodecParametersRef) -> Result<()> {
        unsafe { ffi::avcodec_parameters_to_context(self.as_mut_ptr(), codecpar.as_ptr()) }
            .upgrade()
            .map_err(|_| RsmpegError::CodecSetParameterError)?;
        Ok(())
    }

    pub fn extract_codecpar(&self) -> AVCodecParameters {
        let mut parameters = AVCodecParameters::new();
        // Only fails on no memory, so unwrap.
        unsafe { ffi::avcodec_parameters_from_context(parameters.as_mut_ptr(), self.as_ptr()) }
            .upgrade()
            .unwrap();
        parameters
    }
}

impl<'ctx> AVCodecContext {
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
