use rusty_ffmpeg::ffi;

/// non exhaustive wrapper of AVMediaType
pub struct AVMediaType(pub ffi::AVMediaType);

impl AVMediaType {
    /// A video codec
    pub fn is_video(&self) -> bool {
        self.0 == ffi::AVMEDIA_TYPE_VIDEO
    }

    /// An audio codec
    pub fn is_audio(&self) -> bool {
        self.0 == ffi::AVMEDIA_TYPE_AUDIO
    }

    /// A data codec(Opaque data information usually continuous)
    pub fn is_data(&self) -> bool {
        self.0 == ffi::AVMEDIA_TYPE_DATA
    }

    /// A subtitle codec
    pub fn is_subtitle(&self) -> bool {
        self.0 == ffi::AVMEDIA_TYPE_SUBTITLE
    }

    /// An attachment codec(Opaque data information usually sparse)
    pub fn is_attachment(&self) -> bool {
        self.0 == ffi::AVMEDIA_TYPE_ATTACHMENT
    }
}
