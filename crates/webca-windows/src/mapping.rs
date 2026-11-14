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
