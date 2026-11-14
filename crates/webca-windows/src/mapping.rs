use webca_core::{ControlCapability, ControlId, ControlMode, ControlRange, ControlSetting, DeviceError};

const FLAG_AUTO: i32 = 0x1;
const FLAG_MANUAL: i32 = 0x2;

const HRESULT_DEVICE_NOT_CONNECTED: u32 = 0x8007_048F;
const HRESULT_NOT_FOUND: u32 = 0x8007_0490;
const HRESULT_PROP_ID_UNSUPPORTED: u32 = 0x8007_0492;
#[allow(clippy::cast_possible_wrap)]
pub const E_PROP_ID_UNSUPPORTED: i32 = HRESULT_PROP_ID_UNSUPPORTED as i32;
const HRESULT_PROP_SET_UNSUPPORTED: u32 = 0x8007_0491;
const HRESULT_NOT_SUPPORTED: u32 = 0x8007_0032;
const HRESULT_DEVICE_INVALIDATED: u32 = 0xC00D_3EA2;
const HRESULT_DEVICE_PREEMPTED: u32 = 0xC00D_3EA3;
const HRESULT_SHUTDOWN: u32 = 0xC00D_3E85;
const HRESULT_SHARING_VIOLATION: u32 = 0x8007_0020;

/// Which DirectShow control interface serves a control, and its property id there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    ProcAmp(i32),
    CameraControl(i32),
}

#[must_use]
pub const fn locate(id: ControlId) -> Location {
    match id {
        ControlId::Brightness => Location::ProcAmp(0),
        ControlId::Contrast => Location::ProcAmp(1),
        ControlId::Hue => Location::ProcAmp(2),
        ControlId::Saturation => Location::ProcAmp(3),
        ControlId::Sharpness => Location::ProcAmp(4),
        ControlId::Gamma => Location::ProcAmp(5),
        ControlId::WhiteBalance => Location::ProcAmp(7),
        ControlId::BacklightCompensation => Location::ProcAmp(8),
        ControlId::Gain => Location::ProcAmp(9),
        ControlId::PowerLineFrequency => Location::ProcAmp(13),
        ControlId::Pan => Location::CameraControl(0),
        ControlId::Tilt => Location::CameraControl(1),
        ControlId::Zoom => Location::CameraControl(3),
        ControlId::Exposure => Location::CameraControl(4),
        ControlId::Focus => Location::CameraControl(6),
    }
}

#[must_use]
pub fn identify(location: Location) -> Option<ControlId> {
    ControlId::ALL.into_iter().find(|id| locate(*id) == location)
}

/// When Windows writes a stored default: before the stream starts, or once frames flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Timing {
    BeforeStart,
    AfterStart,
}

/// Controls driven by the camera's auto algorithms need frames to converge, so their defaults
/// are written after the stream starts.
#[must_use]
pub const fn timing(id: ControlId) -> Timing {
    match id {
        ControlId::Exposure | ControlId::Focus | ControlId::WhiteBalance => Timing::AfterStart,
        _ => Timing::BeforeStart,
    }
}

/// Registry path, under `HKEY_LOCAL_MACHINE`, of the device parameters that link a camera to its
/// companion app.
#[must_use]
pub fn device_parameters_key(device_path: &str) -> Option<String> {
    let rest = device_path.strip_prefix(r"\\?\")?;
    let (interface, reference) = rest.rsplit_once('\\').unwrap_or((rest, ""));
    let class_start = interface.rfind("#{")? + 1;
    let class = &interface[class_start..];
    if !class.ends_with('}') {
        return None;
    }
    Some(format!(
        r"SYSTEM\CurrentControlSet\Control\DeviceClasses\{class}\##?#{interface}\#{reference}\Device Parameters"
    ))
}

/// Device instance id (as shown in Device Manager) of the camera behind a device interface path.
#[must_use]
pub fn device_instance_id(device_path: &str) -> Option<String> {
    let rest = device_path.strip_prefix(r"\\?\")?;
    let interface = rest.split_once('\\').map_or(rest, |(interface, _)| interface);
    let without_class = &interface[..interface.rfind("#{")?];
    let parts: Vec<&str> = without_class.split('#').collect();
    let [enumerator, hardware, instance] = parts.as_slice() else { return None };
    Some(format!(r"{}\{}\{}", enumerator.to_uppercase(), hardware.to_uppercase(), instance.to_uppercase()))
}

/// Raw output of `GetRange` on a DirectShow control interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RawRange {
    pub min: i32,
    pub max: i32,
    pub step: i32,
    pub default: i32,
    pub caps: i32,
}

/// Turns a driver-reported range into a capability, or `None` when the driver reports nonsense.
///
/// Some UVC drivers report no capability flags at all; such controls are treated as manual.
#[must_use]
pub fn capability(id: ControlId, raw: RawRange) -> Option<ControlCapability> {
    let range = ControlRange::new(raw.min, raw.max, raw.step, raw.default).ok()?;
    let supports_auto = raw.caps & FLAG_AUTO != 0;
    let supports_manual = raw.caps & FLAG_MANUAL != 0 || raw.caps & (FLAG_AUTO | FLAG_MANUAL) == 0;
    Some(ControlCapability { id, range, supports_auto, supports_manual })
}

#[must_use]
pub const fn setting(value: i32, flags: i32) -> ControlSetting {
    if flags & FLAG_AUTO != 0 { ControlSetting::auto(value) } else { ControlSetting::manual(value) }
}

#[must_use]
pub const fn flags(setting: ControlSetting) -> i32 {
    match setting.mode {
        ControlMode::Auto => FLAG_AUTO,
        ControlMode::Manual => FLAG_MANUAL,
    }
}

/// Maps a failed HRESULT from a control call to a user-facing error.
#[must_use]
pub fn device_error(id: ControlId, code: i32, message: &str) -> DeviceError {
    #[allow(clippy::cast_sign_loss)]
    match code as u32 {
        HRESULT_NOT_FOUND
        | HRESULT_PROP_ID_UNSUPPORTED
        | HRESULT_PROP_SET_UNSUPPORTED
        | HRESULT_NOT_SUPPORTED => DeviceError::Unsupported(id),
        HRESULT_DEVICE_NOT_CONNECTED | HRESULT_DEVICE_INVALIDATED | HRESULT_SHUTDOWN => {
            DeviceError::Disconnected
        }
        HRESULT_DEVICE_PREEMPTED | HRESULT_SHARING_VIOLATION => DeviceError::Busy,
        _ => DeviceError::Platform { code, message: message.trim().to_owned() },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::cast_possible_wrap)]
    const fn hr(code: u32) -> i32 {
        code as i32
    }

    fn raw(caps: i32) -> RawRange {
        RawRange { min: 0, max: 255, step: 1, default: 128, caps }
    }

    #[test]
    fn every_control_has_a_distinct_location() {
        let mut seen = Vec::new();
        for id in ControlId::ALL {
            let location = locate(id);
            assert!(!seen.contains(&location), "{id} shares {location:?}");
            seen.push(location);
        }
        assert_eq!(locate(ControlId::Exposure), Location::CameraControl(4));
        assert_eq!(locate(ControlId::PowerLineFrequency), Location::ProcAmp(13));
    }

    #[test]
    fn identify_inverts_locate() {
        for id in ControlId::ALL {
            assert_eq!(identify(locate(id)), Some(id));
        }
        assert_eq!(identify(Location::ProcAmp(6)), None);
        assert_eq!(identify(Location::CameraControl(5)), None);
    }

    #[test]
    fn auto_driven_controls_apply_after_start() {
        let late: Vec<_> =
            ControlId::ALL.into_iter().filter(|id| timing(*id) == Timing::AfterStart).collect();
        assert_eq!(late, [ControlId::WhiteBalance, ControlId::Exposure, ControlId::Focus]);
        assert_eq!(timing(ControlId::Saturation), Timing::BeforeStart);
    }

    #[test]
    fn device_parameters_key_matches_registry_layout() {
        let path = r"\\?\usb#vid_1d6c&pid_0103&mi_00#7&285f974e&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global";
        assert_eq!(
            device_parameters_key(path).unwrap(),
            r"SYSTEM\CurrentControlSet\Control\DeviceClasses\{e5323777-f976-4f5b-9b55-b94699c46e44}\##?#usb#vid_1d6c&pid_0103&mi_00#7&285f974e&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\#global\Device Parameters"
        );
        let without_reference = r"\\?\root#camera#0000#{e5323777-f976-4f5b-9b55-b94699c46e44}";
        assert!(device_parameters_key(without_reference).unwrap().ends_with(r"\#\Device Parameters"));
    }

    #[test]
    fn device_parameters_key_rejects_foreign_paths() {
        assert_eq!(device_parameters_key("usb#vid#{guid}"), None);
        assert_eq!(device_parameters_key(r"\\?\usb#vid_1d6c\global"), None);
        assert_eq!(device_parameters_key(r"\\?\usb#vid#{guid\global"), None);
    }

    #[test]
    fn device_instance_id_matches_device_manager() {
        let path = r"\\?\usb#vid_1d6c&pid_0103&mi_00#7&285f974e&0&0000#{e5323777-f976-4f5b-9b55-b94699c46e44}\global";
        assert_eq!(device_instance_id(path).unwrap(), r"USB\VID_1D6C&PID_0103&MI_00\7&285F974E&0&0000");
        let no_reference = r"\\?\usb#vid_1&pid_2#5&abc#{e5323777-f976-4f5b-9b55-b94699c46e44}";
        assert_eq!(device_instance_id(no_reference).unwrap(), r"USB\VID_1&PID_2\5&ABC");
        assert_eq!(device_instance_id("usb#a#b#{x}"), None);
        assert_eq!(device_instance_id(r"\\?\usb#a#b\global"), None);
        assert_eq!(device_instance_id(r"\\?\usb#a#{x}\global"), None);
    }

    #[test]
    fn capability_reads_mode_flags() {
        let manual = capability(ControlId::Contrast, raw(FLAG_MANUAL)).unwrap();
        assert!(manual.supports_manual && !manual.supports_auto);
        let both = capability(ControlId::Exposure, raw(FLAG_AUTO | FLAG_MANUAL)).unwrap();
        assert!(both.supports_manual && both.supports_auto);
        let auto_only = capability(ControlId::Exposure, raw(FLAG_AUTO)).unwrap();
        assert!(!auto_only.supports_manual && auto_only.supports_auto);
        let unflagged = capability(ControlId::Gamma, raw(0)).unwrap();
        assert!(unflagged.supports_manual && !unflagged.supports_auto);
        assert_eq!(unflagged.range.default(), 128);
    }

    #[test]
    fn capability_rejects_broken_ranges() {
        let broken = RawRange { min: 10, max: 0, step: 1, default: 5, caps: FLAG_MANUAL };
        assert_eq!(capability(ControlId::Hue, broken), None);
        let zero_step = RawRange { step: 0, ..raw(FLAG_MANUAL) };
        assert_eq!(capability(ControlId::Hue, zero_step), None);
    }

    #[test]
    fn flags_round_trip_through_settings() {
        for setting in [ControlSetting::manual(7), ControlSetting::auto(-3)] {
            assert_eq!(super::setting(setting.value, flags(setting)), setting);
        }
        assert_eq!(super::setting(1, FLAG_AUTO | FLAG_MANUAL), ControlSetting::auto(1));
        assert_eq!(super::setting(1, 0), ControlSetting::manual(1));
    }

    #[test]
    fn hresults_map_to_device_errors() {
        let id = ControlId::Focus;
        for code in [0x8007_0490, 0x8007_0492, 0x8007_0491, 0x8007_0032] {
            assert_eq!(device_error(id, hr(code), ""), DeviceError::Unsupported(id));
        }
        for code in [0x8007_048F, 0xC00D_3EA2, 0xC00D_3E85] {
            assert_eq!(device_error(id, hr(code), ""), DeviceError::Disconnected);
        }
        for code in [0xC00D_3EA3, 0x8007_0020] {
            assert_eq!(device_error(id, hr(code), ""), DeviceError::Busy);
        }
        assert_eq!(
            device_error(id, hr(0x8007_0005), " Access is denied.\r\n"),
            DeviceError::Platform { code: hr(0x8007_0005), message: "Access is denied.".into() }
        );
    }
}
