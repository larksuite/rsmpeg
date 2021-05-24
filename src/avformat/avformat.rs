use std::{
    ffi::CStr,
    marker::PhantomData,
    ops::Drop,
    ptr::{self, NonNull},
};

use crate::{
    avcodec::{
        AVCodec, AVCodecParameters, AVCodecParametersMut, AVCodecParametersRef, AVCodecRef,
        AVPacket,
    },
    avformat::AVIOContext,
    avutil::{AVDictionaryMut, AVDictionaryRef, AVRational},
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

wrap!(AVFormatContextInput: ffi::AVFormatContext);

impl AVFormatContextInput {
    /// Create a [`AVFormatContextInput`] instance of a file, and find info of
    /// all streams.
    pub fn open(filename: &CStr) -> Result<Self> {
        let mut input_format_context = ptr::null_mut();

        // GoodToHave: support custom Input format and custom avdictionary
        unsafe {
            ffi::avformat_open_input(
                &mut input_format_context,
                filename.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        .upgrade()
        .map_err(|_| RsmpegError::OpenInputError)?;

        unsafe { ffi::avformat_find_stream_info(input_format_context, ptr::null_mut()) }
            .upgrade()
            .map_err(|_| RsmpegError::FindStreamInfoError)?;

        // Here we can be sure that context is non null
        let context = NonNull::new(input_format_context).unwrap();

        Ok(unsafe { Self::from_raw(context) })
    }

    /// Dump [`ffi::AVFormatContext`]'s info in the "FFmpeg" way.
    ///
    /// The filename here is just for info printing, it really doesn't matter.
    pub fn dump(&mut self, index: usize, filename: &CStr) -> Result<()> {
        unsafe {
            // This input context, so the last parameter is 0
            ffi::av_dump_format(self.as_mut_ptr(), index as i32, filename.as_ptr(), 0);
        }
        Ok(())
    }

    /// Return the next packet of a stream. This function returns what is stored
    /// in the file, and does not validate that what is there are valid packets
    /// for the decoder. It will split what is stored in the file into packets
    /// and return one for each call. It will not omit invalid data between
    /// valid packets so as to give the decoder the maximum information possible
    /// for decoding.
    ///
    /// Return `Err(_)` on error, Return `Ok(None)` on EOF.
    pub fn read_packet(&mut self) -> Result<Option<AVPacket>> {
        let mut packet = AVPacket::new();
        match unsafe { ffi::av_read_frame(self.as_mut_ptr(), packet.as_mut_ptr()) }.upgrade() {
            Ok(_) => Ok(Some(packet)),
            Err(ffi::AVERROR_EOF) => Ok(None),
            Err(x) => Err(RsmpegError::ReadFrameError(x)),
        }
    }

    /// Return the stream index and stream decoder if there is any "best" stream.
    /// "best" means the most likely what the user wants.
    pub fn find_best_stream(
        &self,
        media_type: ffi::AVMediaType,
    ) -> Result<Option<(usize, AVCodecRef<'static>)>> {
        // After FFmpeg 4.4 this should be changed to *const AVCodec, here we preserve the backward compatibility.
        let mut dec = ptr::null_mut();
        // ATTENTION: usage different from FFmpeg documentation.
        //
        // According to ffmpeg's source code, here we legally assume that
        // `av_find_best_stream` doesn't change given `*mut AVFormatContext`.
        match unsafe {
            ffi::av_find_best_stream(self.as_ptr() as *mut _, media_type, -1, -1, &mut dec, 0)
        }
        .upgrade()
        {
            Ok(index) => Ok(Some((index as usize, unsafe {
                AVCodecRef::from_raw(NonNull::new(dec).unwrap())
            }))),
            Err(ffi::AVERROR_STREAM_NOT_FOUND) => Ok(None),
            Err(e) => Err(RsmpegError::AVError(e)),
        }
    }
}

impl<'stream> AVFormatContextInput {
    /// Get Iterator of all [`AVStream`]s in the [`ffi::AVFormatContext`].
    pub fn streams(&'stream self) -> AVStreamRefs<'stream> {
        AVStreamRefs {
            stream_head: NonNull::new(self.streams as *mut _).unwrap(),
            len: self.nb_streams,
            _marker: PhantomData,
        }
    }

    /// Get Iterator of all [`AVInputFormat`]s in the [`ffi::AVFormatContext`].
    pub fn iformat(&'stream self) -> Option<AVInputFormatRef<'stream>> {
        NonNull::new(self.iformat).map(|x| unsafe { AVInputFormatRef::from_raw(x) })
    }

    /// Get metadata of the [`ffi::AVFormatContext`] in [`crate::avutil::AVDictionary`].
    /// demuxing: set by libavformat in `avformat_open_input()`
    /// muxing: may be set by the caller before `avformat_write_header()`
    pub fn metadata(&'stream self) -> Option<AVDictionaryRef<'stream>> {
        NonNull::new(self.metadata).map(|x| unsafe { AVDictionaryRef::from_raw(x) })
    }
}

impl Drop for AVFormatContextInput {
    fn drop(&mut self) {
        let mut context = self.as_mut_ptr();
        unsafe { ffi::avformat_close_input(&mut context) }
    }
}

wrap!(AVFormatContextOutput: ffi::AVFormatContext);

impl AVFormatContextOutput {
    /// Open a file and create a [`AVFormatContextOutput`] instance of that file.
    pub fn create(filename: &CStr) -> Result<Self> {
        let mut output_format_context = ptr::null_mut();

        // Alloc the context
        unsafe {
            ffi::avformat_alloc_output_context2(
                &mut output_format_context,
                ptr::null_mut(),
                ptr::null_mut(),
                filename.as_ptr(),
            )
        }
        .upgrade()
        .map_err(|_| RsmpegError::OpenOutputError)?;

        let mut output_format_context =
            unsafe { Self::from_raw(NonNull::new(output_format_context).unwrap()) };

        // Open corresponding file
        if unsafe { *output_format_context.oformat }.flags & ffi::AVFMT_NOFILE as i32 == 0 {
            let io_context = AVIOContext::open(filename, ffi::AVIO_FLAG_WRITE)?.into_raw();
            unsafe { output_format_context.deref_mut() }.pb = io_context.as_ptr();
        }

        Ok(output_format_context)
    }

    /// Allocate the stream private data and write the stream header to an
    /// output media file.
    pub fn write_header(&mut self) -> Result<()> {
        unsafe { ffi::avformat_write_header(self.as_mut_ptr(), ptr::null_mut()) }
            .upgrade()
            .map_err(RsmpegError::WriteHeaderError)?;

        Ok(())
    }

    /// Write the stream trailer to an output media file and free the file
    /// private data.
    pub fn write_trailer(&mut self) -> Result<()> {
        unsafe { ffi::av_write_trailer(self.as_mut_ptr()) }
            .upgrade()
            .map_err(|_| RsmpegError::WriteTrailerError)?;
        Ok(())
    }

    /// Dump [`ffi::AVFormatContext`] info in the "FFmpeg" way.
    ///
    /// The filename here is just for info printing, it's really doesn't matter.
    pub fn dump(&mut self, index: i32, filename: &CStr) -> Result<()> {
        unsafe {
            // This is output context, so the last parameter is 1
            ffi::av_dump_format(self.as_mut_ptr(), index, filename.as_ptr(), 1);
        }
        Ok(())
    }

    /// Write a packet to an output media file.
    ///
    /// This function passes the packet directly to the muxer, without any
    /// buffering or reordering. The caller is responsible for correctly
    /// interleaving the packets if the format requires it. Callers that want
    /// libavformat to handle the interleaving should call
    /// [`Self::interleaved_write_frame()`] instead of this function.
    pub fn write_frame(&mut self, packet: &mut AVPacket) -> Result<()> {
        unsafe { ffi::av_write_frame(self.as_mut_ptr(), packet.as_mut_ptr()) }
            .upgrade()
            .map_err(|_| RsmpegError::WriteFrameError)?;
        Ok(())
    }

    /// Write a packet to an output media file ensuring correct interleaving.
    ///
    /// This function will buffer the packets internally as needed to make sure
    /// the packets in the output file are properly interleaved in the order of
    /// increasing dts. Callers doing their own interleaving should call
    /// [`Self::write_frame()`] instead of this function.
    pub fn interleaved_write_frame(&mut self, packet: &mut AVPacket) -> Result<()> {
        unsafe { ffi::av_interleaved_write_frame(self.as_mut_ptr(), packet.as_mut_ptr()) }
            .upgrade()
            .map_err(RsmpegError::InterleavedWriteFrameError)?;
        Ok(())
    }
}

impl<'stream> AVFormatContextOutput {
    /// Return Iterator of [`AVStreamRef`].
    pub fn streams(&'stream self) -> AVStreamRefs<'stream> {
        AVStreamRefs {
            stream_head: NonNull::new(self.streams as *mut _).unwrap(),
            len: self.nb_streams,
            _marker: PhantomData,
        }
    }

    /// Get Iterator of all [`AVOutputFormat`]s in the [`ffi::AVFormatContext`].
    pub fn oformat(&'stream self) -> Option<AVOutputFormatRef<'stream>> {
        NonNull::new(self.oformat).map(|x| unsafe { AVOutputFormatRef::from_raw(x) })
    }

    /// Add a new stream to a media file.
    pub fn new_stream(&'stream mut self, codec: Option<&AVCodec>) -> AVStreamMut<'stream> {
        let codec_ptr = match codec {
            Some(codec) => codec.as_ptr(),
            None => ptr::null(),
        };
        let new_stream = unsafe { ffi::avformat_new_stream(self.as_mut_ptr(), codec_ptr) }
            .upgrade()
            .unwrap();

        unsafe { AVStreamMut::from_raw(new_stream) }
    }
}

impl Drop for AVFormatContextOutput {
    fn drop(&mut self) {
        // Here we drop the io context, which won't be touched by
        // avformat_free_context, so let it dangling is safe.
        if unsafe { *self.oformat }.flags & ffi::AVFMT_NOFILE as i32 == 0 {
            if let Some(pb) = NonNull::new(self.pb) {
                let _ = unsafe { AVIOContext::from_raw(pb) };
            }
        }

        unsafe {
            ffi::avformat_free_context(self.as_mut_ptr());
        }
    }
}

wrap_ref!(AVInputFormat: ffi::AVInputFormat);

wrap_ref!(AVOutputFormat: ffi::AVOutputFormat);

wrap_ref_mut!(AVStream: ffi::AVStream);
settable!(AVStream {
    time_base: AVRational,
});

impl AVStream {
    /// Guess the frame rate, based on both the container and codec information.
    ///
    /// Return None when index is not valid. Some(0/1) if no idea.
    pub fn guess_framerate(&self) -> Option<AVRational> {
        Some(unsafe {
            // ATTENTION: Usage diff from documentation, but according to
            // FFmpeg's implementation, we can use nullptr in first parameter
            // and use const pointer in second parameter.
            ffi::av_guess_frame_rate(ptr::null_mut(), self.as_ptr() as *mut _, ptr::null_mut())
        })
    }

    /// Returns the pts of the last muxed packet + its duration
    /// the retuned value is None when used with a demuxer.
    pub fn get_end_pts(&self) -> Option<i64> {
        let result = unsafe { ffi::av_stream_get_end_pts(self.as_ptr()) };
        (result >= 0).then(|| result as i64)
    }

    /// Set codecpar of current stream with given `parameters`.
    pub fn set_codecpar(&mut self, parameters: AVCodecParameters) {
        // ATTENTION: this workflow differs from c version.
        if let Some(codecpar) = self.codecpar.upgrade() {
            let _ = unsafe { AVCodecParameters::from_raw(codecpar) };
        }
        unsafe {
            self.deref_mut().codecpar = parameters.into_raw().as_ptr();
        }
    }
}

impl<'stream> AVStream {
    /// Get codec parameters of current stream.
    pub fn codecpar(&'stream self) -> Option<AVCodecParametersRef<'stream>> {
        NonNull::new(self.codecpar).map(|x| unsafe { AVCodecParametersRef::from_raw(x) })
    }

    /// Get metadata of current stream.
    pub fn metadata(&'stream self) -> Option<AVDictionaryRef<'stream>> {
        NonNull::new(self.metadata).map(|x| unsafe { AVDictionaryRef::from_raw(x) })
    }

    /// Get mutable reference of codec parameters in current stream.
    pub fn codecpar_mut(&'stream mut self) -> Option<AVCodecParametersMut<'stream>> {
        NonNull::new(self.codecpar).map(|x| unsafe { AVCodecParametersMut::from_raw(x) })
    }

    /// Get mutable reference of metadata in current stream.
    pub fn metadata_mut(&'stream mut self) -> Option<AVDictionaryMut<'stream>> {
        NonNull::new(self.metadata).map(|x| unsafe { AVDictionaryMut::from_raw(x) })
    }
}

/// Iterator on reference to raw AVStream `satellite` array.
pub struct AVStreamRefsIter<'stream> {
    ptr: NonNull<NonNull<ffi::AVStream>>,
    end: NonNull<NonNull<ffi::AVStream>>,
    _marker: PhantomData<&'stream ffi::AVStream>,
}

impl<'stream> std::iter::Iterator for AVStreamRefsIter<'stream> {
    type Item = AVStreamRef<'stream>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.ptr == self.end {
            None
        } else {
            let old = self.ptr;
            unsafe {
                self.ptr = NonNull::new_unchecked(self.ptr.as_ptr().offset(1));
            }
            Some(unsafe { AVStreamRef::from_raw(*old.as_ref()) })
        }
    }
}

// ATTENTION Consider add macro for this when similar pattern occurs again.
/// A reference to raw AVStream `satellite` array, cannot be directly constructed. Using
/// this for safety concerns.
pub struct AVStreamRefs<'stream> {
    stream_head: NonNull<NonNull<ffi::AVStream>>,
    len: u32,
    _marker: PhantomData<&'stream ffi::AVStream>,
}

impl<'stream> std::iter::IntoIterator for AVStreamRefs<'stream> {
    type Item = AVStreamRef<'stream>;
    type IntoIter = AVStreamRefsIter<'stream>;
    fn into_iter(self) -> Self::IntoIter {
        let end =
            NonNull::new(unsafe { self.stream_head.as_ptr().add(self.len as usize) }).unwrap();
        AVStreamRefsIter {
            ptr: self.stream_head,
            end,
            _marker: PhantomData,
        }
    }
}

impl<'stream> AVStreamRefs<'stream> {
    /// Get `streams[`index`]`.
    pub fn get(&self, index: usize) -> Option<AVStreamRef<'stream>> {
        // From u32 to usize, safe.
        if index < self.len as usize {
            let stream_ptr = unsafe { *self.stream_head.as_ptr().add(index) };
            Some(unsafe { AVStreamRef::from_raw(stream_ptr) })
        } else {
            None
        }
    }

    /// Get `streams.len()`.
    pub fn num(&self) -> usize {
        // From u32 to usize, safe.
        self.len as usize
    }
}
