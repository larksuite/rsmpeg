use crate::{
    avcodec::{AVCodecParameters, AVCodecParametersRef, AVPacket},
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};
use std::{
    ffi::CStr,
    ptr::{self, NonNull},
};

wrap_ref!(AVBitStreamFilter: ffi::AVBitStreamFilter);

impl AVBitStreamFilter {
    /// Find a bitstream filter instance with it's short name.
    pub fn find_by_name(name: &CStr) -> Option<AVBitStreamFilterRef> {
        unsafe { ffi::av_bsf_get_by_name(name.as_ptr()) }
            .upgrade()
            .map(|x| unsafe { AVBitStreamFilterRef::from_raw(x) })
    }

    /// Get name of the bitstream filter.
    pub fn name(&self) -> &CStr {
        // We assume name is always NonNull, so we do check here.
        let name = NonNull::new(self.name as *mut _).unwrap();
        unsafe { CStr::from_ptr(name.as_ptr()) }
    }

    /// Create an iterator on all the [`AVBitStreamFilterRef`]s.
    pub fn iterate() -> AVBitStreamFilterIter {
        AVBitStreamFilterIter {
            opaque: ptr::null_mut(),
        }
    }
}

/// Iterator of the inner [`AVBitStreamFilterRef`]s. Usually created by the
/// [`AVBitStreamFilter::iterate`] method.
pub struct AVBitStreamFilterIter {
    opaque: *mut u32,
}

impl std::iter::Iterator for AVBitStreamFilterIter {
    type Item = AVBitStreamFilterRef<'static>;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe { ffi::av_bsf_iterate(&mut self.opaque as *mut _ as _) }
            .upgrade()
            .map(|x| unsafe { AVBitStreamFilterRef::from_raw(x) })
    }
}

wrap!(AVBSFContextUninit: ffi::AVBSFContext);
settable!(AVBSFContextUninit {
    time_base_in: ffi::AVRational
});

/// AVBSFContextUninit exists because you must first init an AVBSFContext before
/// you can send/receive packets.  Use [`AVBSFContextUninit::init`] to get an
/// AVBSFContext.
pub struct AVBSFContext(AVBSFContextUninit);

impl AVBSFContext {
    /// Submit a packet for filtering.
    ///
    /// After sending each packet, the filter must be completely drained by
    /// calling [`Self::receive_packet()`] repeatedly until it returns
    /// [`RsmpegError::BitstreamFullError`] or
    /// [`RsmpegError::BitstreamFlushedError`].
    ///
    /// The bitstream filter will take ownership of the input packet.  If packet
    /// is None, it signals the end of the stream (i.e. no more non-empty
    /// packets will be sent; sending more empty packets does nothing) and will
    /// cause the filter to output any packets it may have buffered internally.
    ///
    /// Return `Ok(())` on success. Return [`RsmpegError::BitstreamFullError`]
    /// if packets need to be retrieved from the filter (using
    /// [`Self::receive_packet()`]) before new input can be consumed.
    /// [`RsmpegError::BitstreamSendPacketError`] is returned when EOF or an
    /// error occurs.
    pub fn send_packet(&mut self, packet: Option<&mut AVPacket>) -> Result<()> {
        let packet_ptr = match packet {
            Some(packet) => packet.as_mut_ptr(),
            None => ptr::null_mut(),
        };

        match unsafe { ffi::av_bsf_send_packet(self.as_mut_ptr(), packet_ptr) }.upgrade() {
            Ok(_) => Ok(()),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::BitstreamFullError),
            // After reading the implementation, `av_bsf_send_packet` will
            // return success after first meet of EOF. Sending frame after EOF
            // will cause `AVERROR(EINVAL)`(which can also emitted from else
            // where). So there is no effective way to distinguish EOF and
            // internal error.
            //
            // Err(ffi::AVERROR_EOF) => Err(RsmpegError::BitstreamFlushedError),
            Err(x) => Err(RsmpegError::BitstreamSendPacketError(x)),
        }
    }

    /// Retrieve a filtered packet.
    ///
    /// After sending each packet, the filter must be completely drained by
    /// calling [`Self::receive_packet()`] repeatedly until it returns
    /// [`RsmpegError::BitstreamDrainError`] or
    /// [`RsmpegError::BitstreamFlushedError`].
    ///
    /// Return `Ok(())` on success. Return [`RsmpegError::BitstreamDrainError`]
    /// if more packets need to be sent to the filter (using
    /// [`Self::send_packet()`]) to get more output.
    /// [`RsmpegError::BitstreamFlushedError`] if there will be no further
    /// output from the filter. [`RsmpegError::BitstreamReceivePacketError`] if
    /// an error occurs.
    ///
    /// # EVILNESS:
    /// I found that FFmpeg rely on the reusage of packet sending to the
    /// `AVBSFContext`.  Which means creating a new packet for receiving the
    /// packet doesn't work, which is really evil...... So the API is designed
    /// to take an packet for holding the output, and this packet should only be
    /// the one that changed by [`Self::send_packet()`], or this function will
    /// returns [`RsmpegError::BitstreamFlushedError`].
    pub fn receive_packet(&mut self, packet: &mut AVPacket) -> Result<()> {
        match unsafe { ffi::av_bsf_receive_packet(self.as_mut_ptr(), packet.as_mut_ptr()) }
            .upgrade()
        {
            Ok(_) => Ok(()),
            Err(AVERROR_EAGAIN) => Err(RsmpegError::BitstreamDrainError),
            Err(ffi::AVERROR_EOF) => Err(RsmpegError::BitstreamFlushedError),
            Err(x) => Err(RsmpegError::BitstreamReceivePacketError(x)),
        }
    }

    /// Get reference to parameters of the output stream, which is set after
    /// [`AVBSFContextUninit::init()`].
    pub fn par_out(&self) -> AVCodecParametersRef<'_> {
        unsafe { AVCodecParametersRef::from_raw(NonNull::new(self.par_out).unwrap()) }
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

    /// Prepare the filter for use, after all the parameters and options have
    /// been set.
    ///
    /// You need to initialize the context before you can send/receive_packets
    /// but after you set input parameters via
    /// [`AVBSFContextUninit::set_par_in`].
    pub fn init(mut self) -> Result<AVBSFContext> {
        unsafe { ffi::av_bsf_init(self.as_mut_ptr()) }.upgrade()?;
        Ok(AVBSFContext(self))
    }

    /// Get `filter` field of current [`AVBSFContext`].
    pub fn filter(&self) -> AVBitStreamFilterRef {
        unsafe { AVBitStreamFilterRef::from_raw(NonNull::new(self.filter as *mut _).unwrap()) }
    }

    /// Reset the internal bitstream filter state. Should be called e.g. when
    /// seeking.
    pub fn flush(&mut self) {
        unsafe {
            ffi::av_bsf_flush(self.as_mut_ptr());
        }
    }

    /// Copies `source_params` into [`ffi::AVBSFContext`]'s `par_in` field. So
    /// we only need a reference to `source_params`.
    pub fn set_par_in(&mut self, source_params: &AVCodecParameters) {
        unsafe { ffi::avcodec_parameters_copy(self.par_in, source_params.as_ptr()) }
            .upgrade()
            .unwrap();
    }

    /// Get null/pass-through bitstream filter("bsf_list").
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
    use super::{AVBSFContextUninit, AVBitStreamFilter};
    use cstr::cstr;

    #[test]
    fn test_filter_by_name() {
        let name = cstr!("null");
        let filter_ref = AVBitStreamFilter::find_by_name(name).unwrap();
        let ctx = AVBSFContextUninit::new(&filter_ref);
        assert_eq!(name, ctx.filter().name());
    }

    #[test]
    fn test_null_filter() {
        let ctx = AVBSFContextUninit::get_null();
        assert_eq!(cstr!("null"), ctx.filter().name());
    }

    #[test]
    fn test_filter_iterate() {
        let mut iter = AVBitStreamFilter::iterate();
        for _ in iter.by_ref() {}
        assert!(iter.next().is_none());
    }
}
