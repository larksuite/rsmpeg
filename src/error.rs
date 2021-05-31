//! Errors of the rsmpeg.
use libc::c_int;
use std::cmp::{Eq, PartialEq};
use thiserror::Error;

/// All the error variants of rsmpeg.
#[non_exhaustive]
#[derive(Error, Debug, Eq, PartialEq)]
pub enum RsmpegError {
    #[error("AVERROR({0})")]
    AVError(c_int),
    #[error("CustomError({0})")]
    CustomError(String),

    // --------- Unstablized error type below ------

    // FFmpeg errors
    #[error("Cannot open input file.")]
    OpenInputError,
    #[error("Cannot open output file.")]
    OpenOutputError,
    #[error("Cannot find stream information.")]
    FindStreamInfoError,
    #[error("Cannot write header to output file. ({0})")]
    WriteHeaderError(c_int),
    #[error("Cannot write trailer to output file.")]
    WriteTrailerError,

    #[error("Failed to open codec. ({0})")]
    CodecOpenError(c_int),
    #[error("Failed to copy decoder parameters to input decoder context.")]
    CodecSetParameterError,
    #[error("Failed to copy encoder parameters to output stream.")]
    CodecGetParameterError,
    #[error("Copying parameters failed.")]
    CopyParameterError,
    #[error("Filter not found.")]
    FilterNotFound,
    #[error("Create filter instance in a filter graph failed.")]
    CreateFilterError,
    #[error("Set property to a filter context failed.")]
    SetPropertyError,

    // Decoder errors
    #[error("Failed to decode a packet")]
    DecodePacketError,
    #[error("Send packet to a codec context failed: ({0})")]
    SendPacketError(i32),
    #[error("Decoder isn't accepting input, try to receive several frames and send again.")]
    SendPacketAgainError,
    #[error("Receive frame from a codec context failed: ({0})")]
    ReceiveFrameError(i32),
    #[error("Decoder have no frame currently, Try send new input.")]
    DecoderDrainError,
    #[error("Decoder is already flushed.")]
    DecoderFlushedError,

    // Encoder errors
    #[error("Send frame to a codec context failed: ({0})")]
    SendFrameError(i32),
    #[error("Encoder isn't accepting input, try to receive several packets and send again.")]
    SendFrameAgainError,
    #[error("Receive packet from a codec context failed: ({0})")]
    ReceivePacketError(i32),
    #[error("Encoder have no packet currently, Try send new input.")]
    EncoderDrainError,
    #[error("Encoder is already flushed.")]
    EncoderFlushedError,

    // Bitstream errors
    #[error("Bitstream filter isn't accepting input, try to receive several frames and send again.")]
    BitstreamSendPacketAgainError,
    #[error("Bitstream filter is already flushed")]
    BitstreamFlushedError,
    #[error("Send packet to a bitstream filter context failed: ({0})")]
    BitstreamSendPacketError(i32),

    #[error("Read frame to an input format context failed: ({0})")]
    ReadFrameError(i32),
    #[error("Write frame to an output format context failed.")]
    WriteFrameError,
    #[error("Interleaved write frame to an output format context failed.")]
    InterleavedWriteFrameError(i32),

    #[error("Flush an encoder failed.")]
    FlushEncoderError,

    #[error("Error while feeding the filtergraph failed.")]
    BufferSrcAddFrameError,
    #[error("Pulling filtered frame from filters failed")]
    BufferSinkGetFrameError,
    #[error("No frames are available at this point")]
    BufferSinkDrainError,
    #[error("There will be no more output frames on this sink")]
    BufferSinkEofError,

    #[error("AVDictionary doesn't understand provided string")]
    DictionaryParseError,
    #[error("AVDictionary failed to get string.")]
    DictionaryGetStringError,

    #[error("AVIO Open failure.")]
    AVIOOpenError,

    #[error("SwrContext init failed.")]
    SwrContextInitError,
    #[error("SwrContext converting data failed.")]
    SwrConvertError,

    #[error("SwsContext scale failed.")]
    SwsScaleError,

    #[error("AudioFifo write failed.")]
    AudioFifoWriteError,
    #[error("AudioFifo read failed.")]
    AudioFifoReadError,

    #[error("AVFrame buffer double allocating.")]
    AVFrameDoubleAllocatingError,
    #[error("AVFrame buffer allocating with incorrect parameters.")]
    AVFrameInvalidAllocatingError,

    #[error("Get picture size error.")]
    AVPictureGetSizeError,
    #[error("Cannot copy a AVPicture.")]
    AVPictureCopyError,
    #[error("Failed to copy a AVPicture's data to buffer.")]
    AVPictureCopyToBufferError,

    #[error("Failed to fill data to image buffer.")]
    AVImageFillArrayError,

    // Non exhaustive
    #[error("Unknown error, contact ldm0 when you see this.")]
    Unknown,
}

/// Overall result of Rsmpeg functions
pub type Result<T> = std::result::Result<T, RsmpegError>;

/// A wrapper around c_int(return type of many ffmpeg inner libraries functions)
pub type Ret = std::result::Result<c_int, c_int>;

impl From<c_int> for RsmpegError {
    fn from(err: c_int) -> Self {
        RsmpegError::AVError(err)
    }
}
