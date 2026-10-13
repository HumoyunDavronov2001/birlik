//! Files with holes. A window of a model lies at its place in a file of the model's full length, and the bytes
//! outside it should take no disk space: Linux and macOS leave the ranges a file never writes as holes, NTFS
//! allocates them unless the file is marked sparse first.

#![allow(unsafe_code)]

use std::fs::File;
use std::io;

/// Marks `file` sparse, so the ranges it never writes take no disk space; nothing to do outside Windows.
///
/// # Errors
/// The file system has no sparse files (FAT32, exFAT): the holes are then written out, which only costs space.
pub fn make_sparse(file: &File) -> io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;

        use windows_sys::Win32::System::IO::DeviceIoControl;
        use windows_sys::Win32::System::Ioctl::FSCTL_SET_SPARSE;

        let mut returned = 0u32;
        // SAFETY: the handle is the open file's own and outlives the call. FSCTL_SET_SPARSE without an input buffer
        // sets the attribute and writes no output; the call is synchronous, so no OVERLAPPED is passed.
        let ok = unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                FSCTL_SET_SPARSE,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
                &raw mut returned,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
    }
    #[cfg(not(windows))]
    let _ = file;
    Ok(())
}

/// Whether `file` takes less disk space than its length: always true where holes need no marking.
///
/// # Errors
/// The file's metadata cannot be read.
pub fn is_sparse(file: &File) -> io::Result<bool> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_SPARSE_FILE: u32 = 0x200;
        Ok(file.metadata()?.file_attributes() & FILE_ATTRIBUTE_SPARSE_FILE != 0)
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Ok(true)
    }
}
