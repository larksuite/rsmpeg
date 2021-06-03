use std::{ffi::CStr, ptr};

// See https://blogs.gentoo.org/lu_zero/2016/03/21/bitstream-filtering/

use crate::{
    avcodec::{AVCodecParameters, AVPacket},
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

wrap_ref!(AVBitStreamFilter: ffi::AVBitStreamFilter);

impl AVBitStreamFilter {
    /// Find a bitstream filter instance with it's short name.
    ///
    /// See [`ffi::av_bsf_get_by_name`] for more info.
    pub fn find_by_name(name: &CStr) -> Option<AVBitStreamFilterRef> {
        unsafe { ffi::av_bsf_get_by_name(name.as_ptr()) }
            .upgrade()
            .map(|x| unsafe { AVBitStreamFilterRef::from_raw(x) })
    }

    // TODO: implement ffi::av_bsf_next
    // const AVBitStreamFilter *av_bsf_next(void **opaque);
}

wrap!(AVBSFContextUninit: ffi::AVBSFContext);
settable!(AVBSFContextUninit {
    time_base_in: ffi::AVRational
});

/// AVBSFContextUninit exists because you must first init an AVBSFContext before you can send/receive packets.
/// Use [`AVBSFContextUninit::init`] to get an AVBSFContext.
pub struct AVBSFContext(AVBSFContextUninit);

impl AVBSFContext {
    /// Provide input data for the bitstream filter to process. To signal the end of the stream, send an NULL packet to the filter.
    ///
    /// See [`ffi::av_bsf_send_packet`] for more info.
    pub fn send_packet(&mut self, packet: Option<AVPacket>) -> Result<()> {
        let packet_ptr = match packet {
            Some(mut packet) => packet.as_mut_ptr(),
            None => std::ptr::null_mut(),
        };

        match unsafe { ffi::av_bsf_send_packet(self.as_mut_ptr(), packet_ptr) }.upgrade() {
            Ok(_) => Ok(()),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::BitstreamSendPacketAgainError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::BitstreamFlushedError),
            Err(x) => Err(RsmpegError::BitstreamSendPacketError(x)),
        }
    }
    /// Get processed data from the bitstream filter.
    ///
    /// See [`ffi::av_bsf_receive_packet`] for more info.
    pub fn receive_packet(&mut self, mut packet: AVPacket) -> Result<AVPacket> {
        match unsafe { ffi::av_bsf_receive_packet(self.as_mut_ptr(), packet.as_mut_ptr()) }.upgrade() {
            Ok(_) => Ok(packet),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::BitstreamSendPacketAgainError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::BitstreamFlushedError),
            Err(x) => Err(RsmpegError::BitstreamReceivePacketError(x)),
        }
    }
}

impl std::ops::Deref for AVBSFContext {
    type Target = AVBSFContextUninit;
    fn deref(&self) -> &AVBSFContextUninit {
        &self.0
    }
}
impl std::ops::DerefMut for AVBSFContext {
    fn deref_mut(&mut self) -> &mut AVBSFContextUninit {
        &mut self.0
    }
}

impl AVBSFContextUninit {
    /// Create a new [`AVBSFContext`] instance, allocate private data and
    /// initialize defaults for the given [`AVBitStreamFilterRef`].
    ///
    /// See [`ffi::av_bsf_alloc`] for more info.
    pub fn new(filter: &AVBitStreamFilter) -> Self {
        let mut bsfc_raw = ptr::null_mut();

        unsafe {
            ffi::av_bsf_alloc(filter.as_ptr(), &mut bsfc_raw);
            Self::from_raw(bsfc_raw.upgrade().unwrap())
        }
    }
    /// You need to initialize the context before you can send/receive_packets but after you set input parameters via [`AVBSFContextUninit::set_par_in`].
    ///
    /// See [`ffi::av_bsf_init`] for more info.
    pub fn init(mut self) -> Result<AVBSFContext> {
        unsafe {
            match ffi::av_bsf_init(self.as_mut_ptr()).upgrade() {
                Ok(_) => Ok(AVBSFContext(self)),
                Err(x) => Err(RsmpegError::BitstreamInitializationError(x)),
            }
        }
    }

    /// See [`ffi::av_bsf_flush`] for more info.
    pub fn flush(&mut self) {
        unsafe {
            ffi::av_bsf_flush(self.as_mut_ptr());
        }
    }
    /// Copies `source_params` into [`ffi::AVBSFContext`]'s `par_in` field. So we only need a reference to `source_params`.
    ///
    /// See [`ffi::avcodec_parameters_copy`] for more info.
    pub fn set_par_in(&mut self, source_params: &AVCodecParameters) -> Result<()> {
        match unsafe { ffi::avcodec_parameters_copy(self.par_in, source_params.as_ptr()) }.upgrade()
        {
            Ok(_) => Ok(()),
            Err(e) => Err(RsmpegError::AVError(e)),
        }
    }
    // FIXME: Returns bsf_list filter for some reason...
    #[doc(hidden)]
    pub fn get_null() -> Self {
        let mut bsfc_raw = ptr::null_mut();

        unsafe {
            ffi::av_bsf_get_null_filter(&mut bsfc_raw);
            Self::from_raw(bsfc_raw.upgrade().unwrap())
        }
    }
}

impl Drop for AVBSFContextUninit {
    fn drop(&mut self) {
        unsafe { ffi::av_bsf_free(&mut self.as_mut_ptr()) }
    }
}

#[cfg(test)]
mod test {
    use super::{AVBSFContextUninit, AVBitStreamFilter, CStr};

    #[test]
    fn test_filter_by_name() {
        let name = std::ffi::CString::new("null").unwrap();
        let filter_ref = AVBitStreamFilter::find_by_name(&name).unwrap();

        let ctx = AVBSFContextUninit::new(&filter_ref);
        let filter = unsafe { *ctx.filter };
        let filter_name = unsafe { CStr::from_ptr(filter.name) };

        assert_eq!(name.as_c_str(), filter_name);
    }

    #[test]
    #[ignore = "get_null returns bsf_list, idk if it's supposed to"]
    fn test_null_filter() {
        let ctx = AVBSFContextUninit::get_null();
        let filter = unsafe { *ctx.filter };
        let filter_name = unsafe { CStr::from_ptr(filter.name) };

        let name = std::ffi::CString::new("null").unwrap();
        assert_eq!(name.as_c_str(), filter_name);
    }
}
