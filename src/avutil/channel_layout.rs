use crate::ffi;

pub fn av_get_channel_layout_nb_channels(channel_layout: u64) -> i32 {
    unsafe { ffi::av_get_channel_layout_nb_channels(channel_layout) }
}

pub fn av_get_default_channel_layout(nb_channels: i32) -> u64 {
    // From i64 to u64, safe.
    unsafe { ffi::av_get_default_channel_layout(nb_channels) as u64 }
}
