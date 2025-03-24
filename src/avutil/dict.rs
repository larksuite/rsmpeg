use crate::{error::Result, ffi, shared::*};

use std::{
    ffi::{CStr, CString},
    os::raw::c_void,
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
        .upgrade()?;
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
            .upgrade()?;
        let result = unsafe { CStr::from_ptr(s).to_owned() };
        unsafe {
            ffi::av_freep(&mut s as *mut _ as *mut c_void);
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

    /// Iterates through all entries in the dictionary by reference.
    pub fn iter(&'dict self) -> AVDictionaryIter<'dict> {
        AVDictionaryIter {
            dict: self,
            ptr: ptr::null(),
            _phantom: std::marker::PhantomData,
        }
    }
}

impl Clone for AVDictionary {
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

impl<'dict> IntoIterator for &'dict AVDictionary {
    type IntoIter = AVDictionaryIter<'dict>;
    type Item = AVDictionaryEntryRef<'dict>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Iterator over [`AVDictionary`] by reference.
pub struct AVDictionaryIter<'dict> {
    dict: &'dict AVDictionary,
    ptr: *const ffi::AVDictionaryEntry,
    _phantom: std::marker::PhantomData<&'dict ()>,
}

impl<'dict> Iterator for AVDictionaryIter<'dict> {
    type Item = AVDictionaryEntryRef<'dict>;
    fn next(&mut self) -> Option<Self::Item> {
        self.ptr = unsafe { ffi::av_dict_iterate(self.dict.as_ptr(), self.ptr) };
        self.ptr
            .upgrade()
            .map(|x| unsafe { AVDictionaryEntryRef::from_raw(x) })
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

    #[test]
    fn set() {
        let _ = AVDictionary::new(c"bob", c"alice", 0);

        let _ = AVDictionary::new(c"bob", c"alice", 0)
            .set(c"a;dsjfadsfa", c"asdfjal;sdfj", 0)
            .set(c"foo", c"bar", 0);

        let _ = AVDictionary::new(c"bob", c"alice", 0).set(c"bob", c"alice", 0);
    }

    #[test]
    fn set_int() {
        let dict = AVDictionary::new_int(c"bob", 2233, 0).set_int(c"foo", 123456789123456789, 0);
        assert_eq!(
            c"123456789123456789".as_ref(),
            dict.get(c"foo", None, 0).unwrap().value()
        );
    }

    #[test]
    fn get() {
        let dict = AVDictionary::new(c"bob", c"alice", 0);
        assert_eq!(
            c"alice".as_ref(),
            dict.get(c"bob", None, 0).unwrap().value()
        );

        let dict = AVDictionary::new(c"bob", c"alice", 0)
            .set(c"bob", c"alice", 0)
            .set(c"bob", c"alice", 0)
            .set(c"bob", c"alice", 0)
            .set(c"bob", c"alice", 0);
        assert_eq!(
            c"alice".as_ref(),
            dict.get(c"bob", None, 0).unwrap().value()
        );

        let dict = AVDictionary::new(c"foo", c"bar", 0).set(c"bob", c"alice", 0);
        assert_eq!(c"bar".as_ref(), dict.get(c"foo", None, 0).unwrap().value());
        assert_eq!(
            c"alice".as_ref(),
            dict.get(c"bob", None, 0).unwrap().value()
        );

        // Find `foo` after after entry of `bob` will fail.
        let entry = dict.get(c"bob", None, 0).unwrap();
        assert_eq!(c"alice".as_ref(), entry.value());
        assert!(dict.get(c"foo", Some(entry), 0).is_none());

        // Shadowing.
        let dict = AVDictionary::new(c"bob", c"alice0", 0)
            .set(c"bob", c"alice1", 0)
            .set(c"bob", c"alice2", 0)
            .set(c"bob", c"alice3", 0)
            .set(c"bob", c"alice4", 0);

        let entry = dict.get(c"bob", None, 0).unwrap();
        assert_eq!(c"alice4".as_ref(), entry.value());
        assert_eq!(c"bob".as_ref(), entry.key());
        assert!(dict.get(c"bob", Some(entry), 0).is_none());
    }

    #[test]
    fn copy() {
        let dicta = AVDictionary::new(c"a", c"b", 0).set(c"c", c"d", 0);

        let dictc = dicta.clone();
        assert_eq!(c"a:b-c:d", dictc.get_string(b':', b'-').unwrap().as_c_str());

        let dictb = AVDictionary::new(c"foo", c"bar", 0)
            .set(c"alice", c"bob", 0)
            .copy(&dictc, 0);
        assert_eq!(
            c"foo:bar-alice:bob-a:b-c:d",
            dictb.get_string(b':', b'-').unwrap().as_c_str(),
        );

        let dicta = dicta.set(c"e", c"f", 0);

        assert_eq!(c"b".as_ref(), dicta.get(c"a", None, 0).unwrap().value());
        assert_eq!(c"d".as_ref(), dicta.get(c"c", None, 0).unwrap().value());
        assert_eq!(c"f".as_ref(), dicta.get(c"e", None, 0).unwrap().value());

        assert_eq!(c"b".as_ref(), dictb.get(c"a", None, 0).unwrap().value());
        assert_eq!(c"d".as_ref(), dictb.get(c"c", None, 0).unwrap().value());
        assert!(dictb.get(c"e", None, 0).is_none());
    }

    #[test]
    fn serialization() {
        let dict = AVDictionary::new(c"a", c"b", 0)
            .set(c"c", c"d", 0)
            .set(c"foo", c"bar", 0)
            .set(c"bob", c"alice", 0);
        assert_eq!(
            c"a:b-c:d-foo:bar-bob:alice",
            dict.get_string(b':', b'-').unwrap().as_c_str()
        );
        let dict = dict.set(c"rust", c"c", 0);
        assert_eq!(
            c"a:b-c:d-foo:bar-bob:alice-rust:c",
            dict.get_string(b':', b'-').unwrap().as_c_str()
        );
    }

    #[test]
    fn deserialization() {
        let dict =
            AVDictionary::from_string(c"a:b-c:d-foo:bar-bob:alice-rust:c", c":", c"-", 0).unwrap();
        assert_eq!(
            c"a:b-c:d-foo:bar-bob:alice-rust:c",
            dict.get_string(b':', b'-').unwrap().as_c_str()
        );
    }
}
