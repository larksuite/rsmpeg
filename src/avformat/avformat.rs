use std::{
    ffi::CStr,
    os::raw::c_int,
    ptr::{self, NonNull},
};

use crate::{
    avcodec::{
        AVCodecParameters, AVCodecParametersMut, AVCodecParametersRef, AVCodecRef, AVPacket,
    },
    avformat::{AVIOContext, AVIOContextCustom, AVIOContextURL},
    avutil::{AVDictionary, AVDictionaryMut, AVDictionaryRef, AVRational},
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

/// Container of all kinds of AVIOContexts.
pub enum AVIOContextContainer {
    Url(AVIOContextURL),
    Custom(AVIOContextCustom),
}

wrap! {
    AVFormatContextInput: ffi::AVFormatContext,
    io_context: Option<AVIOContextContainer> = None,
}

impl AVFormatContextInput {
    /// Create a [`AVFormatContextInput`] instance of a file, and find info of
    /// all streams.
    ///
    /// - `url`: url of the stream to open.
    /// - `format`: input format hint. If `format` is some, this parameter forces a specific input format.
    /// - `options`: A dictionary filled with AVFormatContext and demuxer-private options.
    ///    On return this parameter will be destroyed and replaced with a dict containing
    ///    options that were not found.
    pub fn open(
        url: &CStr,
        fmt: Option<&AVInputFormat>,
        options: &mut Option<AVDictionary>,
    ) -> Result<Self> {
        let mut input_format_context = ptr::null_mut();
        let fmt = fmt.map(|x| x.as_ptr()).unwrap_or_else(std::ptr::null) as _;
        let mut options_ptr = options
            .as_mut()
            .map(|x| x.as_mut_ptr())
            .unwrap_or_else(std::ptr::null_mut);

        unsafe {
            ffi::avformat_open_input(
                &mut input_format_context,
                url.as_ptr(),
                fmt,
                &mut options_ptr,
            )
        }
        .upgrade()
        .map_err(RsmpegError::OpenInputError)?;

        // Forget the old options since it's ownership is transferred.
        let mut new_options = options_ptr
            .upgrade()
            .map(|x| unsafe { AVDictionary::from_raw(x) });
        std::mem::swap(options, &mut new_options);
        std::mem::forget(new_options);

        // Here we can be sure that context is non null, constructing here for
        // dropping when `avformat_find_stream_info` fails.
        let mut context = unsafe { Self::from_raw(NonNull::new(input_format_context).unwrap()) };

        unsafe { ffi::avformat_find_stream_info(context.as_mut_ptr(), ptr::null_mut()) }
            .upgrade()
            .map_err(RsmpegError::FindStreamInfoError)?;

        Ok(context)
    }

    /// Create a [`AVFormatContextInput`] instance from an [`AVIOContext`], and find info of
    /// all streams.
    pub fn from_io_context(mut io_context: AVIOContextContainer) -> Result<Self> {
        let input_format_context = {
            // Only fails on no memory, so unwrap().
            // `avformat_open_input`'s documentation:
            //
            // Note that a user-supplied AVFormatContext will be freed on failure.
            //
            // So here we don't construct the `AVFormatContext`, or the
            // `input_format_context` will be double free.
            let input_format_context = unsafe { ffi::avformat_alloc_context() }.upgrade().unwrap();
            unsafe {
                (*input_format_context.as_ptr()).pb = match &mut io_context {
                    AVIOContextContainer::Url(ctx) => ctx.as_mut_ptr(),
                    AVIOContextContainer::Custom(ctx) => ctx.as_mut_ptr(),
                };
            }
            input_format_context
        };

        unsafe {
            ffi::avformat_open_input(
                &mut input_format_context.as_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            )
        }
        .upgrade()
        .map_err(RsmpegError::OpenInputError)?;

        // After `avformat_open_input`, we can `avformat_close_input` after it,
        // so here we can safely construct a `AVFormatContextInput`.
        let mut input_format_context = unsafe { Self::from_raw(input_format_context) };
        input_format_context.io_context = Some(io_context);

        unsafe {
            ffi::avformat_find_stream_info(input_format_context.as_mut_ptr(), ptr::null_mut())
        }
        .upgrade()
        .map_err(RsmpegError::FindStreamInfoError)?;

        Ok(input_format_context)
    }

    /// Dump [`ffi::AVFormatContext`]'s info in the "FFmpeg" way.
    ///
    /// The index and filename here is just for info printing, it really doesn't matter.
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
            Err(x) => Err(x)?,
        }
    }

    /// Return the stream index and stream decoder if there is any "best" stream.
    /// "best" means the most likely what the user wants.
    pub fn find_best_stream(
        &self,
        media_type: ffi::AVMediaType,
    ) -> Result<Option<(usize, AVCodecRef<'static>)>> {
        // After FFmpeg 4.4 this should be changed to *const AVCodec, here we
        // preserve the backward compatibility.
        let dec = ptr::null_mut();
        let mut dec = dec as _;

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
                AVCodecRef::from_raw(NonNull::new(dec as *mut _).unwrap())
            }))),
            Err(ffi::AVERROR_STREAM_NOT_FOUND) => Ok(None),
            Err(e) => Err(RsmpegError::AVError(e)),
        }
    }
}

impl<'stream> AVFormatContextInput {
    /// Return slice of [`AVStreamRef`].
    pub fn streams(&'stream self) -> &'stream [AVStreamRef<'stream>] {
        // #define `<->` as "has the same layout due to repr(transparent)"
        // ```
        // NonNull<ffi::AVStream> <-> *const ffi::AVStream
        // AVStream <-> NonNull<ffi::AVStream>
        // AVStreamRef <-> AVStream
        // ```
        // indicates: AVStreamRef <-> *const ffi::AVStream
        let streams = self.streams as *const *const ffi::AVStream as *const AVStreamRef<'stream>;
        // u32 to usize, safe
        let len = self.nb_streams as usize;

        // I trust that FFmpeg won't give me null pointers :-(
        #[cfg(debug_assertions)]
        {
            let arr = unsafe {
                std::slice::from_raw_parts(self.streams as *const *const ffi::AVStream, len)
            };
            for ptr in arr {
                assert!(!ptr.is_null());
            }
        }

        unsafe { std::slice::from_raw_parts(streams, len) }
    }

    /// Return slice of [`AVStreamMut`].
    pub fn streams_mut(&'stream mut self) -> &'stream mut [AVStreamMut<'stream>] {
        // #define `<->` as "has the same layout due to repr(transparent)"
        // ```
        // NonNull<ffi::AVStream> <-> *const ffi::AVStream
        // AVStream <-> NonNull<ffi::AVStream>
        // AVStreamMut <-> AVStream
        // ```
        // indicates: AVStreamMut <-> *const ffi::AVStream
        let streams = self.streams as *mut AVStreamMut<'stream>;
        // u32 to usize, safe
        let len = self.nb_streams as usize;

        // I trust that FFmpeg won't give me null pointers :-(
        #[cfg(debug_assertions)]
        {
            let arr = unsafe {
                std::slice::from_raw_parts(self.streams as *const *const ffi::AVStream, len)
            };
            for ptr in arr {
                assert!(!ptr.is_null());
            }
        }

        unsafe { std::slice::from_raw_parts_mut(streams, len) }
    }

    /// Get [`AVInputFormatRef`] in the [`AVFormatContextInput`].
    pub fn iformat(&'stream self) -> AVInputFormatRef<'stream> {
        // From the implementation of FFmpeg's `avformat_open_input`, we can be
        // sure that iformat won't be null when demuxing.
        unsafe { AVInputFormatRef::from_raw(NonNull::new(self.iformat as *mut _).unwrap()) }
    }

    /// Get metadata of the [`ffi::AVFormatContext`] in [`crate::avutil::AVDictionary`].
    /// demuxing: set by libavformat in `avformat_open_input()`
    /// muxing: may be set by the caller before `avformat_write_header()`
    pub fn metadata(&'stream self) -> Option<AVDictionaryRef<'stream>> {
        // From implementation:
        // `avformat_find_stream_info()->()read_frame_internal()`, we know
        // `metadata` can be null.
        NonNull::new(self.metadata).map(|x| unsafe { AVDictionaryRef::from_raw(x) })
    }
}

impl Drop for AVFormatContextInput {
    fn drop(&mut self) {
        let mut context = self.as_mut_ptr();
        unsafe { ffi::avformat_close_input(&mut context) }
    }
}

wrap! {
    AVFormatContextOutput: ffi::AVFormatContext,
    io_context: Option<AVIOContextContainer> = None,
}

impl AVFormatContextOutput {
    /// Open a file and create a [`AVFormatContextOutput`] instance of that
    /// file. Give it an [`AVIOContext`] if you want custom IO.
    pub fn create(filename: &CStr, io_context: Option<AVIOContextContainer>) -> Result<Self> {
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
        .upgrade()?;

        let mut output_format_context =
            unsafe { Self::from_raw(NonNull::new(output_format_context).unwrap()) };

        // Documentation of [`ffi::AVFormatContext::pb`] states:
        //
        // Do NOT set this field if AVFMT_NOFILE flag is set in
        // iformat/oformat.flags. In such a case, the (de)muxer will handle I/O
        // in some other way and this field will be NULL.
        //
        // For safeness, we don't use the user the given AVIOContext even if the
        // caller provides one.
        if output_format_context.oformat().flags & ffi::AVFMT_NOFILE as i32 == 0 {
            // If user provides us an `AVIOCustomContext`, use it, or we create a default one.
            let mut io_context = match io_context {
                Some(x) => x,
                None => {
                    AVIOContextContainer::Url(AVIOContextURL::open(filename, ffi::AVIO_FLAG_WRITE)?)
                }
            };
            unsafe {
                output_format_context.deref_mut().pb = match &mut io_context {
                    AVIOContextContainer::Url(ctx) => ctx.as_mut_ptr(),
                    AVIOContextContainer::Custom(ctx) => ctx.as_mut_ptr(),
                };
            }
            output_format_context.io_context = Some(io_context);
        }

        Ok(output_format_context)
    }

    /// Allocate the stream private data and write the stream header to an
    /// output media file.
    ///
    /// - `options`: An [`AVDictionary`] filled with [`AVFormatContextInput`]
    ///     and muxer-private options. On return this parameter will be replaced
    ///     with a dict containing options that were not found. Set this to `None`
    ///     if it's not needed.
    pub fn write_header(&mut self, dict: &mut Option<AVDictionary>) -> Result<()> {
        let mut dict_ptr = dict
            .take()
            .map(|x| x.into_raw().as_ptr())
            .unwrap_or_else(ptr::null_mut);

        let result = unsafe { ffi::avformat_write_header(self.as_mut_ptr(), &mut dict_ptr as _) };

        // Move back the ownership if not consumed.
        *dict = dict_ptr
            .upgrade()
            .map(|x| unsafe { AVDictionary::from_raw(x) });

        result.upgrade()?;

        Ok(())
    }

    /// Write the stream trailer to an output media file and free the file
    /// private data.
    pub fn write_trailer(&mut self) -> Result<()> {
        unsafe { ffi::av_write_trailer(self.as_mut_ptr()) }.upgrade()?;
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
        unsafe { ffi::av_write_frame(self.as_mut_ptr(), packet.as_mut_ptr()) }.upgrade()?;
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
            .upgrade()?;
        Ok(())
    }
}

impl<'stream> AVFormatContextOutput {
    /// Return slice of [`AVStreamRef`].
    pub fn streams(&'stream self) -> &'stream [AVStreamRef<'stream>] {
        // #define `<->` as "has the same layout due to repr(transparent)"
        // ```
        // NonNull<ffi::AVStream> <-> *const ffi::AVStream
        // AVStream <-> NonNull<ffi::AVStream>
        // AVStreamRef <-> AVStream
        // ```
        // indicates: AVStreamRef <-> *const ffi::AVStream
        let streams = self.streams as *const *const ffi::AVStream as *const AVStreamRef<'stream>;
        // u32 to usize, safe
        let len = self.nb_streams as usize;

        // I trust that FFmpeg won't give me null pointers :-(
        #[cfg(debug_assertions)]
        {
            let arr = unsafe {
                std::slice::from_raw_parts(self.streams as *const *const ffi::AVStream, len)
            };
            for ptr in arr {
                assert!(!ptr.is_null());
            }
        }

        unsafe { std::slice::from_raw_parts(streams, len) }
    }

    /// Get [`AVOutputFormat`] from the [`AVFormatContextOutput`].
    pub fn oformat(&self) -> AVOutputFormatRef<'static> {
        // From the implementation of FFmpeg's `avformat_alloc_output_context2`,
        // we can be sure that `oformat` won't be null when muxing.
        unsafe { AVOutputFormatRef::from_raw(NonNull::new(self.oformat as *mut _).unwrap()) }
    }

    /// Set [`AVOutputFormat`] in the [`AVFormatContextOutput`].
    pub fn set_oformat(&mut self, format: AVOutputFormatRef<'static>) {
        // `as _` is for compatibility with older FFmpeg versions(< 5.0)
        unsafe {
            self.deref_mut().oformat = format.as_ptr() as _;
        }
    }

    /// Add a new stream to a media file, should be called by the user before
    /// [`Self::write_header()`];
    pub fn new_stream(&'stream mut self) -> AVStreamMut<'stream> {
        // According to the FFmpeg documention and inner implementation, the
        // second parameter of avformat_new_stream is unused. So ignore it.
        let new_stream = unsafe { ffi::avformat_new_stream(self.as_mut_ptr(), ptr::null()) }
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

impl AVInputFormat {
    /// Find [`AVInputFormat`] based on the short name of the input format.
    pub fn find(short_name: &CStr) -> Option<AVInputFormatRef<'static>> {
        unsafe { ffi::av_find_input_format(short_name.as_ptr()) }
            .upgrade()
            .map(|x| unsafe { AVInputFormatRef::from_raw(x) })
    }
}

wrap_ref!(AVOutputFormat: ffi::AVOutputFormat);

impl AVOutputFormat {
    /// Return the output format in the list of registered output formats which
    /// best matches the provided parameters, or return NULL if there is no
    /// match.
    pub fn guess_format(
        short_name: Option<&CStr>,
        filename: Option<&CStr>,
        mime_type: Option<&CStr>,
    ) -> Option<AVOutputFormatRef<'static>> {
        let short_name = short_name.map(|x| x.as_ptr()).unwrap_or_else(ptr::null);
        let filename = filename.map(|x| x.as_ptr()).unwrap_or_else(ptr::null);
        let mime_type = mime_type.map(|x| x.as_ptr()).unwrap_or_else(ptr::null);

        unsafe { ffi::av_guess_format(short_name, filename, mime_type) }
            .upgrade()
            .map(|x| unsafe { AVOutputFormatRef::from_raw(x) })
    }
}

wrap_ref_mut!(#[repr(transparent)] AVStream: ffi::AVStream);
settable!(AVStream {
    avg_frame_rate: AVRational,
    discard: i32,
    disposition: c_int,
    duration: i64,
    event_flags: c_int,
    sample_aspect_ratio: AVRational,
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
}

impl<'stream> AVStream {
    /// Get codec parameters of current stream.
    pub fn codecpar(&'stream self) -> AVCodecParametersRef<'stream> {
        // Implementation of `avformat_new_stream` tells us this cannot be Null.
        unsafe { AVCodecParametersRef::from_raw(NonNull::new(self.codecpar).unwrap()) }
    }

    /// Get mutable reference of codec parameters in current stream.
    pub fn codecpar_mut(&'stream mut self) -> AVCodecParametersMut<'stream> {
        unsafe { AVCodecParametersMut::from_raw(NonNull::new(self.codecpar).unwrap()) }
    }

    /// Set codecpar of current stream with given `parameters`.
    pub fn set_codecpar(&mut self, parameters: AVCodecParameters) {
        // Since the codecpar in AVStram is always NonNull, this function accepts
        // a Parameters rather than Option<Parameters>

        // ATTENTION: this workflow differs from c version.
        if let Some(codecpar) = self.codecpar.upgrade() {
            let _ = unsafe { AVCodecParameters::from_raw(codecpar) };
        }
        unsafe {
            self.deref_mut().codecpar = parameters.into_raw().as_ptr();
        }
    }

    /// Get metadata of current stream.
    pub fn metadata(&'stream self) -> Option<AVDictionaryRef<'stream>> {
        NonNull::new(self.metadata).map(|x| unsafe { AVDictionaryRef::from_raw(x) })
    }

    /// Get mutable reference of metadata in current stream.
    pub fn metadata_mut(&'stream mut self) -> Option<AVDictionaryMut<'stream>> {
        NonNull::new(self.metadata).map(|x| unsafe { AVDictionaryMut::from_raw(x) })
    }

    /// Set metadata of current [`AVStream`].
    pub fn set_metadata(&mut self, dict: Option<AVDictionary>) {
        // Drop the old_dict
        let _ = NonNull::new(self.metadata).map(|x| unsafe { AVDictionary::from_raw(x) });

        // Move in the new dict.
        unsafe {
            self.deref_mut().metadata = dict
                .map(|x| x.into_raw().as_ptr())
                .unwrap_or(ptr::null_mut());
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use cstr::cstr;

    #[test]
    fn test_find_input_format() {
        let name = cstr!("mpeg");
        let filter_ref = AVInputFormat::find(name).unwrap();
        assert_eq!(
            unsafe { CStr::from_ptr(filter_ref.long_name) },
            cstr!("MPEG-PS (MPEG-2 Program Stream)")
        );

        let name = cstr!("asf");
        let filter_ref = AVInputFormat::find(name).unwrap();
        assert_eq!(
            unsafe { CStr::from_ptr(filter_ref.long_name) },
            cstr!("ASF (Advanced / Active Streaming Format)")
        );

        let name = cstr!("__random__");
        assert!(AVInputFormat::find(name).is_none());
    }
}
