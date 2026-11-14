use std::path::Path;

use windows::{
    Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, WIN32_ERROR},
        System::Registry::{
            HKEY_CURRENT_USER, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
        },
    },
    core::HSTRING,
};

use webca_core::DeviceError;

use crate::error::platform_error;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "WebCa";

/// The command Windows runs at sign-in to start WebCa in the tray.
#[must_use]
pub fn autostart_command(executable: &Path) -> String {
    format!("\"{}\" --agent", executable.display())
}

/// Registers WebCa to start in the tray when the user signs in.
pub fn enable_autostart(executable: &Path) -> Result<(), DeviceError> {
    let command: Vec<u16> = autostart_command(executable).encode_utf16().chain([0]).collect();
    let bytes = u32::try_from(command.len() * 2).expect("command length fits in u32");
    // SAFETY: key, value name and data are valid NUL-terminated wide strings of the stated size.
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY),
            &HSTRING::from(VALUE_NAME),
            REG_SZ.0,
            Some(command.as_ptr().cast()),
            bytes,
        )
    };
    check(status)
}

pub fn disable_autostart() -> Result<(), DeviceError> {
    // SAFETY: valid NUL-terminated key and value names.
    let status =
        unsafe { RegDeleteKeyValueW(HKEY_CURRENT_USER, &HSTRING::from(RUN_KEY), &HSTRING::from(VALUE_NAME)) };
    if status == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(status) }
}

/// The registered sign-in command, if WebCa is set to start automatically.
pub fn autostart_entry() -> Result<Option<String>, DeviceError> {
    let mut buffer = [0u16; 1024];
    let mut size = u32::try_from(size_of_val(&buffer)).expect("buffer size fits in u32");
    // SAFETY: the buffer and its byte size are valid.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(RUN_KEY),
            &HSTRING::from(VALUE_NAME),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&raw mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    check(status)?;
    let length = buffer.iter().position(|&unit| unit == 0).unwrap_or(buffer.len());
    Ok(Some(String::from_utf16_lossy(&buffer[..length])))
}

fn check(status: WIN32_ERROR) -> Result<(), DeviceError> {
    if status == WIN32_ERROR(0) {
        Ok(())
    } else {
        Err(platform_error(&windows::core::Error::from_hresult(status.to_hresult())))
    }
}
