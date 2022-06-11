use crate::{
    error::{Result, RsmpegError},
    ffi,
    shared::*,
};

use std::{
    ffi::{CStr, CString},
    ops::Drop,
    ptr::{self, NonNull},
};

wrap_ref_mut!(AVDictionary: ffi::AVDictionary);

impl AVDictionary {
    /// Create a dictionary while calling `set()`.
    pub fn new(key: &CStr, value: &CStr, flags: u32) -> Self {
        // Since AVDictionary is a non-null pointer to ffi::AVDictionary.
        // Without a new macro `wrap_nullable`, we cannot new a Self containing
        // null pointer.
        let mut dict = ptr::null_mut();
        unsafe { ffi::av_dict_set(&mut dict, key.as_ptr(), value.as_ptr(), flags as i32) }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(NonNull::new(dict).unwrap()) }
    }

    /// Create a dictionary while calling `set_int()`.
    pub fn new_int(key: &CStr, value: i64, flags: u32) -> Self {
        let mut dict = ptr::null_mut();
        unsafe { ffi::av_dict_set_int(&mut dict, key.as_ptr(), value, flags as i32) }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(NonNull::new(dict).unwrap()) }
    }

    // Create a dictionary while calling `Self::parse_string()`.
    pub fn from_string(
        str: &CStr,
        key_val_sep: &CStr,
        pairs_sep: &CStr,
        flags: u32,
    ) -> Option<Self> {
        let mut dict = ptr::null_mut();
        unsafe {
            ffi::av_dict_parse_string(
                &mut dict,
                str.as_ptr(),
                key_val_sep.as_ptr(),
                pairs_sep.as_ptr(),
                flags as i32,
            )
        }
        .upgrade()
        .ok()?;
        Some(unsafe { Self::from_raw(NonNull::new(dict).unwrap()) })
    }

    /// The set function is so strange is because adding a new entry to
    /// AVDictionary invalidates all existing entries.... So this functions
    /// consumes itself.
    pub fn set(mut self, key: &CStr, value: &CStr, flags: u32) -> Self {
        let mut dict = self.as_mut_ptr();
        // Only error on AVERROR_ENOMEM, so unwrap
        unsafe { ffi::av_dict_set(&mut dict, key.as_ptr(), value.as_ptr(), flags as i32) }
            .upgrade()
            .unwrap();
        unsafe { self.set_ptr(NonNull::new(dict).unwrap()) };
        self
    }

    /// Similar to the `set` function.
    pub fn set_int(mut self, key: &CStr, value: i64, flags: u32) -> Self {
        let mut dict = self.as_mut_ptr();
        // Only error on AVERROR_ENOMEM, so unwrap
        unsafe { ffi::av_dict_set_int(&mut dict, key.as_ptr(), value, flags as i32) }
            .upgrade()
            .unwrap();
        unsafe { self.set_ptr(NonNull::new(dict).unwrap()) };
        self
    }

    /// Parse the key/value pairs list and add the parsed entries to a
    /// dictionary.
    pub fn parse_string(
        mut self,
        str: &CStr,
        key_val_sep: &CStr,
        pairs_sep: &CStr,
        flags: u32,
    ) -> Result<Self> {
        let mut dict = self.as_mut_ptr();
        unsafe {
            ffi::av_dict_parse_string(
                &mut dict,
                str.as_ptr(),
                key_val_sep.as_ptr(),
                pairs_sep.as_ptr(),
                flags as i32,
            )
        }
        .upgrade()
        .map_err(RsmpegError::DictionaryParseError)?;
        unsafe { self.set_ptr(NonNull::new(dict).unwrap()) };
        Ok(self)
    }

    /// Copy entries from one AVDictionary struct into self.
    pub fn copy(mut self, another: &AVDictionary, flags: u32) -> Self {
        let mut dict = self.as_mut_ptr();
        // Only error on AVERROR_ENOMEM, so unwrap
        unsafe { ffi::av_dict_copy(&mut dict, another.as_ptr(), flags as i32) }
            .upgrade()
            .unwrap();
        unsafe { self.set_ptr(NonNull::new(dict).unwrap()) };
        self
    }

    /// Get dictionary entries as a string.
    ///
    /// Create a string containing dictionary's entries.
    /// Such string may be passed back to `Self::parse_string()`.
    pub fn get_string(&self, key_val_sep: u8, pairs_sep: u8) -> Result<CString> {
        let mut s = ptr::null_mut();
        unsafe { ffi::av_dict_get_string(self.as_ptr(), &mut s, key_val_sep as _, pairs_sep as _) }
            .upgrade()
            .map_err(RsmpegError::DictionaryGetStringError)?;
        let result = unsafe { CStr::from_ptr(s).to_owned() };
        unsafe {
            ffi::av_freep(&mut s as *mut _ as *mut libc::c_void);
        }
        Ok(result)
    }
}

impl<'dict> AVDictionary {
    /// Get a dictionary entry with matching key.
    ///
    /// The returned entry key or value must not be changed, or it will
    /// cause undefined behavior.
    ///
    /// To iterate through all the dictionary entries, you can set the matching key
    /// to the null string "" and set the AV_DICT_IGNORE_SUFFIX flag.
    pub fn get(
        &'dict self,
        key: &CStr,
        prev: Option<AVDictionaryEntryRef>,
        flags: u32,
    ) -> Option<AVDictionaryEntryRef<'dict>> {
        let prev_ptr = match prev {
            Some(entry) => entry.as_ptr(),
            None => ptr::null(),
        };
        unsafe { ffi::av_dict_get(self.as_ptr(), key.as_ptr(), prev_ptr, flags as i32) }
            .upgrade()
            .map(|ptr| unsafe { AVDictionaryEntryRef::from_raw(ptr) })
    }
}

impl std::clone::Clone for AVDictionary {
    /// Similar to `Self::copy()`, while set the copy flag to `0`.
    fn clone(&self) -> Self {
        let mut newer = ptr::null_mut();
        unsafe { ffi::av_dict_copy(&mut newer, self.as_ptr(), 0) }
            .upgrade()
            .unwrap();
        unsafe { Self::from_raw(NonNull::new(newer).unwrap()) }
    }
}

impl Drop for AVDictionary {
    fn drop(&mut self) {
        let mut dict = self.as_mut_ptr();
        unsafe { ffi::av_dict_free(&mut dict) }
    }
}

wrap_ref_mut!(AVDictionaryEntry: ffi::AVDictionaryEntry);

impl AVDictionaryEntry {
    pub fn key(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.key) }
    }

    pub fn value(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.value) }
    }
}

#[cfg(test)]
mod test {
    use super::AVDictionary;
    use cstr::cstr;

    #[test]
    fn set() {
        let _ = AVDictionary::new(cstr!("bob"), cstr!("alice"), 0);

        let _ = AVDictionary::new(cstr!("bob"), cstr!("alice"), 0)
            .set(cstr!("a;dsjfadsfa"), cstr!("asdfjal;sdfj"), 0)
            .set(cstr!("foo"), cstr!("bar"), 0);

        let _ =
            AVDictionary::new(cstr!("bob"), cstr!("alice"), 0).set(cstr!("bob"), cstr!("alice"), 0);
    }

    #[test]
    fn set_int() {
        let dict = AVDictionary::new_int(cstr!("bob"), 2233, 0).set_int(
            cstr!("foo"),
            123456789123456789,
            0,
        );
        assert_eq!(
            cstr!("123456789123456789").as_ref(),
            dict.get(cstr!("foo"), None, 0).unwrap().value()
        );
    }

    #[test]
    fn get() {
        let dict = AVDictionary::new(cstr!("bob"), cstr!("alice"), 0);
        assert_eq!(
            cstr!("alice").as_ref(),
            dict.get(cstr!("bob"), None, 0).unwrap().value()
        );

        let dict = AVDictionary::new(cstr!("bob"), cstr!("alice"), 0)
            .set(cstr!("bob"), cstr!("alice"), 0)
            .set(cstr!("bob"), cstr!("alice"), 0)
            .set(cstr!("bob"), cstr!("alice"), 0)
            .set(cstr!("bob"), cstr!("alice"), 0);
        assert_eq!(
            cstr!("alice").as_ref(),
            dict.get(cstr!("bob"), None, 0).unwrap().value()
        );

        let dict =
            AVDictionary::new(cstr!("foo"), cstr!("bar"), 0).set(cstr!("bob"), cstr!("alice"), 0);
        assert_eq!(
            cstr!("bar").as_ref(),
            dict.get(cstr!("foo"), None, 0).unwrap().value()
        );
        assert_eq!(
            cstr!("alice").as_ref(),
            dict.get(cstr!("bob"), None, 0).unwrap().value()
        );

        // Find `foo` after after entry of `bob` will fail.
        let entry = dict.get(cstr!("bob"), None, 0).unwrap();
        assert_eq!(cstr!("alice").as_ref(), entry.value());
        assert!(dict.get(cstr!("foo"), Some(entry), 0).is_none());

        // Shadowing.
        let dict = AVDictionary::new(cstr!("bob"), cstr!("alice0"), 0)
            .set(cstr!("bob"), cstr!("alice1"), 0)
            .set(cstr!("bob"), cstr!("alice2"), 0)
            .set(cstr!("bob"), cstr!("alice3"), 0)
            .set(cstr!("bob"), cstr!("alice4"), 0);

        let entry = dict.get(cstr!("bob"), None, 0).unwrap();
        assert_eq!(cstr!("alice4").as_ref(), entry.value());
        assert_eq!(cstr!("bob").as_ref(), entry.key());
        assert!(dict.get(cstr!("bob"), Some(entry), 0).is_none());
    }

    #[test]
    fn copy() {
        let dicta = AVDictionary::new(cstr!("a"), cstr!("b"), 0).set(cstr!("c"), cstr!("d"), 0);

        let dictc = dicta.clone();
        assert_eq!(
            cstr!("a:b-c:d"),
            dictc.get_string(b':', b'-').unwrap().as_c_str()
        );

        let dictb = AVDictionary::new(cstr!("foo"), cstr!("bar"), 0)
            .set(cstr!("alice"), cstr!("bob"), 0)
            .copy(&dictc, 0);
        assert_eq!(
            cstr!("foo:bar-alice:bob-a:b-c:d"),
            dictb.get_string(b':', b'-').unwrap().as_c_str(),
        );

        let dicta = dicta.set(cstr!("e"), cstr!("f"), 0);

        assert_eq!(
            cstr!("b").as_ref(),
            dicta.get(cstr!("a"), None, 0).unwrap().value()
        );
        assert_eq!(
            cstr!("d").as_ref(),
            dicta.get(cstr!("c"), None, 0).unwrap().value()
        );
        assert_eq!(
            cstr!("f").as_ref(),
            dicta.get(cstr!("e"), None, 0).unwrap().value()
        );

        assert_eq!(
            cstr!("b").as_ref(),
            dictb.get(cstr!("a"), None, 0).unwrap().value()
        );
        assert_eq!(
            cstr!("d").as_ref(),
            dictb.get(cstr!("c"), None, 0).unwrap().value()
        );
        assert!(dictb.get(cstr!("e"), None, 0).is_none());
    }

    #[test]
    fn serialization() {
        let dict = AVDictionary::new(cstr!("a"), cstr!("b"), 0)
            .set(cstr!("c"), cstr!("d"), 0)
            .set(cstr!("foo"), cstr!("bar"), 0)
            .set(cstr!("bob"), cstr!("alice"), 0);
        assert_eq!(
            cstr!("a:b-c:d-foo:bar-bob:alice"),
            dict.get_string(b':', b'-').unwrap().as_c_str()
        );
        let dict = dict.set(cstr!("rust"), cstr!("c"), 0);
        assert_eq!(
            cstr!("a:b-c:d-foo:bar-bob:alice-rust:c"),
            dict.get_string(b':', b'-').unwrap().as_c_str()
        );
    }

    #[test]
    fn deserialization() {
        let dict = AVDictionary::from_string(
            cstr!("a:b-c:d-foo:bar-bob:alice-rust:c"),
            cstr!(":"),
            cstr!("-"),
            0,
        )
        .unwrap();
        assert_eq!(
            cstr!("a:b-c:d-foo:bar-bob:alice-rust:c"),
            dict.get_string(b':', b'-').unwrap().as_c_str()
        );
    }
}
