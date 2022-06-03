use std::ops::Drop;

use crate::{
    avcodec::{AVCodecContext, AVCodecID, AVPacket},
    error::*,
    ffi,
    shared::*,
};

wrap!(AVCodecParserContext: ffi::AVCodecParserContext);

impl AVCodecParserContext {
    /// Allocate a [`AVCodecParserContext`] with given [`AVCodecID`].
    pub fn find(codec_id: AVCodecID) -> Option<Self> {
        unsafe { ffi::av_parser_init(codec_id as i32) }
            .upgrade()
            .map(|x| unsafe { Self::from_raw(x) })
    }

    /// Parse a packet.
    ///
    /// Return `Err(_)` On failure, `bool` field of returned tuple means if
    /// packet is ready, `usize` field of returned tuple means the offset of the
    /// data being parsed.
    pub fn parse_packet(
        &mut self,
        codec_context: &mut AVCodecContext,
        packet: &mut AVPacket,
        data: &[u8],
    ) -> Result<(bool, usize)> {
        let mut packet_data = packet.data;
        let mut packet_size = packet.size;
        let offset = unsafe {
            ffi::av_parser_parse2(
                self.as_mut_ptr(),
                codec_context.as_mut_ptr(),
                &mut packet_data,
                &mut packet_size,
                data.as_ptr(),
                data.len() as i32,
                packet.pts,
                packet.dts,
                packet.pos,
            )
        }
        .upgrade()?;
        unsafe {
            packet.deref_mut().data = packet_data;
            packet.deref_mut().size = packet_size;
        }
        Ok((packet.size != 0, offset as usize))
    }
}

impl Drop for AVCodecParserContext {
    fn drop(&mut self) {
        unsafe { ffi::av_parser_close(self.as_mut_ptr()) }
    }
}
