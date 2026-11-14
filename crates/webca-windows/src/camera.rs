use windows::{
    Win32::{
        Media::{
            DirectShow::{IAMCameraControl, IAMVideoProcAmp},
            MediaFoundation::{
                IMFActivate, IMFAttributes, IMFMediaSource, MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME,
                MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID,
                MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK, MFCreateAttributes,
                MFEnumDeviceSources,
            },
        },
        System::Com::CoTaskMemFree,
    },
    core::{GUID, Interface, PWSTR},
};

use webca_core::{
    CameraDevice, CameraInfo, CameraKey, ControlCapability, ControlId, ControlSetting, DeviceError,
};

use crate::{
    error::{control_error, platform_error},
    mapping::{self, Location, RawRange},
};

/// A camera found by Media Foundation, ready to be opened.
#[derive(Debug, Clone)]
pub struct CameraEntry {
    pub info: CameraInfo,
    pub device_path: String,
    pub(crate) activate: IMFActivate,
}

/// Lists video capture devices in the order Windows reports them.
pub fn list_cameras() -> Result<Vec<CameraEntry>, DeviceError> {
    let activations = enumerate_activations().map_err(|error| platform_error(&error))?;
    let mut cameras = Vec::with_capacity(activations.len());
    for activate in activations {
        let device_path =
            allocated_string(&activate, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_SYMBOLIC_LINK)
                .map_err(|error| platform_error(&error))?;
        let name = allocated_string(&activate, &MF_DEVSOURCE_ATTRIBUTE_FRIENDLY_NAME)
            .unwrap_or_else(|_| "Camera".to_owned());
        cameras.push(CameraEntry {
            info: CameraInfo { key: CameraKey::new(&device_path), name },
            device_path,
            activate,
        });
    }
    Ok(cameras)
}

fn enumerate_activations() -> windows::core::Result<Vec<IMFActivate>> {
    let mut attributes: Option<IMFAttributes> = None;
    // SAFETY: out-pointer is a valid local; the returned interface is owned by `attributes`.
    unsafe { MFCreateAttributes(&raw mut attributes, 1)? };
    let attributes = attributes.ok_or_else(windows::core::Error::empty)?;
    // SAFETY: both GUIDs are static constants.
    unsafe {
        attributes
            .SetGUID(&MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE, &MF_DEVSOURCE_ATTRIBUTE_SOURCE_TYPE_VIDCAP_GUID)?;
    }
    let mut array: *mut Option<IMFActivate> = std::ptr::null_mut();
    let mut count = 0u32;
    // SAFETY: out-pointers are valid locals; ownership of the array and its elements passes to us.
    unsafe { MFEnumDeviceSources(&attributes, &raw mut array, &raw mut count)? };
    let mut activations = Vec::with_capacity(count as usize);
    for index in 0..count as usize {
        // SAFETY: `array` holds `count` initialized elements; each is read exactly once to take ownership.
        if let Some(activate) = unsafe { array.add(index).read() } {
            activations.push(activate);
        }
    }
    // SAFETY: the array was allocated by Media Foundation with CoTaskMemAlloc; elements were moved out above.
    unsafe { CoTaskMemFree(Some(array.cast_const().cast())) };
    Ok(activations)
}

fn allocated_string(attributes: &IMFActivate, key: &GUID) -> windows::core::Result<String> {
    let mut value = PWSTR::null();
    let mut length = 0u32;
    // SAFETY: out-pointers are valid locals; the string is freed below with CoTaskMemFree.
    unsafe { attributes.GetAllocatedString(key, &raw mut value, &raw mut length)? };
    // SAFETY: `value` is a NUL-terminated wide string returned by the call above.
    let text = unsafe { value.to_string() };
    // SAFETY: `value` was allocated by Media Foundation with CoTaskMemAlloc.
    unsafe { CoTaskMemFree(Some(value.0.cast_const().cast())) };
    text.map_err(|_| windows::core::Error::empty())
}

/// An opened camera whose controls are reached through the DirectShow control interfaces.
#[derive(Debug)]
pub struct WindowsCamera {
    info: CameraInfo,
    source: IMFMediaSource,
    proc_amp: Option<IAMVideoProcAmp>,
    camera_control: Option<IAMCameraControl>,
}

impl WindowsCamera {
    pub fn open(entry: &CameraEntry) -> Result<Self, DeviceError> {
        // SAFETY: `activate` is a live activation object from enumeration.
        let source: IMFMediaSource =
            unsafe { entry.activate.ActivateObject() }.map_err(|error| platform_error(&error))?;
        Ok(Self {
            info: entry.info.clone(),
            proc_amp: source.cast().ok(),
            camera_control: source.cast().ok(),
            source,
        })
    }

    fn range(&self, id: ControlId) -> windows::core::Result<RawRange> {
        let mut raw = RawRange { min: 0, max: 0, step: 0, default: 0, caps: 0 };
        let RawRange { min, max, step, default, caps } = &mut raw;
        // SAFETY: all out-pointers reference fields of the local `raw`.
        unsafe {
            match (mapping::locate(id), &self.proc_amp, &self.camera_control) {
                (Location::ProcAmp(property), Some(amp), _) => {
                    amp.GetRange(property, min, max, step, default, caps)?;
                }
                (Location::CameraControl(property), _, Some(control)) => {
                    control.GetRange(property, min, max, step, default, caps)?;
                }
                _ => return Err(unsupported()),
            }
        }
        Ok(raw)
    }
}

fn unsupported() -> windows::core::Error {
    windows::core::Error::from_hresult(windows::core::HRESULT(mapping::E_PROP_ID_UNSUPPORTED))
}

impl CameraDevice for WindowsCamera {
    fn info(&self) -> &CameraInfo {
        &self.info
    }

    fn capabilities(&self) -> Result<Vec<ControlCapability>, DeviceError> {
        let mut capabilities = Vec::new();
        for id in ControlId::ALL {
            match self.range(id) {
                Ok(raw) => capabilities.extend(mapping::capability(id, raw)),
                Err(error) => match control_error(id, &error) {
                    DeviceError::Unsupported(_) | DeviceError::Platform { .. } => {}
                    fatal => return Err(fatal),
                },
            }
        }
        Ok(capabilities)
    }

    fn read(&self, id: ControlId) -> Result<ControlSetting, DeviceError> {
        let (mut value, mut flags) = (0i32, 0i32);
        // SAFETY: out-pointers reference locals.
        let result = unsafe {
            match (mapping::locate(id), &self.proc_amp, &self.camera_control) {
                (Location::ProcAmp(property), Some(amp), _) => {
                    amp.Get(property, &raw mut value, &raw mut flags)
                }
                (Location::CameraControl(property), _, Some(control)) => {
                    control.Get(property, &raw mut value, &raw mut flags)
                }
                _ => Err(unsupported()),
            }
        };
        result.map_err(|error| control_error(id, &error))?;
        Ok(mapping::setting(value, flags))
    }

    fn write(&self, id: ControlId, setting: ControlSetting) -> Result<(), DeviceError> {
        let flags = mapping::flags(setting);
        // SAFETY: plain value arguments on live interfaces.
        let result = unsafe {
            match (mapping::locate(id), &self.proc_amp, &self.camera_control) {
                (Location::ProcAmp(property), Some(amp), _) => amp.Set(property, setting.value, flags),
                (Location::CameraControl(property), _, Some(control)) => {
                    control.Set(property, setting.value, flags)
                }
                _ => Err(unsupported()),
            }
        };
        result.map_err(|error| control_error(id, &error))
    }
}

impl Drop for WindowsCamera {
    fn drop(&mut self) {
        // SAFETY: shutting down a source we own; errors on an already-removed device are irrelevant.
        let _ = unsafe { self.source.Shutdown() };
    }
}
