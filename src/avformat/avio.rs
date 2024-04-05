use std::{
    ffi::CStr,
    ops::Deref,
    ptr::{self, NonNull},
    slice,
};

use crate::{avutil::AVMem, error::*, ffi, shared::*};

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
    pub fn open(url: &CStr, flags: u32) -> Result<Self> {
        let mut io_context = ptr::null_mut();
        unsafe { ffi::avio_open(&mut io_context, url.as_ptr(), flags as _) }.upgrade()?;
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

/// Custom [`AVIOContext`], used for custom IO.
pub struct AVIOContextCustom {
    inner: AVIOContext,
    _opaque: Box<Opaque>,
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

pub type ReadPacketCallback = Box<dyn FnMut(&mut Vec<u8>, &mut [u8]) -> i32 + Send + 'static>;
pub type WritePacketCallback = Box<dyn FnMut(&mut Vec<u8>, &[u8]) -> i32 + Send + 'static>;
pub type SeekCallback = Box<dyn FnMut(&mut Vec<u8>, i64, i32) -> i64 + Send + 'static>;

pub struct Opaque {
    data: Vec<u8>,
    read_packet: Option<ReadPacketCallback>,
    write_packet: Option<WritePacketCallback>,
    seek: Option<SeekCallback>,
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
            use std::os::raw::c_void;
            // Function is called when the function is given and opaque is not null.
            unsafe extern "C" fn read_c(opaque: *mut c_void, data: *mut u8, len: i32) -> i32 {
                let buf = unsafe { slice::from_raw_parts_mut(data, len as usize) };
                let opaque = unsafe { (opaque as *mut Opaque).as_mut() }.unwrap();
                opaque.read_packet.as_mut().unwrap()(&mut opaque.data, buf)
            }
            #[cfg(not(feature = "ffmpeg7"))]
            unsafe extern "C" fn write_c(opaque: *mut c_void, data: *mut u8, len: i32) -> i32 {
                let buf = unsafe { slice::from_raw_parts(data, len as usize) };
                let opaque = unsafe { (opaque as *mut Opaque).as_mut() }.unwrap();
                opaque.write_packet.as_mut().unwrap()(&mut opaque.data, buf)
            }
            #[cfg(feature = "ffmpeg7")]
            unsafe extern "C" fn write_c(opaque: *mut c_void, data: *const u8, len: i32) -> i32 {
                let buf = unsafe { slice::from_raw_parts(data, len as usize) };
                let opaque = unsafe { (opaque as *mut Opaque).as_mut() }.unwrap();
                opaque.write_packet.as_mut().unwrap()(&mut opaque.data, buf)
            }
            unsafe extern "C" fn seek_c(opaque: *mut c_void, offset: i64, whence: i32) -> i64 {
                let opaque = unsafe { (opaque as *mut Opaque).as_mut() }.unwrap();
                opaque.seek.as_mut().unwrap()(&mut opaque.data, offset, whence)
            }

            (
                read_packet.is_some().then_some(read_c as _),
                // Note: If compiler errors here, you might have used wrong feature flag(ffmpeg6|ffmpeg7).
                write_packet.is_some().then_some(write_c as _),
                seek.is_some().then_some(seek_c as _),
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
