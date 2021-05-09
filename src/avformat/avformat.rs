use std::{
    ffi::CStr,
    marker::PhantomData,
    ops::Drop,
    ptr::{self, NonNull},
};

use crate::{
    avcodec::{AVCodec, AVCodecParameters, AVCodecParametersMut, AVCodecParametersRef, AVPacket},
    avformat::AVIOContext,
    avutil::{AVDictionaryMut, AVDictionaryRef, AVRational},
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

wrap!(AVFormatContextInput: ffi::AVFormatContext);

impl AVFormatContextInput {
    /// GoodToHave: adding a boolean to trigger if call avformat_find_stream_info()
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

    /// Dump FormatContext info in the "FFmpeg" way.
    ///
    /// The filename here is just for info printing, it's really doesn't matter.
    pub fn dump(&mut self, index: usize, filename: &CStr) -> Result<()> {
        unsafe {
            // This input context, so the last parameter is 0
            ffi::av_dump_format(self.as_mut_ptr(), index as i32, filename.as_ptr(), 0);
        }
        Ok(())
    }

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
    ) -> Result<Option<(usize, AVCodec)>> {
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
                AVCodec::from_raw(NonNull::new(dec).unwrap())
            }))),
            Err(ffi::AVERROR_STREAM_NOT_FOUND) => Ok(None),
            Err(e) => Err(RsmpegError::AVError(e)),
        }
    }
}

impl<'stream> AVFormatContextInput {
    pub fn streams(&'stream self) -> AVStreamRefs<'stream> {
        AVStreamRefs {
            stream_head: NonNull::new(self.streams as *mut _).unwrap(),
            len: self.nb_streams,
            _marker: PhantomData,
        }
    }

    pub fn iformat(&'stream self) -> AVInputFormatRef<'stream> {
        unsafe { AVInputFormatRef::from_raw(NonNull::new(self.iformat).unwrap()) }
    }

    pub fn metadata(&'stream self) -> AVDictionaryRef<'stream> {
        // Is valid after avformat_open_input(), so safe.
        unsafe { AVDictionaryRef::from_raw(NonNull::new(self.metadata).unwrap()) }
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

    // Write output file header
    pub fn write_header(&mut self) -> Result<()> {
        unsafe { ffi::avformat_write_header(self.as_mut_ptr(), ptr::null_mut()) }
            .upgrade()
            .map_err(RsmpegError::WriteHeaderError)?;

        Ok(())
    }

    // Write output file trailer
    pub fn write_trailer(&mut self) -> Result<()> {
        unsafe { ffi::av_write_trailer(self.as_mut_ptr()) }
            .upgrade()
            .map_err(|_| RsmpegError::WriteTrailerError)?;
        Ok(())
    }

    /// Dump FormatContext info in the "FFmpeg" way.
    ///
    /// The filename here is just for info printing, it's really doesn't matter.
    pub fn dump(&mut self, index: i32, filename: &CStr) -> Result<()> {
        unsafe {
            // This is output context, so the last parameter is 1
            ffi::av_dump_format(self.as_mut_ptr(), index, filename.as_ptr(), 1);
        }
        Ok(())
    }

    pub fn write_frame(&mut self, packet: &mut AVPacket) -> Result<()> {
        unsafe { ffi::av_write_frame(self.as_mut_ptr(), packet.as_mut_ptr()) }
            .upgrade()
            .map_err(|_| RsmpegError::WriteFrameError)?;
        Ok(())
    }

    pub fn interleaved_write_frame(&mut self, packet: &mut AVPacket) -> Result<()> {
        unsafe { ffi::av_interleaved_write_frame(self.as_mut_ptr(), packet.as_mut_ptr()) }
            .upgrade()
            .map_err(RsmpegError::InterleavedWriteFrameError)?;
        Ok(())
    }
}

impl<'stream> AVFormatContextOutput {
    /// Return Iterator of StreamRefs
    pub fn streams(&'stream self) -> AVStreamRefs<'stream> {
        AVStreamRefs {
            stream_head: NonNull::new(self.streams as *mut _).unwrap(),
            len: self.nb_streams,
            _marker: PhantomData,
        }
    }

    pub fn oformat(&'stream self) -> AVOutputFormatRef<'stream> {
        unsafe { AVOutputFormatRef::from_raw(NonNull::new(self.oformat).unwrap()) }
    }

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
    // Return None when index is not valid. Some(0/1) if no idea.
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
        // TODO: consider using then() :-P?
        if result < 0 {
            None
        } else {
            Some(result as i64)
        }
    }

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
    pub fn codecpar(&'stream self) -> AVCodecParametersRef<'stream> {
        unsafe { AVCodecParametersRef::from_raw(NonNull::new(self.codecpar).unwrap()) }
    }

    pub fn metadata(&'stream self) -> AVDictionaryRef<'stream> {
        unsafe { AVDictionaryRef::from_raw(NonNull::new(self.metadata).unwrap()) }
    }

    pub fn codecpar_mut(&'stream mut self) -> AVCodecParametersMut<'stream> {
        unsafe { AVCodecParametersMut::from_raw(NonNull::new(self.codecpar).unwrap()) }
    }

    pub fn metadata_mut(&'stream mut self) -> AVDictionaryMut<'stream> {
        unsafe { AVDictionaryMut::from_raw(NonNull::new(self.metadata).unwrap()) }
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

/// A reference to raw AVStream `satellite` array, cannot be directly constructed. Using
/// this for safety concerns.
///
/// ATTENTION Consider add macro for this when similar pattern occurs again.
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
    pub fn get(&self, index: usize) -> Option<AVStreamRef<'stream>> {
        // From u32 to usize, safe.
        if index < self.len as usize {
            let stream_ptr = unsafe { *self.stream_head.as_ptr().add(index) };
            Some(unsafe { AVStreamRef::from_raw(stream_ptr) })
        } else {
            None
        }
    }

    pub fn num(&self) -> usize {
        // From u32 to usize, safe.
        self.len as usize
    }
}
