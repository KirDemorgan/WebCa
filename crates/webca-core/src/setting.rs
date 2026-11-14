use serde::{Deserialize, Serialize};

use crate::ControlRange;

/// Whether the camera picks the value itself or uses the stored one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlMode {
    #[default]
    Manual,
    Auto,
}

/// A value for one control together with its mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ControlSetting {
    pub value: i32,
    #[serde(default)]
    pub mode: ControlMode,
}

impl ControlSetting {
    #[must_use]
    pub const fn manual(value: i32) -> Self {
        Self {
            value,
            mode: ControlMode::Manual,
        }
    }

    #[must_use]
    pub const fn auto(value: i32) -> Self {
        Self {
            value,
            mode: ControlMode::Auto,
        }
    }

    #[must_use]
    pub const fn is_auto(&self) -> bool {
        matches!(self.mode, ControlMode::Auto)
    }

    /// The same setting with its value made legal for `range`.
    #[must_use]
    pub fn fitted_to(self, range: &ControlRange) -> Self {
        Self {
            value: range.snap(self.value),
            ..self
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_set_mode() {
        assert!(!ControlSetting::manual(3).is_auto());
        assert!(ControlSetting::auto(3).is_auto());
        assert_eq!(ControlMode::default(), ControlMode::Manual);
    }

    #[test]
    fn fitting_snaps_value_and_keeps_mode() {
        let range = ControlRange::new(0, 100, 10, 50).unwrap();
        assert_eq!(
            ControlSetting::auto(87).fitted_to(&range),
            ControlSetting::auto(90)
        );
        assert_eq!(
            ControlSetting::manual(-4).fitted_to(&range),
            ControlSetting::manual(0)
        );
    }

    #[test]
    fn mode_defaults_to_manual_when_missing() {
        let setting: ControlSetting = toml::from_str("value = 12").unwrap();
        assert_eq!(setting, ControlSetting::manual(12));
        let encoded = toml::to_string(&ControlSetting::auto(5)).unwrap();
        assert!(encoded.contains(r#"mode = "auto""#));
    }
}
