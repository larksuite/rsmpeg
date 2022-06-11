use std::{
    ffi::CStr,
    mem,
    ops::Drop,
    ptr::{self, NonNull},
    slice,
};

use crate::{
    avcodec::{AVCodecID, AVCodecParameters, AVPacket},
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

    pub fn channel_layouts(&'codec self) -> Option<&'codec [u64]> {
        // terminates with -1
        Self::build_array(self.channel_layouts, 0)
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

    /// Decode a subtitle message.
    ///
    /// Some decoders (those marked with `AV_CODEC_CAP_DELAY`) have a delay
    /// between input and output. This means that for some packets they will not
    /// immediately produce decoded output and need to be flushed at the end of
    /// decoding to get all the decoded data. Flushing is done by calling this
    /// function with `None`.
    pub fn decode_subtitle(&mut self, packet: Option<&mut AVPacket>) -> Result<Option<AVSubtitle>> {
        let mut subtitle = AVSubtitle::new();
        let mut got_sub = 0;
        let mut local_packet;

        // FFmpeg's documentation of `avcodec_decode_subtitle2`:
        //
        // Flushing is done by calling this function with packets with
        // avpkt->data set to NULL and avpkt->size set to 0 until it stops
        // returning subtitles. It is safe to flush even those decoders that
        // are not marked with AV_CODEC_CAP_DELAY, then no subtitles will be
        // returned.
        let packet = match packet {
            Some(x) => x.as_mut_ptr(),
            None => {
                local_packet = AVPacket::new();
                debug_assert_eq!(local_packet.data, ptr::null_mut());
                debug_assert_eq!(local_packet.size, 0);
                local_packet.as_mut_ptr()
            }
        };

        let _ = unsafe {
            ffi::avcodec_decode_subtitle2(
                self.as_mut_ptr(),
                subtitle.as_mut_ptr(),
                &mut got_sub,
                packet,
            )
        }
        .upgrade()
        .map_err(RsmpegError::AVError)?;

        if got_sub == 0 {
            return Ok(None);
        }
        Ok(Some(subtitle))
    }

    /// Encode subtitle to buffer.
    pub fn encode_subtitle(&mut self, subtitle: &AVSubtitle, buf: &mut [u8]) -> Result<()> {
        unsafe {
            ffi::avcodec_encode_subtitle(
                self.as_mut_ptr(),
                buf.as_mut_ptr(),
                buf.len() as i32,
                subtitle.as_ptr(),
            )
        }
        .upgrade()
        .map_err(RsmpegError::AVError)?;
        Ok(())
    }

    /// Fill the codec context based on the values from the supplied codec parameters.
    ///
    /// ATTENTION: There is no codecpar field in `AVCodecContext`, this function
    /// just fill the codec context based on the values from the supplied codec
    /// parameters. Any allocated fields in current `AVCodecContext` that have a
    /// corresponding field in `codecpar` are freed and replaced with duplicates
    /// of the corresponding field in `codecpar`. Fields in current
    /// `AVCodecContext` that do not have a counterpart in given `codecpar` are
    /// not touched.
    pub fn apply_codecpar(&mut self, codecpar: &AVCodecParameters) -> Result<()> {
        unsafe { ffi::avcodec_parameters_to_context(self.as_mut_ptr(), codecpar.as_ptr()) }
            .upgrade()
            .map_err(RsmpegError::CodecSetParameterError)?;
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
        // We doesn't expose the `AVHWAccel` because the documentation states:
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

wrap_ref_mut!(AVSubtitle: ffi::AVSubtitle);

impl Default for AVSubtitle {
    fn default() -> Self {
        Self::new()
    }
}

impl AVSubtitle {
    /// Create a new [`AVSubtitle`].
    pub fn new() -> Self {
        let subtitle = ffi::AVSubtitle {
            format: 0,
            start_display_time: 0,
            end_display_time: 0,
            num_rects: 0,
            rects: ptr::null_mut(),
            pts: 0,
        };
        let subtitle = Box::leak(Box::new(subtitle));
        // Shouldn't be null, so unwrap here.
        let subtitle = NonNull::new(subtitle).unwrap();
        unsafe { AVSubtitle::from_raw(subtitle) }
    }
}

impl Drop for AVSubtitle {
    fn drop(&mut self) {
        unsafe {
            // Free all allocated data in the given subtitle struct.
            ffi::avsubtitle_free(self.as_mut_ptr());
            // Free the subtitle struct.
            let _ = Box::from_raw(self.as_mut_ptr());
        }
    }
}
