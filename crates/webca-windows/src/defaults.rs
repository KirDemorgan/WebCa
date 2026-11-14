use std::{ffi::c_void, mem::size_of};

use windows::{
    Win32::{
        Foundation::{E_ACCESSDENIED, E_UNEXPECTED, ERROR_FILE_NOT_FOUND, WIN32_ERROR},
        Media::{
            KernelStreaming::{
                KSIDENTIFIER, KSPROPERTY_TYPE_SET, PROPSETID_VIDCAP_CAMERACONTROL,
                PROPSETID_VIDCAP_VIDEOPROCAMP,
            },
            MediaFoundation::{
                CLSID_CameraConfigurationManager, IMFCameraConfigurationManager, IMFCameraControlDefaults,
                IMFCameraControlDefaultsCollection, MF_CAMERA_CONTROL_CONFIGURATION_TYPE,
                MF_CAMERA_CONTROL_CONFIGURATION_TYPE_POSTSTART,
                MF_CAMERA_CONTROL_CONFIGURATION_TYPE_PRESTART,
            },
        },
        System::{
            Com::{CLSCTX_INPROC_SERVER, CoCreateInstance},
            Registry::{HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ, RegGetValueW},
        },
    },
    core::{GUID, HSTRING, PCWSTR},
};

use webca_core::{ControlId, ControlSetting, DeviceError};

use crate::{
    camera::CameraEntry,
    error::platform_error,
    mapping::{self, Location, Timing},
};

const COMPANION_VALUE: &str = "SCSVCamPfn";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DefaultsError {
    #[error("WebCa is not registered as this camera's companion app")]
    NotAuthorized,
    #[error(transparent)]
    Device(#[from] DeviceError),
}

/// Layout shared by `KSPROPERTY_VIDEOPROCAMP_S` and `KSPROPERTY_CAMERACONTROL_S`.
#[repr(C)]
#[derive(Clone, Copy)]
struct ControlPayload {
    property: KSIDENTIFIER,
    value: i32,
    flags: u32,
    capabilities: u32,
}

#[allow(clippy::cast_possible_truncation)]
const PAYLOAD_SIZE: u32 = size_of::<ControlPayload>() as u32;

/// The per-user camera defaults that Windows applies whenever any app starts the camera.
#[derive(Debug)]
pub struct DefaultsStore {
    manager: IMFCameraConfigurationManager,
}

impl DefaultsStore {
    pub fn open() -> Result<Self, DefaultsError> {
        // SAFETY: standard in-process COM activation on an initialized apartment.
        let manager =
            unsafe { CoCreateInstance(&CLSID_CameraConfigurationManager, None, CLSCTX_INPROC_SERVER) }
                .map_err(|error| platform_error(&error))?;
        Ok(Self { manager })
    }

    /// Stored defaults for the controls WebCa manages; other entries are ignored.
    pub fn load(&self, entry: &CameraEntry) -> Result<Vec<(ControlId, ControlSetting)>, DefaultsError> {
        let collection = self.collection(entry)?;
        // SAFETY: `collection` is a live COM object.
        let count = unsafe { collection.GetControlCount() };
        let mut settings = Vec::new();
        for index in 0..count {
            // SAFETY: `index` is below the reported count.
            let control = unsafe { collection.GetControl(index) }.map_err(|error| platform_error(&error))?;
            if let Some(setting) = read_payload(&control)? {
                settings.push(setting);
            }
        }
        Ok(settings)
    }

    /// Replaces every WebCa-managed default of the camera with `settings`.
    pub fn save(
        &self,
        entry: &CameraEntry,
        settings: &[(ControlId, ControlSetting)],
    ) -> Result<(), DefaultsError> {
        let collection = self.collection(entry)?;
        for id in ControlId::ALL {
            let (set, property) = property_of(id);
            // SAFETY: plain arguments; a missing entry is reported as an error that is irrelevant here.
            let _ = unsafe { collection.RemoveControl(&raw const set, property) };
        }
        for (id, setting) in settings {
            write_payload(&collection, *id, *setting)?;
        }
        // SAFETY: `collection` came from this manager.
        unsafe { self.manager.SaveDefaults(&collection) }.map_err(|error| {
            if error.code() == E_ACCESSDENIED {
                DefaultsError::NotAuthorized
            } else {
                platform_error(&error).into()
            }
        })
    }

    pub fn clear(&self, entry: &CameraEntry) -> Result<(), DefaultsError> {
        self.save(entry, &[])
    }

    fn collection(&self, entry: &CameraEntry) -> Result<IMFCameraControlDefaultsCollection, DefaultsError> {
        // SAFETY: the activation object exposes the attributes that identify the camera.
        unsafe { self.manager.LoadDefaults(&entry.activate) }.map_err(|error| platform_error(&error).into())
    }
}

impl Drop for DefaultsStore {
    fn drop(&mut self) {
        // SAFETY: releases the manager's resources; no calls follow.
        unsafe { self.manager.Shutdown() };
    }
}

fn property_of(id: ControlId) -> (GUID, u32) {
    match mapping::locate(id) {
        Location::ProcAmp(property) => (PROPSETID_VIDCAP_VIDEOPROCAMP, property.unsigned_abs()),
        Location::CameraControl(property) => (PROPSETID_VIDCAP_CAMERACONTROL, property.unsigned_abs()),
    }
}

fn configuration_type(id: ControlId) -> MF_CAMERA_CONTROL_CONFIGURATION_TYPE {
    match mapping::timing(id) {
        Timing::BeforeStart => MF_CAMERA_CONTROL_CONFIGURATION_TYPE_PRESTART,
        Timing::AfterStart => MF_CAMERA_CONTROL_CONFIGURATION_TYPE_POSTSTART,
    }
}

fn write_payload(
    collection: &IMFCameraControlDefaultsCollection,
    id: ControlId,
    setting: ControlSetting,
) -> Result<(), DefaultsError> {
    let (set, property) = property_of(id);
    // SAFETY: sizes match `ControlPayload`, the layout Windows expects for these property sets.
    let control = unsafe {
        collection.GetOrAddControl(
            configuration_type(id),
            &raw const set,
            property,
            PAYLOAD_SIZE,
            PAYLOAD_SIZE,
        )
    }
    .map_err(|error| platform_error(&error))?;
    let mut payload = ControlPayload {
        property: KSIDENTIFIER::default(),
        value: setting.value,
        flags: mapping::flags(setting).unsigned_abs(),
        capabilities: 0,
    };
    payload.property.Anonymous.Anonymous.Set = set;
    payload.property.Anonymous.Anonymous.Id = property;
    payload.property.Anonymous.Anonymous.Flags = KSPROPERTY_TYPE_SET;
    let written = with_locked(&control, |header, data| {
        // SAFETY: `with_locked` verified both buffers hold at least one `ControlPayload`.
        unsafe {
            header.cast::<ControlPayload>().write_unaligned(payload);
            data.cast::<ControlPayload>().write_unaligned(payload);
        }
    })?;
    if written {
        Ok(())
    } else {
        Err(DeviceError::Platform {
            code: E_UNEXPECTED.0,
            message: format!("unexpected default layout for {id}"),
        }
        .into())
    }
}

fn read_payload(
    control: &IMFCameraControlDefaults,
) -> Result<Option<(ControlId, ControlSetting)>, DefaultsError> {
    let mut found = None;
    with_locked(control, |header, data| {
        // SAFETY: `with_locked` verified both buffers hold at least one `ControlPayload`.
        let (header, data) = unsafe {
            (header.cast::<ControlPayload>().read_unaligned(), data.cast::<ControlPayload>().read_unaligned())
        };
        // SAFETY: KSIDENTIFIER is a union whose struct view is always initialized for properties.
        let (set, property) =
            unsafe { (header.property.Anonymous.Anonymous.Set, header.property.Anonymous.Anonymous.Id) };
        let location = match set {
            PROPSETID_VIDCAP_VIDEOPROCAMP => Location::ProcAmp(property.cast_signed()),
            PROPSETID_VIDCAP_CAMERACONTROL => Location::CameraControl(property.cast_signed()),
            _ => return,
        };
        found = mapping::identify(location)
            .map(|id| (id, mapping::setting(data.value, data.flags.cast_signed())));
    })?;
    Ok(found)
}

fn with_locked(
    control: &IMFCameraControlDefaults,
    action: impl FnOnce(*mut c_void, *mut c_void),
) -> Result<bool, DefaultsError> {
    let mut header: *mut c_void = std::ptr::null_mut();
    let mut header_size = 0u32;
    let mut data: *mut c_void = std::ptr::null_mut();
    let mut data_size = 0u32;
    // SAFETY: out-pointers are valid locals; the buffers stay valid until `UnlockControlData`.
    unsafe {
        control
            .LockControlData(
                &raw mut header,
                &raw mut header_size,
                Some(&raw mut data),
                Some(&raw mut data_size),
            )
            .map_err(|error| platform_error(&error))?;
    }
    let fits =
        header_size >= PAYLOAD_SIZE && data_size >= PAYLOAD_SIZE && !header.is_null() && !data.is_null();
    if fits {
        action(header, data);
    }
    // SAFETY: balances the successful lock above.
    unsafe { control.UnlockControlData() }.map_err(|error| platform_error(&error))?;
    Ok(fits)
}

/// The package family name a camera names as its companion app, if any.
pub fn companion_app(device_path: &str) -> Result<Option<String>, DeviceError> {
    let Some(key) = mapping::device_parameters_key(device_path) else {
        return Ok(None);
    };
    let key = HSTRING::from(key);
    let value = HSTRING::from(COMPANION_VALUE);
    let mut buffer = [0u16; 256];
    let mut size = u32::try_from(size_of_val(&buffer)).expect("buffer size fits in u32");
    // SAFETY: the buffer and its byte size are valid; strings are NUL-terminated HSTRINGs.
    let status = unsafe {
        RegGetValueW(
            HKEY_LOCAL_MACHINE,
            PCWSTR(key.as_ptr()),
            PCWSTR(value.as_ptr()),
            RRF_RT_REG_SZ,
            None,
            Some(buffer.as_mut_ptr().cast()),
            Some(&raw mut size),
        )
    };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(None);
    }
    if status != WIN32_ERROR(0) {
        return Err(platform_error(&windows::core::Error::from_hresult(status.to_hresult())));
    }
    let length = buffer.iter().position(|&unit| unit == 0).unwrap_or(buffer.len());
    Ok(Some(String::from_utf16_lossy(&buffer[..length])))
}
