use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// A camera control that WebCa knows how to present and persist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ControlId {
    Brightness,
    Contrast,
    Hue,
    Saturation,
    Sharpness,
    Gamma,
    WhiteBalance,
    BacklightCompensation,
    Gain,
    PowerLineFrequency,
    Exposure,
    Focus,
    Zoom,
    Pan,
    Tilt,
}

/// Visual grouping of controls in the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ControlGroup {
    Image,
    Light,
    Optics,
}

impl ControlId {
    pub const ALL: [Self; 15] = [
        Self::Brightness,
        Self::Contrast,
        Self::Hue,
        Self::Saturation,
        Self::Sharpness,
        Self::Gamma,
        Self::WhiteBalance,
        Self::BacklightCompensation,
        Self::Gain,
        Self::PowerLineFrequency,
        Self::Exposure,
        Self::Focus,
        Self::Zoom,
        Self::Pan,
        Self::Tilt,
    ];

    /// Stable identifier used in profiles, the CLI and the MCP server.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Brightness => "brightness",
            Self::Contrast => "contrast",
            Self::Hue => "hue",
            Self::Saturation => "saturation",
            Self::Sharpness => "sharpness",
            Self::Gamma => "gamma",
            Self::WhiteBalance => "white_balance",
            Self::BacklightCompensation => "backlight_compensation",
            Self::Gain => "gain",
            Self::PowerLineFrequency => "power_line_frequency",
            Self::Exposure => "exposure",
            Self::Focus => "focus",
            Self::Zoom => "zoom",
            Self::Pan => "pan",
            Self::Tilt => "tilt",
        }
    }

    #[must_use]
    pub const fn group(self) -> ControlGroup {
        match self {
            Self::Brightness
            | Self::Contrast
            | Self::Hue
            | Self::Saturation
            | Self::Sharpness
            | Self::Gamma => ControlGroup::Image,
            Self::WhiteBalance
            | Self::BacklightCompensation
            | Self::Gain
            | Self::PowerLineFrequency
            | Self::Exposure => ControlGroup::Light,
            Self::Focus | Self::Zoom | Self::Pan | Self::Tilt => ControlGroup::Optics,
        }
    }

    /// Controls whose values are discrete choices rather than a continuous scale.
    #[must_use]
    pub const fn is_choice(self) -> bool {
        matches!(self, Self::PowerLineFrequency)
    }
}

impl fmt::Display for ControlId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.key())
    }
}

/// Returned when a string does not name a known control.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown control `{0}`")]
pub struct UnknownControl(pub String);

impl FromStr for ControlId {
    type Err = UnknownControl;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let normalized = s.trim().to_ascii_lowercase().replace(['-', ' '], "_");
        Self::ALL.into_iter().find(|id| id.key() == normalized).ok_or_else(|| UnknownControl(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_unique_and_round_trip() {
        let mut keys: Vec<_> = ControlId::ALL.iter().map(|id| id.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), ControlId::ALL.len());
        for id in ControlId::ALL {
            assert_eq!(id.key().parse::<ControlId>(), Ok(id));
            assert_eq!(id.to_string(), id.key());
        }
    }

    #[test]
    fn parsing_accepts_human_spellings() {
        assert_eq!("White Balance".parse::<ControlId>(), Ok(ControlId::WhiteBalance));
        assert_eq!(" power-line-frequency ".parse::<ControlId>(), Ok(ControlId::PowerLineFrequency));
        assert_eq!("BRIGHTNESS".parse::<ControlId>(), Ok(ControlId::Brightness));
    }

    #[test]
    fn parsing_rejects_unknown_names() {
        let err = "iris".parse::<ControlId>().unwrap_err();
        assert_eq!(err, UnknownControl("iris".into()));
        assert_eq!(err.to_string(), "unknown control `iris`");
    }

    #[test]
    fn groups_cover_every_control() {
        use ControlGroup::{Image, Light, Optics};
        let expected = [
            Image, Image, Image, Image, Image, Image, Light, Light, Light, Light, Light, Optics, Optics,
            Optics, Optics,
        ];
        let actual: Vec<_> = ControlId::ALL.iter().map(|id| id.group()).collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn only_power_line_frequency_is_a_choice() {
        let choices: Vec<_> = ControlId::ALL.into_iter().filter(|id| id.is_choice()).collect();
        assert_eq!(choices, [ControlId::PowerLineFrequency]);
    }

    #[test]
    fn serde_uses_snake_case_keys() {
        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct Wrapper {
            id: ControlId,
        }
        let encoded = toml::to_string(&Wrapper { id: ControlId::BacklightCompensation }).unwrap();
        assert_eq!(encoded.trim(), r#"id = "backlight_compensation""#);
        assert_eq!(toml::from_str::<Wrapper>(&encoded).unwrap().id, ControlId::BacklightCompensation);
    }
}
