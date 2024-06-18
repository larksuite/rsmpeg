//! A module consists of Wrapper macros. These macros wrap ffi structs to custom
//! type with some convenient functions

/// Wrapping with XXX -> XXX mapping.
macro_rules! wrap_pure {
    (
        $(#[$meta:meta])*
        ($wrapped_type: ident): $ffi_type: ty
        $(,$attach: ident: $attach_type: ty = $attach_default: expr)*
    ) => {
        $(#[$meta])*
        pub struct $wrapped_type {
            something_should_not_be_touched_directly: std::ptr::NonNull<$ffi_type>,
            // Publicize the attachment, can be directly changed without deref_mut()
            $(pub $attach: $attach_type,)*
        }

        impl $wrapped_type {
            pub fn as_ptr(&self) -> *const $ffi_type {
                self.something_should_not_be_touched_directly.as_ptr() as *const _
            }

            pub fn as_mut_ptr(&mut self) -> *mut $ffi_type {
                self.something_should_not_be_touched_directly.as_ptr()
            }

            /// # Safety
            /// This function should only be called when the pointer is valid and
            /// the data it's pointing to can be dropped.
            pub unsafe fn set_ptr(&mut self, ptr: std::ptr::NonNull<$ffi_type>) {
                self.something_should_not_be_touched_directly = ptr;
            }

            /// # Safety
            /// This function should only be called when the pointer is valid and
            /// the data it's pointing to can be dropped.
            pub unsafe fn from_raw(raw: std::ptr::NonNull<$ffi_type>) -> Self {
                Self {
                    something_should_not_be_touched_directly: raw,
                    $($attach: $attach_default,)*
                }
            }

            pub fn into_raw(self) -> std::ptr::NonNull<$ffi_type> {
                let $wrapped_type {
                    something_should_not_be_touched_directly: raw,
                    $($attach: _,)*
                } = self;
                #[allow(clippy::forget_non_drop)]
                std::mem::forget(self);
                raw
            }
        }

        impl std::ops::Deref for $wrapped_type {
            type Target = $ffi_type;

            fn deref(&self) -> &Self::Target {
                unsafe { self.something_should_not_be_touched_directly.as_ref() }
            }
        }

        impl crate::shared::UnsafeDerefMut for $wrapped_type {
            unsafe fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { self.something_should_not_be_touched_directly.as_mut() }
            }
        }

        unsafe impl Send for $wrapped_type {}
    };
}

/// Wrapping with XXXRef -> XXX.
macro_rules! wrap_ref_pure {
    (($wrapped_type: ident, $wrapped_ref: ident): $ffi_type: ty) => {
        // This is needed for wrapping reference owned value from ffi
        #[repr(transparent)]
        pub struct $wrapped_ref<'a> {
            inner: std::mem::ManuallyDrop<$wrapped_type>,
            _marker: std::marker::PhantomData<&'a $wrapped_type>,
        }

        impl<'a> std::ops::Deref for $wrapped_ref<'a> {
            type Target = $wrapped_type;

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl<'a> std::ops::Drop for $wrapped_ref<'a> {
            fn drop(&mut self) {
                // Do nothing
            }
        }

        impl<'a> $wrapped_ref<'a> {
            /// # Safety
            /// This function should only be called when `raw` is valid and can
            /// be dropped. Please ensure its lifetime when used.
            pub unsafe fn from_raw(raw: std::ptr::NonNull<$ffi_type>) -> Self {
                Self {
                    inner: std::mem::ManuallyDrop::new(unsafe { $wrapped_type::from_raw(raw) }),
                    _marker: std::marker::PhantomData,
                }
            }
        }

        unsafe impl<'a> Send for $wrapped_ref<'a> {}
    };
}

/// Wrapping with XXXMut -> XXX.
macro_rules! wrap_mut_pure {
    (($wrapped_type: ident, $wrapped_mut: ident): $ffi_type: ty) => {
        // This is needed for wrapping mutable reference owned value from ffi
        #[repr(transparent)]
        pub struct $wrapped_mut<'a> {
            inner: std::mem::ManuallyDrop<$wrapped_type>,
            _marker: std::marker::PhantomData<&'a $wrapped_type>,
        }

        impl<'a> std::ops::Deref for $wrapped_mut<'a> {
            type Target = $wrapped_type;

            fn deref(&self) -> &Self::Target {
                &self.inner
            }
        }

        impl<'a> std::ops::DerefMut for $wrapped_mut<'a> {
            fn deref_mut(&mut self) -> &mut Self::Target {
                &mut self.inner
            }
        }

        impl<'a> std::ops::Drop for $wrapped_mut<'a> {
            fn drop(&mut self) {
                // Do nothing
            }
        }

        impl<'a> $wrapped_mut<'a> {
            /// # Safety
            /// This function should only be called when `raw` is valid and can
            /// be dropped. Please ensure its lifetime when used.
            #[must_use]
            pub unsafe fn from_raw(raw: std::ptr::NonNull<$ffi_type>) -> Self {
                Self {
                    inner: std::mem::ManuallyDrop::new(unsafe { $wrapped_type::from_raw(raw) }),
                    _marker: std::marker::PhantomData,
                }
            }
        }

        unsafe impl<'a> Send for $wrapped_mut<'a> {}
    };
}

/// Wrapping with XXXRef, XXXMut, XXX -> XXX.
macro_rules! wrap_ref_mut {
    (
        $(#[$meta:meta])*
        $name: ident: $ffi_type: ty
        $(,$attach: ident: $attach_type: ty = $attach_default: expr)* $(,)?
    ) => {
        paste::paste! {
            wrap_pure!($(#[$meta])* ($name): $ffi_type $(,$attach: $attach_type = $attach_default)*);
            wrap_ref_pure!(($name, [<$name Ref>]): $ffi_type);
            wrap_mut_pure!(($name, [<$name Mut>]): $ffi_type);
        }
    };
}

/// Wrapping with XXXRef, XXX -> XXX.
macro_rules! wrap_ref {
    (
        $(#[$meta:meta])*
        $name: ident: $ffi_type: ty
        $(,$attach: ident: $attach_type: ty = $attach_default: expr)* $(,)?
    ) => {
        paste::paste! {
            wrap_pure!($(#[$meta])* ($name): $ffi_type $(,$attach: $attach_type = $attach_default)*);
            wrap_ref_pure!(($name, [<$name Ref>]): $ffi_type);
        }
    };
}

/// Wrapping with XXXMut, XXX -> XXX.
macro_rules! wrap_mut {
    (
        $(#[$meta:meta])*
        $name: ident: $ffi_type: ty
        $(,$attach: ident: $attach_type: ty = $attach_default: expr)* $(,)?
    ) => {
        paste::paste! {
            wrap_pure!($(#[$meta])* ($name): $ffi_type $(,$attach: $attach_type = $attach_default)*);
            wrap_mut_pure!(($name, [<$name Mut>]): $ffi_type);
        }
    };
}

/// Wrapping with XXX -> XXX.
macro_rules! wrap {
    (
        $(#[$meta:meta])*
        $name: ident: $ffi_type: ty
        $(,$attach: ident: $attach_type: ty = $attach_default: expr)* $(,)?
    ) => {
        paste::paste! {
            wrap_pure!($(#[$meta])* ($name): $ffi_type $(,$attach: $attach_type = $attach_default)*);
        }
    };
}

/// Autogen single set function.
macro_rules! set_fn {
    ($impl_type:ident {
        $(
            ($fn_name:ident, $property:ident, $property_type:path)
        )+
    }) => {
        impl $impl_type {
            $(pub fn $fn_name(&mut self, $property: $property_type) {
                unsafe {
                    self.deref_mut().$property = $property;
                }
            })+
        }
    }
}

/// Autogen multiple set functions.
macro_rules! settable {
    ($impl_type:ident {
        $(
            $property:ident : $property_type:path
        ),+ $(,)?
    }) => {
        paste::paste! {
            set_fn!($impl_type {
                $(
                    ([<set_ $property>], $property, $property_type)
                )+
            });
        }
    };
}

#[cfg(test)]
#[allow(dead_code)]
mod test {
    use std::{ptr::NonNull, slice};
    #[test]
    fn test_attachment() {
        wrap!(PinStr: u8, len: usize = 0, capacity: usize = 0);

        impl PinStr {
            fn new(s: &str) -> Self {
                let buffer = s.to_owned().into_bytes();
                let len = buffer.len();
                let capacity = buffer.capacity();
                let buffer = buffer.leak().as_mut_ptr();
                let mut s = unsafe { Self::from_raw(NonNull::new(buffer).unwrap()) };
                s.len = len;
                s.capacity = capacity;
                s
            }

            fn to_str(&self) -> &str {
                unsafe {
                    std::str::from_utf8_unchecked(slice::from_raw_parts(self.as_ptr(), self.len))
                }
            }
        }

        impl Drop for PinStr {
            fn drop(&mut self) {
                let _ = unsafe { Vec::from_raw_parts(self.as_mut_ptr(), self.len, self.capacity) };
            }
        }

        let pin_str1 = PinStr::new("Hello, Indian mifans. Are you ok?");
        assert_eq!(pin_str1.to_str(), "Hello, Indian mifans. Are you ok?");
        let pin_str3 = {
            let pin_str2 = pin_str1;
            assert_eq!(pin_str2.to_str(), "Hello, Indian mifans. Are you ok?");
            pin_str2
        };
        assert_eq!(pin_str3.to_str(), "Hello, Indian mifans. Are you ok?");
    }
}
