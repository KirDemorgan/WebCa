use std::{path::Path, process::Command};

use windows::{
    Win32::{
        Foundation::{
            APPMODEL_ERROR_NO_PACKAGE, CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER,
            WIN32_ERROR,
        },
        Storage::Packaging::Appx::GetCurrentPackageFamilyName,
        System::{
            Registry::{
                HKEY_LOCAL_MACHINE, REG_SZ, RRF_RT_REG_SZ, RegDeleteKeyValueW, RegGetValueW, RegSetKeyValueW,
            },
            Threading::{GetExitCodeProcess, INFINITE, WaitForSingleObject},
        },
        UI::{
            Shell::{SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW, ShellExecuteExW},
            WindowsAndMessaging::SW_HIDE,
        },
    },
    core::{HSTRING, PCWSTR, PWSTR, w},
};

use webca_core::DeviceError;

use crate::{
    error::platform_error,
    mapping::{device_instance_id, device_parameters_key},
};

const COMPANION_VALUE: &str = "SCSVCamPfn";
const PREVIOUS_VALUE: &str = "WebCaPreviousPfn";

/// Package family name of the running process, or `None` for the portable build.
pub fn current_package_family() -> Option<String> {
    let mut length = 0u32;
    // SAFETY: querying the required length with a null buffer.
    let status = unsafe { GetCurrentPackageFamilyName(&raw mut length, None) };
    if status == APPMODEL_ERROR_NO_PACKAGE || status != ERROR_INSUFFICIENT_BUFFER {
        return None;
    }
    let mut buffer = vec![0u16; length as usize];
    // SAFETY: the buffer holds `length` UTF-16 units as requested above.
    let status = unsafe { GetCurrentPackageFamilyName(&raw mut length, Some(PWSTR(buffer.as_mut_ptr()))) };
    (status == WIN32_ERROR(0)).then(|| String::from_utf16_lossy(&buffer[..length.saturating_sub(1) as usize]))
}

/// Names `package_family` as the camera's companion app, keeping any previous companion for restoring.
/// Requires administrator rights.
pub fn associate(device_path: &str, package_family: &str) -> Result<(), DeviceError> {
    let key = parameters_key(device_path)?;
    if let Some(previous) = read_value(&key, COMPANION_VALUE)?
        && previous != package_family
        && read_value(&key, PREVIOUS_VALUE)?.is_none()
    {
        write_value(&key, PREVIOUS_VALUE, &previous)?;
    }
    write_value(&key, COMPANION_VALUE, package_family)?;
    restart_device(device_path)
}

/// Removes WebCa as the companion app and restores the previous one, if any. Requires administrator rights.
pub fn dissociate(device_path: &str) -> Result<(), DeviceError> {
    let key = parameters_key(device_path)?;
    match read_value(&key, PREVIOUS_VALUE)? {
        Some(previous) => {
            write_value(&key, COMPANION_VALUE, &previous)?;
            delete_value(&key, PREVIOUS_VALUE)?;
        }
        None => delete_value(&key, COMPANION_VALUE)?,
    }
    restart_device(device_path)
}

/// Runs `executable` with `arguments` as administrator after the UAC prompt and returns its exit code.
pub fn run_elevated(executable: &Path, arguments: &str) -> Result<u32, DeviceError> {
    let file = HSTRING::from(executable.as_os_str());
    let parameters = HSTRING::from(arguments);
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(size_of::<SHELLEXECUTEINFOW>()).expect("struct size fits in u32"),
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: w!("runas"),
        lpFile: PCWSTR(file.as_ptr()),
        lpParameters: PCWSTR(parameters.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: `info` is fully initialized and its strings outlive the call.
    unsafe { ShellExecuteExW(&raw mut info) }.map_err(|error| platform_error(&error))?;
    let mut code = 0u32;
    // SAFETY: SEE_MASK_NOCLOSEPROCESS gives us a process handle that we wait on and close.
    unsafe {
        WaitForSingleObject(info.hProcess, INFINITE);
        let result = GetExitCodeProcess(info.hProcess, &raw mut code);
        let _ = CloseHandle(info.hProcess);
        result.map_err(|error| platform_error(&error))?;
    }
    Ok(code)
}

fn parameters_key(device_path: &str) -> Result<HSTRING, DeviceError> {
    device_parameters_key(device_path).map(HSTRING::from).ok_or_else(|| DeviceError::Platform {
        code: 0,
        message: format!("unrecognized camera path {device_path}"),
    })
}

fn restart_device(device_path: &str) -> Result<(), DeviceError> {
    let Some(instance) = device_instance_id(device_path) else { return Ok(()) };
    let status = Command::new("pnputil")
        .args(["/restart-device", &instance])
        .status()
        .map_err(|error| DeviceError::Platform { code: 0, message: error.to_string() })?;
    if status.success() {
        Ok(())
    } else {
        Err(DeviceError::Platform {
            code: status.code().unwrap_or(-1),
            message: "the camera could not be restarted".into(),
        })
    }
}

fn read_value(key: &HSTRING, name: &str) -> Result<Option<String>, DeviceError> {
    let mut buffer = [0u16; 256];
    let mut size = u32::try_from(size_of_val(&buffer)).expect("buffer size fits in u32");
    // SAFETY: the buffer and its byte size are valid.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            key,
            &HSTRING::from(name),
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

fn write_value(key: &HSTRING, name: &str, value: &str) -> Result<(), DeviceError> {
    let data: Vec<u16> = value.encode_utf16().chain([0]).collect();
    let bytes = u32::try_from(data.len() * 2).expect("value length fits in u32");
    // SAFETY: `data` is a NUL-terminated wide string of `bytes` bytes.
    let status = unsafe {
        RegSetKeyValueW(
            HKEY_LOCAL_MACHINE,
            key,
            &HSTRING::from(name),
            REG_SZ.0,
            Some(data.as_ptr().cast()),
            bytes,
        )
    };
    check(status)
}

fn delete_value(key: &HSTRING, name: &str) -> Result<(), DeviceError> {
    // SAFETY: valid NUL-terminated key and value names.
    let status = unsafe { RegDeleteKeyValueW(HKEY_LOCAL_MACHINE, key, &HSTRING::from(name)) };
    if status == ERROR_FILE_NOT_FOUND { Ok(()) } else { check(status) }
}

fn check(status: WIN32_ERROR) -> Result<(), DeviceError> {
    if status == WIN32_ERROR(0) {
        Ok(())
    } else {
        Err(platform_error(&windows::core::Error::from_hresult(status.to_hresult())))
    }
}
