//! Errors of the rsmpeg.
use libc::c_int;
use std::{
    cmp::{Eq, PartialEq},
    num::TryFromIntError,
};
use thiserror::Error;

use crate::{avutil::err2str, ffi, shared::AVERROR_EAGAIN};

/// All the error variants of rsmpeg.
#[non_exhaustive]
#[derive(Error, Debug, Eq, PartialEq)]
pub enum RsmpegError {
    #[error("AVERROR({0}): `{}`", err2str(*.0).unwrap_or_else(|| "Unknown error code.".to_string()))]
    AVError(c_int),
    #[error("{0}")]
    CustomError(String),

    // --------- Unstablized error type below ------

    // FFmpeg errors
    #[error("Cannot open input file. ({0})")]
    OpenInputError(c_int),
    #[error("Cannot open output file. ({0})")]
    OpenOutputError(c_int),
    #[error("Cannot find stream information. ({0})")]
    FindStreamInfoError(c_int),
    #[error("Cannot write header to output file. ({0})")]
    WriteHeaderError(c_int),
    #[error("Cannot write trailer to output file. ({0})")]
    WriteTrailerError(c_int),

    #[error("Failed to open codec. ({0})")]
    CodecOpenError(c_int),
    #[error("Failed to copy decoder parameters to input decoder context. ({0})")]
    CodecSetParameterError(c_int),
    #[error("Filter not found.")]
    FilterNotFound,
    #[error("Create filter instance in a filter graph failed. ({0})")]
    CreateFilterError(c_int),
    #[error("Set property to a filter context failed. ({0})")]
    SetPropertyError(c_int),

    // Decoder errors
    #[error("Send packet to a codec context failed. ({0})")]
    SendPacketError(c_int),
    #[error("Decoder isn't accepting input, try to receive several frames and send again.")]
    DecoderFullError,
    #[error("Receive frame from a codec context failed. ({0})")]
    ReceiveFrameError(c_int),
    #[error("Decoder have no frame currently, Try send new input.")]
    DecoderDrainError,
    #[error("Decoder is already flushed.")]
    DecoderFlushedError,

    // Encoder errors
    #[error("Send frame to a codec context failed. ({0})")]
    SendFrameError(c_int),
    #[error("Encoder isn't accepting input, try to receive several packets and send again.")]
    SendFrameAgainError,
    #[error("Receive packet from a codec context failed. ({0})")]
    ReceivePacketError(c_int),
    #[error("Encoder have no packet currently, Try send new input.")]
    EncoderDrainError,
    #[error("Encoder is already flushed.")]
    EncoderFlushedError,

    // Bitstream errors
    #[error("Bitstream filter isn't accepting input, receive packets and send again.")]
    BitstreamFullError,
    #[error("More packets need to be sent to the bitstream filter.")]
    BitstreamDrainError,
    #[error("Bitstream filter is already flushed")]
    BitstreamFlushedError,
    #[error("Send packet to a bitstream filter context failed. ({0})")]
    BitstreamSendPacketError(c_int),
    #[error("Receive packet from a bitstream filter context failed. ({0})")]
    BitstreamReceivePacketError(c_int),
    #[error("Failed to initialize bitstream filter context. ({0})")]
    BitstreamInitializationError(c_int),

    #[error("Interleaved write frame to an output format context failed. ({0})")]
    InterleavedWriteFrameError(c_int),

    #[error("Error while feeding the filtergraph failed. ({0})")]
    BufferSrcAddFrameError(c_int),
    #[error("Pulling filtered frame from filters failed ({0})")]
    BufferSinkGetFrameError(c_int),
    #[error("No frames are available at this point")]
    BufferSinkDrainError,
    #[error("There will be no more output frames on this sink")]
    BufferSinkEofError,

    #[error("AVDictionary doesn't understand provided string. ({0})")]
    DictionaryParseError(c_int),
    #[error("AVDictionary failed to get string. ({0})")]
    DictionaryGetStringError(c_int),

    #[error("AVIO Open failure. ({0})")]
    AVIOOpenError(c_int),

    #[error("SwsContext scale failed. ({0})")]
    SwsScaleError(c_int),

    #[error("AVFrame buffer double allocating.")]
    AVFrameDoubleAllocatingError,
    #[error("AVFrame buffer allocating with incorrect parameters. ({0})")]
    AVFrameInvalidAllocatingError(c_int),

    #[error("Failed to fill data to image buffer. ({0})")]
    AVImageFillArrayError(c_int),

    #[error("{0}")]
    TryFromIntError(TryFromIntError),

    // Non exhaustive
    #[error("Unknown error, contact ldm0 when you see this.")]
    Unknown,
}

impl RsmpegError {
    #[must_use]
    pub fn raw_error(&self) -> Option<c_int> {
        match self {
            Self::AVError(err)
            | Self::OpenInputError(err)
            | Self::OpenOutputError(err)
            | Self::FindStreamInfoError(err)
            | Self::WriteHeaderError(err)
            | Self::WriteTrailerError(err)
            | Self::CodecOpenError(err)
            | Self::CodecSetParameterError(err)
            | Self::CreateFilterError(err)
            | Self::SetPropertyError(err)
            | Self::SendPacketError(err)
            | Self::ReceiveFrameError(err)
            | Self::SendFrameError(err)
            | Self::ReceivePacketError(err)
            | Self::BitstreamSendPacketError(err)
            | Self::BitstreamReceivePacketError(err)
            | Self::BitstreamInitializationError(err)
            | Self::InterleavedWriteFrameError(err)
            | Self::BufferSrcAddFrameError(err)
            | Self::BufferSinkGetFrameError(err)
            | Self::DictionaryParseError(err)
            | Self::DictionaryGetStringError(err)
            | Self::AVIOOpenError(err)
            | Self::SwsScaleError(err)
            | Self::AVFrameInvalidAllocatingError(err)
            | Self::AVImageFillArrayError(err) => Some(*err),

            Self::DecoderFullError
            | Self::BufferSinkDrainError
            | Self::DecoderDrainError
            | Self::SendFrameAgainError
            | Self::BitstreamFullError
            | Self::BitstreamDrainError
            | Self::EncoderDrainError => Some(AVERROR_EAGAIN),

            Self::BufferSinkEofError
            | Self::DecoderFlushedError
            | Self::EncoderFlushedError
            | Self::BitstreamFlushedError => Some(ffi::AVERROR_EOF),

            Self::AVFrameDoubleAllocatingError
            | Self::FilterNotFound
            | Self::CustomError(_)
            | Self::TryFromIntError(_)
            | Self::Unknown => None,
        }
    }
}

/// Overall result of Rsmpeg functions
pub type Result<T, E = RsmpegError> = std::result::Result<T, E>;

/// A wrapper around c_int(return type of many ffmpeg inner libraries functions)
pub type Ret = std::result::Result<c_int, c_int>;

impl From<c_int> for RsmpegError {
    fn from(err: c_int) -> Self {
        Self::AVError(err)
    }
}

impl From<TryFromIntError> for RsmpegError {
    fn from(err: TryFromIntError) -> Self {
        Self::TryFromIntError(err)
    }
}
