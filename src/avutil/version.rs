
use crate::ffi;

use std::{
    ffi::{c_uint, CStr},
    fmt::Display,
};

/// Version information decoded from `av*_version()`
/// 
/// See FFmpeg documentation for more details: <https://ffmpeg.org/doxygen/trunk/group__version__utils.html>
#[derive(Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Clone, Copy)]
pub struct AvVersion {
    pub major: u8,
    pub minor: u8,
    pub micro: u8,
}

impl AvVersion {
    pub const fn new(major: u8, minor: u8, micro: u8) -> Self {
        Self {
            major, minor, micro
        }
    }

    pub const fn from_av_int(version: c_uint) -> Self {
        // Note: libavutil defines macros that uses bitmasks and shifts to extract each byte.
        // Extracting the bytes manually like this makes the intent of the code more clear, and nobody should be calling this function so much that any performance hit would matter.

        // We use little-endian to avoid potential issues on systems where c_uint is not exactly 4 bytes.
        let bytes = version.to_le_bytes();
        Self {
            major: bytes[2],
            minor: bytes[1],
            micro: bytes[0],
        }
    }


    pub const fn to_av_int(self) -> c_uint {
        let mut bytes = [0u8; std::mem::size_of::<c_uint>()];
        bytes[2] = self.major;
        bytes[1] = self.minor;
        bytes[0] = self.micro;

        c_uint::from_le_bytes(bytes)
    }
}

impl Display for AvVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.micro)
    }
}

macro_rules! _impl_version {
    ($modname:ident) => {
        paste::paste! {
            #[doc = r" Descriptive semver version of the `lib" $modname r"` that ffi bindings were generated against."]
            ///
            /// NOTE: This is not necessarily the same as what is linked into the executable, using [`version`] is preferred.
            pub const VERSION_STATIC: crate::avutil::AvVersion = crate::avutil::AvVersion::new(
                crate::ffi::[< LIB $modname:upper _VERSION_MAJOR >] as u8,
                crate::ffi::[< LIB $modname:upper _VERSION_MINOR >] as u8,
                crate::ffi::[< LIB $modname:upper _VERSION_MICRO >] as u8,
            );

            #[doc = r" Returns a semver version of the `lib" $modname r"` that has been linked in the executable (dynamically or statically)."]
            /// 
            /// NOTE: This is not the same as the version of FFmpeg, see [`avutil::version_info`].
            /// 
            /// # Examples
            /// ```
            #[doc = r" use rsmpeg::{" $modname r", ffi};"]
            /// 
            #[doc = r" let version = " $modname "::version();"]
            /// assert_ne!(version.to_av_int(), 0);
            /// // prints e.g. "59.39.100"
            /// println!("{}", version);
            /// ```
            pub fn version() -> crate::avutil::AvVersion {
                crate::avutil::AvVersion::from_av_int(unsafe { crate::ffi::[< $modname _version >]() })
            }

            #[doc = r" Returns the license of the `lib" $modname r" that has been linked in the executable (dynamically or statically)."]
            ///
            /// # Examples
            /// ```
            #[doc = r" use rsmpeg::" $modname r";"]
            /// 
            #[doc = r" let license = " $modname "::license();"]
            /// let license = license.to_string_lossy();
            /// assert!(license.contains("GPL"));
            /// ```
            pub fn license() -> &'static core::ffi::CStr {
                unsafe { core::ffi::CStr::from_ptr(crate::ffi::[< $modname _license >]()) }
            }
        }
    }
}

pub (crate) use _impl_version as impl_version;



/// Return an informative version string.
/// 
/// This usually is the actual release version number or a git commit description. This string has no fixed format and can change any time. It should never be parsed by code.
/// 
/// # Examples
/// ```
/// use rsmpeg::avutil::version_info;
/// 
/// let version_info = version_info();
/// let version_info = version_info.to_string_lossy();
/// assert_ne!(version_info.len(), 0);
/// // prints e.g. "7.1.1"
/// println!("{}", version_info);
/// ```
pub fn version_info() -> &'static CStr {
    unsafe { CStr::from_ptr(ffi::av_version_info()) }
}


#[cfg(test)]
mod tests {
    use crate::avutil::{AvVersion};
    use core::ffi::c_uint;

    #[test]
    fn test_avversion_semver_order() {
        let a = AvVersion {major: 100, minor: 100, micro: 100};

        // Major overrules minor and micro
        assert!(a < AvVersion{major: 101, minor: 0, micro: 0});
        assert!(a > AvVersion{major: 99, minor: 255, micro: 255});

        // Minor overrules micro
        assert!(a < AvVersion{major: 100, minor: 101, micro: 0});
        assert!(a > AvVersion{major: 100, minor: 99, micro: 255});

        // Micro is not ignored
        assert!(a < AvVersion{major: 100, minor: 100, micro: 101});
        assert!(a > AvVersion{major: 100, minor: 100, micro: 99});
    }

    #[test]
    fn test_avversion_decode() {
        // Taken from avformat 61.7.100
        let av_int: c_uint = 3999588;
        let version = AvVersion::from_av_int(av_int);

        assert_eq!(version, AvVersion {major:  61, minor: 7, micro: 100});
        assert_eq!(version.to_av_int(), av_int);
    }
}