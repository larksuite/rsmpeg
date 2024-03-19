use std::{fmt, ptr::NonNull};

use crate::{avutil::AVRational, ffi, shared::*};

wrap!(AVPacket: ffi::AVPacket);
settable!(AVPacket {
    pts: i64,
    dts: i64,
    stream_index: i32,
    flags: i32,
    duration: i64,
    pos: i64,
});

impl AVPacket {
    /// Create an [`AVPacket`] and set its fields to default values.
    pub fn new() -> Self {
        let packet = unsafe { ffi::av_packet_alloc() };
        unsafe { Self::from_raw(NonNull::new(packet).unwrap()) }
    }

    /// Convert valid timing fields (timestamps / durations) in a packet from
    /// one timebase to another. Timestamps with unknown values
    /// (`AV_NOPTS_VALUE`) will be ignored.
    pub fn rescale_ts(&mut self, from: AVRational, to: AVRational) {
        unsafe {
            ffi::av_packet_rescale_ts(self.as_mut_ptr(), from, to);
        }
    }
}

impl fmt::Debug for AVPacket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AVPacket")
            .field("pts", &self.pts)
            .field("dts", &self.dts)
            .field("size", &self.size)
            .field("stream_index", &self.stream_index)
            .field("flags", &self.flags)
            .field("duration", &self.duration)
            .field("pos", &self.pos)
            .finish()
    }
}

impl Default for AVPacket {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for AVPacket {
    /// Free the packet, if the packet is reference counted, it will be
    /// unreferenced first.
    fn drop(&mut self) {
        let mut packet = self.as_mut_ptr();
        unsafe {
            ffi::av_packet_free(&mut packet);
        }
    }
}
