use crate::{
    error::Result,
    ffi,
    shared::{PointerUpgrade, RetUpgrade},
};
use std::{
    ffi::{CStr, CString},
    mem::MaybeUninit,
    os::raw::c_void,
    ptr::NonNull,
};

wrap_ref!(AVChannelLayout: ffi::AVChannelLayout);

impl Drop for AVChannelLayout {
    fn drop(&mut self) {
        let layout = self.as_mut_ptr();
        unsafe { ffi::av_channel_layout_uninit(layout) };
        let _ = unsafe { Box::from_raw(layout) };
    }
}

impl Clone for AVChannelLayout {
    fn clone(&self) -> Self {
        let mut layout = MaybeUninit::<ffi::AVChannelLayout>::uninit();
        // unwrap: this function only fail on OOM.
        unsafe { ffi::av_channel_layout_copy(layout.as_mut_ptr(), self.as_ptr()) }
            .upgrade()
            .unwrap();
        let layout = unsafe { layout.assume_init() };
        unsafe { Self::from_raw(NonNull::new(Box::into_raw(Box::new(layout))).unwrap()) }
    }
}

impl AVChannelLayout {
    /// Convert self into [`ffi::AVChannelLayout`]`.
    ///
    /// Be careful when using it. Since this fucntion leaks the raw type,
    /// you have to manually do `ffi::av_channel_layout_uninit``.
    pub fn into_inner(mut self) -> ffi::AVChannelLayout {
        let layout = self.as_mut_ptr();
        let layout = *unsafe { Box::from_raw(layout) };
        std::mem::forget(self);
        layout
    }

    /// Initialize a native channel layout from a bitmask indicating which channels are present.
    pub fn from_mask(mask: u64) -> Option<Self> {
        let mut layout = MaybeUninit::<ffi::AVChannelLayout>::uninit();
        if unsafe { ffi::av_channel_layout_from_mask(layout.as_mut_ptr(), mask) } == 0 {
            let layout = unsafe { layout.assume_init() };
            Some(unsafe { Self::from_raw(NonNull::new(Box::into_raw(Box::new(layout))).unwrap()) })
        } else {
            None
        }
    }

    /// Initialize a channel layout from a given string description.
    /// The input string can be represented by:
    /// - the formal channel layout name (returned by av_channel_layout_describe())
    /// - single or multiple channel names (returned by av_channel_name(), eg. "FL",
    ///   or concatenated with "+", each optionally containing a custom name after
    ///   a "@", eg. "FL@Left+FR@Right+LFE")
    /// - a decimal or hexadecimal value of a native channel layout (eg. "4" or "0x4")
    /// - the number of channels with default layout (eg. "4c")
    /// - the number of unordered channels (eg. "4C" or "4 channels")
    /// - the ambisonic order followed by optional non-diegetic channels (eg.
    ///   "ambisonic 2+stereo")
    pub fn from_string(str: &CStr) -> Option<Self> {
        let mut layout = MaybeUninit::<ffi::AVChannelLayout>::uninit();
        if unsafe { ffi::av_channel_layout_from_string(layout.as_mut_ptr(), str.as_ptr()) } == 0 {
            let layout = unsafe { layout.assume_init() };
            Some(unsafe { Self::from_raw(NonNull::new(Box::into_raw(Box::new(layout))).unwrap()) })
        } else {
            None
        }
    }

    /// Get the default channel layout for a given number of channels.
    pub fn from_nb_channels(nb_channels: i32) -> Self {
        let mut layout = MaybeUninit::<ffi::AVChannelLayout>::uninit();
        unsafe { ffi::av_channel_layout_default(layout.as_mut_ptr(), nb_channels) }
        let layout = unsafe { layout.assume_init() };
        unsafe { Self::from_raw(NonNull::new(Box::into_raw(Box::new(layout))).unwrap()) }
    }

    /// Make a copy of a channel layout. This differs from just assigning src to dst
    /// in that it allocates and copies the map for AV_CHANNEL_ORDER_CUSTOM.
    pub fn copy(&mut self, src: &Self) {
        // unwrap: this function only fail on OOM.
        unsafe { ffi::av_channel_layout_copy(self.as_mut_ptr(), src.as_ptr()) }
            .upgrade()
            .unwrap();
    }

    /// Get a human-readable string describing the channel layout properties.
    /// The string will be in the same format that is accepted by
    /// [`AVChannelLayout::from_string`], allowing to rebuild the same
    /// channel layout, except for opaque pointers.
    pub fn describe(&self) -> Result<CString> {
        const BUF_SIZE: usize = 32;
        let mut buf = vec![0u8; BUF_SIZE];

        // # Safety: `as usize` after upgrading, len is assumed to be positive.
        let len = unsafe {
            ffi::av_channel_layout_describe(
                self.as_ptr(),
                buf.as_mut_ptr() as *mut std::ffi::c_char,
                BUF_SIZE,
            )
        }
        .upgrade()? as usize;

        let len = if len > BUF_SIZE {
            buf.resize(len, 0);
            unsafe {
                ffi::av_channel_layout_describe(
                    self.as_ptr(),
                    buf.as_mut_ptr() as *mut std::ffi::c_char,
                    len,
                )
            }
            .upgrade()? as usize
        } else {
            len
        };
        Ok(CString::new(&buf[..len - 1]).unwrap())
    }

    /// Get the channel with the given index in a channel layout.
    ///
    /// Return `None` if idx is not valid or the channel order is unspecified
    pub fn channel_from_index(&self, idx: u32) -> Option<ffi::AVChannel> {
        let channel = unsafe { ffi::av_channel_layout_channel_from_index(self.as_ptr(), idx) };
        (channel != ffi::AV_CHAN_NONE).then_some(channel)
    }

    /// Get the index of a given channel in a channel layout. In case multiple
    /// channels are found, only the first match will be returned.
    ///
    /// Return `None` when channel is not present in channel_layout
    pub fn index_from_channel(&self, channel: ffi::AVChannel) -> Option<u32> {
        unsafe { ffi::av_channel_layout_index_from_channel(self.as_ptr(), channel) }
            .upgrade()
            .ok()
            .map(|x| x as u32)
    }

    /// Get the index in a channel layout of a channel described by the given string.
    /// In case multiple channels are found, only the first match will be returned.
    pub fn index_from_string(&self, name: &CStr) -> Option<u32> {
        unsafe { ffi::av_channel_layout_index_from_string(self.as_ptr(), name.as_ptr()) }
            .upgrade()
            .ok()
            .map(|x| x as u32)
    }

    /// Get a channel described by the given string.
    pub fn channel_from_string(&self, name: &CStr) -> Option<ffi::AVChannel> {
        let channel =
            unsafe { ffi::av_channel_layout_channel_from_string(self.as_ptr(), name.as_ptr()) };
        (channel != ffi::AV_CHAN_NONE).then_some(channel)
    }

    /// Find out what channels from a given set are present in a channel layout,
    /// without regard for their positions.
    pub fn subset(&self, mask: u64) -> u64 {
        unsafe { ffi::av_channel_layout_subset(self.as_ptr(), mask) }
    }

    /// Check whether a channel layout is valid, i.e. can possibly describe audio data.
    ///
    /// Return `true` if channel_layout is valid, `false` otherwise.
    pub fn check(&self) -> bool {
        let ret = unsafe { ffi::av_channel_layout_check(self.as_ptr()) };
        ret == 1
    }

    /// Check whether two channel layouts are semantically the same, i.e. the same
    /// channels are present on the same positions in both.
    ///
    /// If one of the channel layouts is AV_CHANNEL_ORDER_UNSPEC, while the other is
    /// not, they are considered to be unequal. If both are AV_CHANNEL_ORDER_UNSPEC,
    /// they are considered equal iff the channel counts are the same in both.
    pub fn equal(&self, other: &Self) -> Result<bool> {
        let ret =
            unsafe { ffi::av_channel_layout_compare(self.as_ptr(), other.as_ptr()) }.upgrade()?;
        Ok(ret == 0)
    }
}

/// Iterate over all standard channel layouts.
pub struct AVChannelLayoutIter {
    opaque: *mut c_void,
}

impl Default for AVChannelLayoutIter {
    fn default() -> Self {
        Self {
            opaque: std::ptr::null_mut(),
        }
    }
}

impl Iterator for AVChannelLayoutIter {
    type Item = AVChannelLayoutRef<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe { ffi::av_channel_layout_standard(&mut self.opaque) }
            .upgrade()
            .map(|ptr| unsafe { AVChannelLayoutRef::from_raw(ptr) })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_layout_iterator_test() {
        let mut iter = AVChannelLayoutIter::default();
        let item = iter.next().unwrap();
        assert_eq!(item.describe().unwrap().to_str().unwrap(), "mono");
        let mut item = iter.next().unwrap();
        assert_eq!(item.describe().unwrap().to_str().unwrap(), "stereo");
        for x in iter {
            item = x;
            assert!(!item.describe().unwrap().to_str().unwrap().is_empty())
        }
        assert_eq!(item.describe().unwrap().to_str().unwrap(), "22.2");
    }
}
