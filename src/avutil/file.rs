use std::{
    ffi::CStr,
    ops::Deref,
    ptr::{null_mut, NonNull},
    slice,
};

use crate::{error::Result, ffi, shared::RetUpgrade};

/// A read-only file buffer, the file is mmaped when available.
pub struct AVMmap {
    bufptr: NonNull<u8>,
    size: usize,
}

unsafe impl Send for AVMmap {}

impl Deref for AVMmap {
    type Target = [u8];
    fn deref(&self) -> &Self::Target {
        unsafe { slice::from_raw_parts(self.bufptr.as_ptr(), self.size) }
    }
}

impl AVMmap {
    /// Read the file with name filename, and put its content in a newly
    /// allocated read-only buffer (will map it with mmap() when available).
    pub fn new(filename: &CStr) -> Result<Self> {
        let mut bufptr = null_mut();
        let mut size = 0;
        unsafe { ffi::av_file_map(filename.as_ptr(), &mut bufptr, &mut size, 0, null_mut()) }
            .upgrade()?;
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
    use std::{ffi::CString, fs::File, io::Write};

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
