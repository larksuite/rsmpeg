use std::{
    ffi::{c_void, CStr},
    marker::PhantomData,
    ops::Deref,
    ptr::{self, NonNull},
    slice,
};

use crate::{
    avutil::{AVDictionary, AVMem},
    error::*,
    ffi,
    shared::*,
};

wrap!(AVIOContext: ffi::AVIOContext);

pub struct AVIOContextURL(AVIOContext);

impl Deref for AVIOContextURL {
    type Target = AVIOContext;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl std::ops::DerefMut for AVIOContextURL {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AVIOContextURL {
    /// Create and initialize a [`AVIOContextURL`] for accessing the resource indicated
    /// by url.
    ///
    /// When the resource indicated by url has been opened in read+write mode,
    /// the [`AVIOContextURL`] can be used only for writing.
    ///
    /// `options` A dictionary filled with protocol-private options.
    pub fn open(
        url: &CStr,
        flags: u32,
        options: Option<&mut Option<AVDictionary>>,
    ) -> Result<Self> {
        let mut io_context = ptr::null_mut();
        let mut dummy_options = None;
        let options = options.unwrap_or(&mut dummy_options);
        let mut options_ptr = options
            .as_mut()
            .map(|x| x.as_mut_ptr())
            .unwrap_or_else(ptr::null_mut);

        unsafe {
            ffi::avio_open2(
                &mut io_context,
                url.as_ptr(),
                flags as _,
                ptr::null(),
                &mut options_ptr,
            )
        }
        .upgrade()?;

        // Forget the old options since it's ownership is transferred.
        let mut new_options = options_ptr
            .upgrade()
            .map(|x| unsafe { AVDictionary::from_raw(x) });
        std::mem::swap(options, &mut new_options);
        std::mem::forget(new_options);

        Ok(Self(unsafe {
            AVIOContext::from_raw(NonNull::new(io_context).unwrap())
        }))
    }
}

impl Drop for AVIOContextURL {
    fn drop(&mut self) {
        unsafe { ffi::avio_close(self.as_mut_ptr()) }
            .upgrade()
            .unwrap();
    }
}

mod opaque {
    use std::{ffi::c_void, slice};

    pub type ReadOpaqueCallback<T> = Box<dyn FnMut(&mut T, &mut [u8]) -> i32 + Send + 'static>;
    pub type WriteOpaqueCallback<T> = Box<dyn FnMut(&mut T, &[u8]) -> i32 + Send + 'static>;
    pub type SeekOpaqueCallback<T> = Box<dyn FnMut(&mut T, i64, i32) -> i64 + Send + 'static>;

    pub type ReadPacketCallback = ReadOpaqueCallback<Vec<u8>>;
    pub type WritePacketCallback = WriteOpaqueCallback<Vec<u8>>;
    pub type SeekCallback = SeekOpaqueCallback<Vec<u8>>;

    pub struct Opaque<T: Send + Sync> {
        pub data: T,
        pub read_packet: Option<ReadOpaqueCallback<T>>,
        pub write_packet: Option<WriteOpaqueCallback<T>>,
        pub seek: Option<SeekOpaqueCallback<T>>,
    }

    pub unsafe extern "C" fn read_c<T: Send + Sync>(
        opaque: *mut c_void,
        data: *mut u8,
        len: i32,
    ) -> i32 {
        let buf = unsafe { slice::from_raw_parts_mut(data, len as usize) };
        let opaque = unsafe { (opaque as *mut Opaque<T>).as_mut() }.unwrap();
        opaque.read_packet.as_mut().unwrap()(&mut opaque.data, buf)
    }

    pub unsafe extern "C" fn write_c<T: Send + Sync>(
        opaque: *mut c_void,
        data: *const u8,
        len: i32,
    ) -> i32 {
        let buf = unsafe { slice::from_raw_parts(data, len as usize) };
        let opaque = unsafe { (opaque as *mut Opaque<T>).as_mut() }.unwrap();
        opaque.write_packet.as_mut().unwrap()(&mut opaque.data, buf)
    }

    #[cfg(not(feature = "ffmpeg7"))]
    unsafe extern "C" fn write_c<T: Send + Sync>(
        opaque: *mut c_void,
        data: *mut u8,
        len: i32,
    ) -> i32 {
        let buf = unsafe { slice::from_raw_parts(data, len as usize) };
        let opaque = unsafe { (opaque as *mut Opaque<T>).as_mut() }.unwrap();
        opaque.write_packet.as_mut().unwrap()(&mut opaque.data, buf)
    }
    pub unsafe extern "C" fn seek_c<T: Send + Sync>(
        opaque: *mut c_void,
        offset: i64,
        whence: i32,
    ) -> i64 {
        let opaque = unsafe { (opaque as *mut Opaque<T>).as_mut() }.unwrap();
        opaque.seek.as_mut().unwrap()(&mut opaque.data, offset, whence)
    }
}

pub use opaque::{
    Opaque, ReadOpaqueCallback, ReadPacketCallback, SeekCallback, SeekOpaqueCallback,
    WriteOpaqueCallback, WritePacketCallback,
};

/// Custom [`AVIOContext`], used for custom IO.
pub struct AVIOContextCustom {
    inner: AVIOContext,
    _opaque: Box<Opaque<Vec<u8>>>,
}

impl Deref for AVIOContextCustom {
    type Target = AVIOContext;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for AVIOContextCustom {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl AVIOContextCustom {
    /// `write_flag` - set to `false` on read, set to `true` on write.
    pub fn alloc_context(
        mut buffer: AVMem,
        write_flag: bool,
        data: Vec<u8>,
        read_packet: Option<ReadPacketCallback>,
        write_packet: Option<WritePacketCallback>,
        seek: Option<SeekCallback>,
    ) -> Self {
        // According to the documentation of `avio_alloc_context`:
        //
        // `buffer`: Memory block for input/output operations via AVIOContext. The
        // buffer must be allocated with av_malloc() and friends. It may be freed
        // and replaced with a new buffer by libavformat.
        //
        // So this function accepts `AVMem` rather than ordinary `*mut u8`.

        let (read_packet_c, write_packet_c, seek_c) = {
            use opaque::{read_c, seek_c, write_c};
            (
                read_packet.is_some().then_some(read_c::<Vec<u8>> as _),
                // Note: If compiler errors here, you might have used wrong feature flag(ffmpeg6|ffmpeg7).
                write_packet.is_some().then_some(write_c::<Vec<u8>> as _),
                seek.is_some().then_some(seek_c::<Vec<u8>> as _),
            )
        };

        let mut opaque = Box::new(Opaque {
            data,
            read_packet,
            write_packet,
            seek,
        });

        // After reading the implementation, avio_alloc_context only fails on no
        // memory.
        let context = unsafe {
            ffi::avio_alloc_context(
                buffer.as_mut_ptr(),
                buffer.len as _,
                if write_flag { 1 } else { 0 },
                &mut *opaque as *mut _ as _,
                read_packet_c,
                write_packet_c,
                seek_c,
            )
        }
        .upgrade()
        .unwrap();

        // If `AVIOContext` allocation successes, buffer is transferred to
        // `AVIOContext::buffer`, so we don't call drop function of `AVMem`, later
        // it will be freed in `AVIOContext::drop`.
        let _ = buffer.into_raw();

        Self {
            inner: unsafe { AVIOContext::from_raw(context) },
            _opaque: opaque,
        }
    }

    /// Re-take the ownership of the `data` passed in `alloc_context`.
    /// The `data` inside this will be set to an empty vector.
    pub fn take_data(&mut self) -> Vec<u8> {
        std::mem::take(&mut self._opaque.data)
    }

    /// Get a mutable reference to the data inside this context.
    pub fn as_mut_data(&mut self) -> &mut Vec<u8> {
        &mut self._opaque.data
    }
}

impl Drop for AVIOContextCustom {
    fn drop(&mut self) {
        // Recover the `AVMem` fom the buffer and drop it. We don't attach the
        // AVMem to this type because according to the documentation, the buffer
        // pointer may be changed during it's usage.
        //
        // There is no need to change self.buffer to null because
        // avio_context_free is just `av_freep`.
        if let Some(buffer) = NonNull::new(self.buffer) {
            let _ = unsafe { AVMem::from_raw(buffer) };
        }
        unsafe { ffi::avio_context_free(&mut self.as_mut_ptr()) };
    }
}

// pub type ReadPacketCallback = Box<dyn FnMut(&mut Vec<u8>, &mut [u8]) -> i32 + Send + 'static>;
// pub type WritePacketCallback = Box<dyn FnMut(&mut Vec<u8>, &[u8]) -> i32 + Send + 'static>;
// pub type SeekCallback = Box<dyn FnMut(&mut Vec<u8>, i64, i32) -> i64 + Send + 'static>;

pub struct AVIOContextOpaque {
    inner: AVIOContext,
}

impl AVIOContextOpaque {
    pub fn alloc_context<T: Send + Sync>(
        mut buffer: AVMem,
        write_flag: bool,
        opaque: T,
        read_packet: Option<ReadOpaqueCallback<T>>,
        write_packet: Option<WriteOpaqueCallback<T>>,
        seek_packet: Option<SeekOpaqueCallback<T>>,
    ) -> Self {
        use opaque::{read_c, seek_c, write_c};

        let (read_c, write_c, seek_c) = {
            (
                read_packet.is_some().then_some(read_c::<T> as _),
                write_packet.is_some().then_some(write_c::<T> as _),
                seek_packet.is_some().then_some(seek_c::<T> as _),
            )
        };

        let opaque = Box::new(Opaque {
            data: opaque,
            read_packet,
            write_packet,
            seek: seek_packet,
        });
        let context = unsafe {
            ffi::avio_alloc_context(
                buffer.as_mut_ptr(),
                buffer.len as _,
                if write_flag { 1 } else { 0 },
                Box::into_raw(opaque) as *mut _ as _,
                read_c,
                write_c,
                seek_c,
            )
        }
        .upgrade()
        .unwrap();

        Self {
            inner: unsafe { AVIOContext::from_raw(context) },
        }
    }
}

impl Deref for AVIOContextOpaque {
    type Target = AVIOContext;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl std::ops::DerefMut for AVIOContextOpaque {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl Drop for AVIOContextOpaque {
    fn drop(&mut self) {
        // Recover the `AVMem` fom the buffer and drop it. We don't attach the
        // AVMem to this type because according to the documentation, the buffer
        // pointer may be changed during it's usage.
        //
        // There is no need to change self.buffer to null because
        // avio_context_free is just `av_freep`.
        if let Some(buffer) = NonNull::new(self.buffer) {
            let _ = unsafe { AVMem::from_raw(buffer) };
        }
        unsafe { ffi::avio_context_free(&mut self.as_mut_ptr()) };
    }
}

pub struct AVIOProtocol;

impl AVIOProtocol {
    /// Return the name of the protocol that will handle the passed URL.
    pub fn find_protocol_name(url: &CStr) -> Option<&'static CStr> {
        unsafe {
            ffi::avio_find_protocol_name(url.as_ptr())
                .upgrade()
                .map(|x| CStr::from_ptr(x.as_ptr()))
        }
    }

    /// Iterate through names of available output protocols.
    pub fn outputs() -> AVIOProtocolIter {
        AVIOProtocolIter {
            opaque: ptr::null_mut(),
            output: 1,
        }
    }

    /// Iterate through names of available input protocols.
    pub fn inputs() -> AVIOProtocolIter {
        AVIOProtocolIter {
            opaque: ptr::null_mut(),
            output: 0,
        }
    }
}

pub struct AVIOProtocolIter {
    opaque: *mut c_void,
    output: i32,
}

impl Iterator for AVIOProtocolIter {
    type Item = &'static CStr;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            ffi::avio_enum_protocols(&mut self.opaque, self.output)
                .upgrade()
                .map(|x| CStr::from_ptr(x.as_ptr()))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_iterate_output_protocols() {
        let outputs = AVIOProtocol::outputs()
            .map(|x| x.to_str().unwrap())
            .collect::<Vec<_>>();
        dbg!(&outputs);
        assert!(!outputs.is_empty());
        assert!(outputs.contains(&"file"));
        assert!(outputs.contains(&"http"));
        assert!(outputs.contains(&"rtmp"));
    }

    #[test]
    fn test_iterate_input_protocols() {
        let inputs = AVIOProtocol::inputs()
            .map(|x| x.to_str().unwrap())
            .collect::<Vec<_>>();
        dbg!(&inputs);
        assert!(!inputs.is_empty());
        assert!(inputs.contains(&"file"));
        assert!(inputs.contains(&"http"));
        assert!(inputs.contains(&"async"));
    }
}
