use std::ptr::{null_mut, NonNull};
use std::slice;
use std::{ffi::CStr, ops::Deref};

use crate::error::{Result, RsmpegError};
use crate::ffi;
use crate::shared::RetUpgrade;

pub struct AVMmap {
    bufptr: NonNull<u8>,
    size: ffi::size_t,
}

impl Deref for AVMmap {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.bufptr.as_ptr(), self.size as _) }
    }
}

impl AVMmap {
    pub fn new(filename: &CStr) -> Result<Self> {
        let mut bufptr = null_mut();
        let mut size = 0;
        unsafe { ffi::av_file_map(filename.as_ptr(), &mut bufptr, &mut size, 0, null_mut()) }
            .upgrade()
            .map(RsmpegError::AVError)?;
        Ok(Self {
            bufptr: NonNull::new(bufptr).unwrap(),
            size,
        })
    }
}

impl Drop for AVMmap {
    fn drop(&mut self) {
        unsafe {
            ffi::av_file_unmap(self.bufptr.as_ptr(), self.size);
        }
    }
}

#[cfg(test)]
mod test {
    use std::ffi::CString;
    use std::fs::File;
    use std::io::Write;

    use super::*;
    use tempdir::TempDir;
    #[test]
    fn test_file_mapping() {
        let tempdir = TempDir::new("tmp").unwrap();
        let file_path = tempdir.path().join("emm.txt");
        {
            let mut x = File::create(&file_path).unwrap();
            x.write_all(b"hello? you here?").unwrap();
        }
        let file_path = &CString::new(file_path.into_os_string().into_string().unwrap()).unwrap();
        let mmap = AVMmap::new(file_path).unwrap();
        let x: &[u8] = &mmap;
        assert_eq!(x, b"hello? you here?");
    }
}
