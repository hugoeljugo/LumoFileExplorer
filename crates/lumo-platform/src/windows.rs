//! Win32 FFI. Every `unsafe` block here is a single API call with arguments
//! that are valid for the duration of the call.
#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

pub(crate) fn logical_drives() -> Vec<PathBuf> {
    // SAFETY: takes no arguments; returns a bitmask of drive letters (0 on failure).
    let mask = unsafe { GetLogicalDrives() };
    (b'A'..=b'Z')
        .enumerate()
        .filter(|(bit, _)| mask & (1 << bit) != 0)
        .map(|(_, letter)| PathBuf::from(format!("{}:\\", char::from(letter))))
        .collect()
}

pub(crate) fn message_box(title: &str, message: &str) {
    let title = wide(title);
    let message = wide(message);
    // SAFETY: both pointers are NUL-terminated UTF-16 buffers that outlive the
    // call; a null owner window is allowed.
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            title.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}
