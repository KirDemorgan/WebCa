use webca_core::{ControlGroup, ControlId};

pub const fn control(id: ControlId) -> &'static str {
    match id {
        ControlId::Brightness => "Brightness",
        ControlId::Contrast => "Contrast",
        ControlId::Hue => "Hue",
        ControlId::Saturation => "Saturation",
        ControlId::Sharpness => "Sharpness",
        ControlId::Gamma => "Gamma",
        ControlId::WhiteBalance => "White balance",
        ControlId::BacklightCompensation => "Backlight compensation",
        ControlId::Gain => "Gain",
        ControlId::PowerLineFrequency => "Anti-flicker",
        ControlId::Exposure => "Exposure",
        ControlId::Focus => "Focus",
        ControlId::Zoom => "Zoom",
        ControlId::Pan => "Pan",
        ControlId::Tilt => "Tilt",
    }
}

pub const fn group(group: ControlGroup) -> &'static str {
    match group {
        ControlGroup::Image => "IMAGE",
        ControlGroup::Light => "LIGHT",
        ControlGroup::Optics => "OPTICS",
    }
}

pub fn value(id: ControlId, value: i32) -> String {
    match id {
        ControlId::WhiteBalance => format!("{value} K"),
        ControlId::Exposure if value < 0 => format!("\u{2212}{}", value.unsigned_abs()),
        _ => value.to_string(),
    }
}

/// Labels of the anti-flicker choices, indexed by the UVC power line frequency value.
pub const POWER_LINE_CHOICES: [&str; 3] = ["Off", "50 Hz", "60 Hz"];
