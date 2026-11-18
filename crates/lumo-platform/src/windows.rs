//! Win32 FFI. Every `unsafe` block here is a single API call with arguments
//! that are valid for the duration of the call.
#![allow(unsafe_code)]

use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows_sys::Win32::Foundation::ERROR_SUCCESS;
use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, RRF_RT_REG_DWORD, RRF_RT_REG_SZ, RegGetValueW,
};
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

/// Explorer's clipboard format telling paste whether to move or copy.
const PREFERRED_DROP_EFFECT: &str = "Preferred DropEffect";
const DROPEFFECT_COPY_LINK: u32 = 5;
const DROPEFFECT_MOVE: u32 = 2;

fn open_clipboard() -> std::io::Result<clipboard_win::Clipboard> {
    // Another app may hold the clipboard briefly; retry a few times.
    clipboard_win::Clipboard::new_attempts(10).map_err(|e| std::io::Error::other(e.to_string()))
}

pub(crate) fn set_file_clipboard(paths: &[PathBuf], cut: bool) -> std::io::Result<()> {
    use clipboard_win::raw;
    let to_io = |e: clipboard_win::ErrorCode| std::io::Error::other(e.to_string());
    let _clipboard = open_clipboard()?;
    raw::empty().map_err(to_io)?;
    let names: Vec<String> = paths
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    raw::set_file_list(&names).map_err(to_io)?;
    if let Some(format) = raw::register_format(PREFERRED_DROP_EFFECT) {
        let effect = if cut {
            DROPEFFECT_MOVE
        } else {
            DROPEFFECT_COPY_LINK
        };
        raw::set_without_clear(format.get(), &effect.to_le_bytes()).map_err(to_io)?;
    }
    Ok(())
}

pub(crate) fn file_clipboard() -> Option<crate::FileClipboard> {
    use clipboard_win::raw;
    let _clipboard = open_clipboard().ok()?;
    let mut paths = Vec::new();
    raw::get_file_list_path(&mut paths).ok()?;
    if paths.is_empty() {
        return None;
    }
    let cut = raw::register_format(PREFERRED_DROP_EFFECT)
        .filter(|format| raw::is_format_avail(format.get()))
        .and_then(|format| {
            let mut effect = [0u8; 4];
            raw::get(format.get(), &mut effect).ok()?;
            Some(u32::from_le_bytes(effect) & DROPEFFECT_MOVE != 0)
        })
        .unwrap_or(false);
    Some(crate::FileClipboard { paths, cut })
}

pub(crate) fn clear_clipboard() {
    if let Ok(_clipboard) = open_clipboard() {
        let _ = clipboard_win::raw::empty();
    }
}

fn registry_string(root: HKEY, subkey: &str, value: &str) -> Option<String> {
    let subkey = wide(subkey);
    let value = wide(value);
    let mut buffer = [0u16; 256];
    let mut size = std::mem::size_of_val(&buffer) as u32;
    // SAFETY: subkey/value are NUL-terminated UTF-16 buffers alive for the
    // call; `buffer` is writable and `size` holds its length in bytes.
    let status = unsafe {
        RegGetValueW(
            root,
            subkey.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_SZ,
            std::ptr::null_mut(),
            buffer.as_mut_ptr().cast(),
            &raw mut size,
        )
    };
    if status != ERROR_SUCCESS {
        return None;
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    Some(String::from_utf16_lossy(&buffer[..len]))
}

/// e.g. "Windows 11 24H2 (build 26200)". The registry's product name says
/// "Windows 10" on Windows 11 too, so the build number decides.
pub(crate) fn os_description() -> String {
    const KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
    let build = registry_string(HKEY_LOCAL_MACHINE, KEY, "CurrentBuildNumber");
    let release = registry_string(HKEY_LOCAL_MACHINE, KEY, "DisplayVersion");
    let name = match build.as_deref().and_then(|b| b.parse::<u32>().ok()) {
        Some(b) if b >= 22000 => "Windows 11",
        Some(_) => "Windows 10",
        None => "Windows",
    };
    match (release, build) {
        (Some(release), Some(build)) => format!("{name} {release} (build {build})"),
        (None, Some(build)) => format!("{name} (build {build})"),
        _ => name.to_owned(),
    }
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
