//! Win32 FFI. Every `unsafe` block here is a single API call with arguments
//! that are valid for the duration of the call.
#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows_sys::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};

/// The user's accent color from `HKCU\Software\Microsoft\Windows\DWM\AccentColor`
/// (stored as 0xAABBGGRR), as RGB.
pub(crate) fn accent_color() -> Option<[u8; 3]> {
    let subkey = wide(r"Software\Microsoft\Windows\DWM");
    let value = wide("AccentColor");
    let mut data: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    // SAFETY: subkey/value are NUL-terminated UTF-16 buffers alive for the
    // call; `data` is a writable u32 and `size` holds its byte length.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut data).cast(),
            &raw mut size,
        )
    };
    (status == ERROR_SUCCESS).then(|| {
        let [r, g, b, _a] = data.to_le_bytes();
        [r, g, b]
    })
}

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
