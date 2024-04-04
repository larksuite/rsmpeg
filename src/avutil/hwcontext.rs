use super::{AVBufferRef, AVDictionary, AVFrame};
use crate::{
    error::Result,
    ffi,
    shared::{PointerUpgrade, RetUpgrade},
};
use std::{
    ffi::CStr,
    ops::{Deref, DerefMut},
    os::raw::c_int,
    ptr::{self, NonNull},
};

#[repr(transparent)]
pub struct AVHWDeviceContext {
    buffer_ref: AVBufferRef,
}

impl AVHWDeviceContext {
    /// Allocate an [`AVHWDeviceContext`] for a given hardware type.
    pub fn alloc(r#type: ffi::AVHWDeviceType) -> Self {
        let buffer_ref = unsafe { ffi::av_hwdevice_ctx_alloc(r#type) };
        // this only panic on OOM
        let buffer_ref = buffer_ref.upgrade().unwrap();
        Self {
            buffer_ref: unsafe { AVBufferRef::from_raw(buffer_ref) },
        }
    }

    /// Finalize the device context before use. This function must be called after
    /// the context is filled with all the required information and before it is
    /// used in any way.
    pub fn init(&mut self) -> Result<()> {
        unsafe { ffi::av_hwdevice_ctx_init(self.buffer_ref.as_mut_ptr()) }.upgrade()?;
        Ok(())
    }

    /// Open a device of the specified type and create an [`AVHWDeviceContext`] for it.
    ///
    /// This is a convenience function intended to cover the simple cases. Callers
    /// who need to fine-tune device creation/management should open the device
    /// manually and then wrap it in an [`AVHWDeviceContext`] using
    /// [`Self::alloc()`] / [`Self::init()`].
    ///
    /// The returned context is already initialized and ready for use, the caller
    /// should not call [`Self::init()`] on it. The user_opaque/free fields of
    /// the created [`AVHWDeviceContext`] are set by this function and should not be
    /// touched by the caller.
    pub fn create(
        r#type: ffi::AVHWDeviceType,
        device: Option<&CStr>,
        opts: Option<&AVDictionary>,
        flags: c_int,
    ) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let opts = opts.map(|opts| opts.as_ptr()).unwrap_or_else(ptr::null);
        let device = device
            .map(|device| device.as_ptr())
            .unwrap_or_else(ptr::null);
        unsafe { ffi::av_hwdevice_ctx_create(&mut ptr, r#type, device, opts as *mut _, flags) }
            .upgrade()?;
        // this won't panic since av_hwdevice_ctx_create ensures it's non-null if successful.
        let ptr = ptr.upgrade().unwrap();
        let buffer_ref = unsafe { AVBufferRef::from_raw(ptr) };
        Ok(Self { buffer_ref })
    }

    /// Create a new device of the specified type from an existing device.
    ///
    /// If the source device is a device of the target type or was originally
    /// derived from such a device (possibly through one or more intermediate
    /// devices of other types), then this will return a reference to the
    /// existing device of the same type as is requested.
    ///
    /// Otherwise, it will attempt to derive a new device from the given source
    /// device.  If direct derivation to the new type is not implemented, it will
    /// attempt the same derivation from each ancestor of the source device in
    /// turn looking for an implemented derivation method.
    pub fn create_derived(&self, r#type: ffi::AVHWDeviceType) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        // `flags` parameter of av_hwdevice_ctx_create_derived is unused and need to be set to 0
        unsafe {
            ffi::av_hwdevice_ctx_create_derived(&mut ptr, r#type, self.as_ptr() as *mut _, 0)
        }
        .upgrade()?;
        // this won't panic since av_hwdevice_ctx_create_derived ensures it's non-null if successful.
        let ptr = ptr.upgrade().unwrap();
        Ok(Self {
            buffer_ref: unsafe { AVBufferRef::from_raw(ptr) },
        })
    }

    /// Create a new device of the specified type from an existing device.
    ///
    /// This function performs the same action as av_hwdevice_ctx_create_derived,
    /// however, it is able to set options for the new device to be derived.
    pub fn create_derived_opts(
        &self,
        r#type: ffi::AVHWDeviceType,
        options: Option<&AVDictionary>,
    ) -> Result<Self> {
        let mut ptr = ptr::null_mut();
        let options = options.map(|opts| opts.as_ptr()).unwrap_or_else(ptr::null);
        // `flags` parameter of av_hwdevice_ctx_create_derived is unused and need to be set to 0
        unsafe {
            ffi::av_hwdevice_ctx_create_derived_opts(
                &mut ptr,
                r#type,
                self.as_ptr() as *mut _,
                options as *mut _,
                0,
            )
        }
        .upgrade()?;
        // this won't panic since av_hwdevice_ctx_create_derived_opts ensures it's non-null if successful.
        let ptr = ptr.upgrade().unwrap();
        Ok(Self {
            buffer_ref: unsafe { AVBufferRef::from_raw(ptr) },
        })
    }

    /// Allocate an [`AVHWFramesContext`] tied to a given device context.
    pub fn hwframe_ctx_alloc(&self) -> AVHWFramesContext {
        let buffer_ref = unsafe {
            ffi::av_hwframe_ctx_alloc(self.as_ptr() as *mut _)
                .upgrade()
                .unwrap()
        };
        AVHWFramesContext {
            buffer_ref: unsafe { AVBufferRef::from_raw(buffer_ref) },
        }
    }

    /// Consume self and get the underlying buffer ref.
    pub fn into_inner(self) -> AVBufferRef {
        self.buffer_ref
    }
}

impl Deref for AVHWDeviceContext {
    type Target = AVBufferRef;

    fn deref(&self) -> &Self::Target {
        &self.buffer_ref
    }
}

impl DerefMut for AVHWDeviceContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer_ref
    }
}

/// This struct describes a set or pool of "hardware" frames (i.e. those with
/// data not located in normal system memory). All the frames in the pool are
/// assumed to be allocated in the same way and interchangeable.
///
/// This struct is reference-counted with the AVBuffer mechanism and tied to a
/// given AVHWDeviceContext instance. The av_hwframe_ctx_alloc() constructor
/// yields a reference, whose data field points to the actual AVHWFramesContext
/// struct.
#[derive(Clone)]
#[repr(transparent)]
pub struct AVHWFramesContext {
    pub(crate) buffer_ref: AVBufferRef,
}

// Here we manually use `wrap_ref_pure` and `wrap_mut_pure` for owned-reference type which can be used by `AVCodecContext::hw_frames_ctx*`.
//
// We want type safety, which means we cannot add methods of AVHWFramesContext to AVBufferRef directly.
wrap_ref_pure!((AVHWFramesContext, AVHWFramesContextRef): ffi::AVBufferRef);
wrap_mut_pure!((AVHWFramesContext, AVHWFramesContextMut): ffi::AVBufferRef);

impl AVHWFramesContext {
    /// Finalize the context before use. This function must be called after the
    /// context is filled with all the required information and before it is attached
    /// to any frames.
    pub fn init(&mut self) -> Result<()> {
        unsafe { ffi::av_hwframe_ctx_init(self.buffer_ref.as_mut_ptr()) }.upgrade()?;
        Ok(())
    }

    /// Return the mutable reference of the underlying AVHWFramesContext.
    pub fn data(&mut self) -> &mut ffi::AVHWFramesContext {
        unsafe { &mut *(self.buffer_ref.data as *mut ffi::AVHWFramesContext) }
    }

    /// Allocate a new frame attached to the current AVHWFramesContext.
    ///
    /// `frame`: an empty (freshly allocated or unreffed) frame to be filled with newly allocated buffers.
    pub fn get_buffer(&mut self, frame: &mut AVFrame) -> Result<()> {
        unsafe { ffi::av_hwframe_get_buffer(self.buffer_ref.as_mut_ptr(), frame.as_mut_ptr(), 0) }
            .upgrade()?;
        Ok(())
    }

    /// # Safety
    ///
    /// This function is only save when given `raw` points to a valid AVHWFramesContext.
    pub unsafe fn from_raw(raw: NonNull<ffi::AVBufferRef>) -> Self {
        Self {
            buffer_ref: unsafe { AVBufferRef::from_raw(raw) },
        }
    }

    /// Consume self and get the underlying buffer ref.
    pub fn into_inner(self) -> AVBufferRef {
        self.buffer_ref
    }
}

impl Deref for AVHWFramesContext {
    type Target = AVBufferRef;

    fn deref(&self) -> &Self::Target {
        &self.buffer_ref
    }
}

impl DerefMut for AVHWFramesContext {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.buffer_ref
    }
}
