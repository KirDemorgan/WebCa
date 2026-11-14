use crate::{CameraKey, ControlId, ControlRange, ControlSetting};

/// What a camera reports about one of its controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlCapability {
    pub id: ControlId,
    pub range: ControlRange,
    pub supports_auto: bool,
    pub supports_manual: bool,
}

impl ControlCapability {
    #[must_use]
    pub const fn manual(id: ControlId, range: ControlRange) -> Self {
        Self { id, range, supports_auto: false, supports_manual: true }
    }

    #[must_use]
    pub const fn with_auto(self) -> Self {
        Self { supports_auto: true, ..self }
    }

    /// Whether the camera accepts `setting` as is, ignoring the value grid.
    #[must_use]
    pub const fn accepts(&self, setting: ControlSetting) -> bool {
        if setting.is_auto() { self.supports_auto } else { self.supports_manual }
    }
}

/// A camera as listed by the platform.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CameraInfo {
    pub key: CameraKey,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeviceError {
    #[error("the camera does not support {0}")]
    Unsupported(ControlId),
    #[error("the camera was disconnected")]
    Disconnected,
    #[error("the camera is busy in another application")]
    Busy,
    #[error("camera driver error {code:#010x}: {message}")]
    Platform { code: i32, message: String },
}

/// Access to the controls of one opened camera.
pub trait CameraDevice {
    fn info(&self) -> &CameraInfo;

    fn capabilities(&self) -> Result<Vec<ControlCapability>, DeviceError>;

    fn read(&self, id: ControlId) -> Result<ControlSetting, DeviceError>;

    fn write(&self, id: ControlId, setting: ControlSetting) -> Result<(), DeviceError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn range() -> ControlRange {
        ControlRange::new(0, 10, 1, 5).unwrap()
    }

    #[test]
    fn capability_acceptance_follows_supported_modes() {
        let manual_only = ControlCapability::manual(ControlId::Hue, range());
        assert!(manual_only.accepts(ControlSetting::manual(3)));
        assert!(!manual_only.accepts(ControlSetting::auto(3)));

        let both = manual_only.with_auto();
        assert!(both.accepts(ControlSetting::auto(3)));

        let auto_only = ControlCapability { supports_manual: false, ..both };
        assert!(!auto_only.accepts(ControlSetting::manual(3)));
    }

    #[test]
    fn errors_describe_the_problem() {
        assert_eq!(
            DeviceError::Unsupported(ControlId::Focus).to_string(),
            "the camera does not support focus"
        );
        assert_eq!(DeviceError::Disconnected.to_string(), "the camera was disconnected");
        assert_eq!(DeviceError::Busy.to_string(), "the camera is busy in another application");
        let platform = DeviceError::Platform { code: -2_147_024_891, message: "Access is denied.".into() };
        assert_eq!(platform.to_string(), "camera driver error 0x80070005: Access is denied.");
    }
}
