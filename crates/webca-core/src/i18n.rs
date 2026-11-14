use crate::{ControlGroup, ControlId};

/// Interface language.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Locale {
    #[default]
    English,
    Russian,
}

impl Locale {
    /// Picks the language for a BCP 47 tag such as `ru-RU`; unknown languages fall back to English.
    #[must_use]
    pub fn from_tag(tag: &str) -> Self {
        let language = tag.split(['-', '_']).next().unwrap_or_default();
        if language.eq_ignore_ascii_case("ru") { Self::Russian } else { Self::English }
    }
}

/// Every fixed phrase of the interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Text {
    LookingForCamera,
    NoCamera,
    CannotOpenCamera,
    PreviewStarting,
    PreviewUnavailable,
    UnsavedChanges,
    EverythingSaved,
    Saving,
    SavedNative,
    SavedAgent,
    SavedButRefused,
    NativeOn,
    TurnOnNative,
    NativeNeedsInstall,
    WaitingForPermission,
    CompanionRejected,
    Auto,
    Reset,
    Revert,
    Save,
    LockValues,
    OpenWebCa,
    Quit,
    UpdateAvailable,
}

impl Text {
    pub const ALL: [Self; 24] = [
        Self::LookingForCamera,
        Self::NoCamera,
        Self::CannotOpenCamera,
        Self::PreviewStarting,
        Self::PreviewUnavailable,
        Self::UnsavedChanges,
        Self::EverythingSaved,
        Self::Saving,
        Self::SavedNative,
        Self::SavedAgent,
        Self::SavedButRefused,
        Self::NativeOn,
        Self::TurnOnNative,
        Self::NativeNeedsInstall,
        Self::WaitingForPermission,
        Self::CompanionRejected,
        Self::Auto,
        Self::Reset,
        Self::Revert,
        Self::Save,
        Self::LockValues,
        Self::OpenWebCa,
        Self::Quit,
        Self::UpdateAvailable,
    ];
}

#[must_use]
pub const fn text(locale: Locale, text: Text) -> &'static str {
    match locale {
        Locale::English => english(text),
        Locale::Russian => russian(text),
    }
}

const fn english(text: Text) -> &'static str {
    match text {
        Text::LookingForCamera => "Looking for your camera…",
        Text::NoCamera => "No camera is connected.",
        Text::CannotOpenCamera => "Cannot open the camera:",
        Text::PreviewStarting => "Starting preview…",
        Text::PreviewUnavailable => "Preview is unavailable:",
        Text::UnsavedChanges => "Unsaved changes",
        Text::EverythingSaved => "Everything is saved",
        Text::Saving => "Saving…",
        Text::SavedNative => "Saved. Windows applies it in every app.",
        Text::SavedAgent => "Saved. WebCa keeps it applied from the tray.",
        Text::SavedButRefused => "Saved, but Windows refused:",
        Text::NativeOn => "System-wide mode is on",
        Text::TurnOnNative => "Turn on system-wide mode",
        Text::NativeNeedsInstall => "System-wide mode is available in the installed version of WebCa.",
        Text::WaitingForPermission => "Waiting for permission…",
        Text::CompanionRejected => "Windows did not accept WebCa as the camera companion.",
        Text::Auto => "Auto",
        Text::Reset => "Reset",
        Text::Revert => "Revert",
        Text::Save => "Save",
        Text::LockValues => "Lock values",
        Text::OpenWebCa => "Open WebCa",
        Text::Quit => "Quit",
        Text::UpdateAvailable => "Update available:",
    }
}

const fn russian(text: Text) -> &'static str {
    match text {
        Text::LookingForCamera => "Ищем камеру…",
        Text::NoCamera => "Камера не подключена.",
        Text::CannotOpenCamera => "Не удалось открыть камеру:",
        Text::PreviewStarting => "Запускаем превью…",
        Text::PreviewUnavailable => "Превью недоступно:",
        Text::UnsavedChanges => "Есть несохранённые изменения",
        Text::EverythingSaved => "Всё сохранено",
        Text::Saving => "Сохраняем…",
        Text::SavedNative => "Сохранено. Windows применяет настройки во всех приложениях.",
        Text::SavedAgent => "Сохранено. WebCa поддерживает настройки из трея.",
        Text::SavedButRefused => "Сохранено, но Windows отказала:",
        Text::NativeOn => "Системный режим включён",
        Text::TurnOnNative => "Включить системный режим",
        Text::NativeNeedsInstall => "Системный режим доступен в установленной версии WebCa.",
        Text::WaitingForPermission => "Ждём разрешения…",
        Text::CompanionRejected => "Windows не приняла WebCa как приложение камеры.",
        Text::Auto => "Авто",
        Text::Reset => "Сбросить",
        Text::Revert => "Отменить",
        Text::Save => "Сохранить",
        Text::LockValues => "Закрепить значения",
        Text::OpenWebCa => "Открыть WebCa",
        Text::Quit => "Выход",
        Text::UpdateAvailable => "Доступно обновление:",
    }
}

#[must_use]
pub const fn control_label(locale: Locale, id: ControlId) -> &'static str {
    match (locale, id) {
        (Locale::English, ControlId::Brightness) => "Brightness",
        (Locale::English, ControlId::Contrast) => "Contrast",
        (Locale::English, ControlId::Hue) => "Hue",
        (Locale::English, ControlId::Saturation) => "Saturation",
        (Locale::English, ControlId::Sharpness) => "Sharpness",
        (Locale::English, ControlId::Gamma) => "Gamma",
        (Locale::English, ControlId::WhiteBalance) => "White balance",
        (Locale::English, ControlId::BacklightCompensation) => "Backlight compensation",
        (Locale::English, ControlId::Gain) => "Gain",
        (Locale::English, ControlId::PowerLineFrequency) => "Anti-flicker",
        (Locale::English, ControlId::Exposure) => "Exposure",
        (Locale::English, ControlId::Focus) => "Focus",
        (Locale::English, ControlId::Zoom) => "Zoom",
        (Locale::English, ControlId::Pan) => "Pan",
        (Locale::English, ControlId::Tilt) => "Tilt",
        (Locale::Russian, ControlId::Brightness) => "Яркость",
        (Locale::Russian, ControlId::Contrast) => "Контрастность",
        (Locale::Russian, ControlId::Hue) => "Оттенок",
        (Locale::Russian, ControlId::Saturation) => "Насыщенность",
        (Locale::Russian, ControlId::Sharpness) => "Резкость",
        (Locale::Russian, ControlId::Gamma) => "Гамма",
        (Locale::Russian, ControlId::WhiteBalance) => "Баланс белого",
        (Locale::Russian, ControlId::BacklightCompensation) => "Компенсация подсветки",
        (Locale::Russian, ControlId::Gain) => "Усиление",
        (Locale::Russian, ControlId::PowerLineFrequency) => "Антимерцание",
        (Locale::Russian, ControlId::Exposure) => "Экспозиция",
        (Locale::Russian, ControlId::Focus) => "Фокус",
        (Locale::Russian, ControlId::Zoom) => "Зум",
        (Locale::Russian, ControlId::Pan) => "Панорама",
        (Locale::Russian, ControlId::Tilt) => "Наклон",
    }
}

#[must_use]
pub const fn group_label(locale: Locale, group: ControlGroup) -> &'static str {
    match (locale, group) {
        (Locale::English, ControlGroup::Image) => "IMAGE",
        (Locale::English, ControlGroup::Light) => "LIGHT",
        (Locale::English, ControlGroup::Optics) => "OPTICS",
        (Locale::Russian, ControlGroup::Image) => "ИЗОБРАЖЕНИЕ",
        (Locale::Russian, ControlGroup::Light) => "СВЕТ",
        (Locale::Russian, ControlGroup::Optics) => "ОПТИКА",
    }
}

/// Label of an anti-flicker choice by its UVC power line frequency value.
#[must_use]
pub fn power_line_label(locale: Locale, value: i32) -> String {
    match (locale, value) {
        (Locale::English, 0) => "Off".to_owned(),
        (Locale::Russian, 0) => "Выкл".to_owned(),
        (_, 1) => "50 Hz".to_owned(),
        (_, 2) => "60 Hz".to_owned(),
        (_, other) => other.to_string(),
    }
}

/// A control value as shown next to its slider.
#[must_use]
pub fn value_label(id: ControlId, value: i32) -> String {
    match id {
        ControlId::WhiteBalance => format!("{value} K"),
        ControlId::Exposure if value < 0 => format!("\u{2212}{}", value.unsigned_abs()),
        _ => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCALES: [Locale; 2] = [Locale::English, Locale::Russian];

    #[test]
    fn locale_follows_the_language_part_of_the_tag() {
        assert_eq!(Locale::from_tag("ru-RU"), Locale::Russian);
        assert_eq!(Locale::from_tag("RU"), Locale::Russian);
        assert_eq!(Locale::from_tag("ru_KZ"), Locale::Russian);
        assert_eq!(Locale::from_tag("en-US"), Locale::English);
        assert_eq!(Locale::from_tag("rue-UA"), Locale::English);
        assert_eq!(Locale::from_tag(""), Locale::English);
        assert_eq!(Locale::default(), Locale::English);
    }

    #[test]
    fn every_phrase_is_translated() {
        for item in Text::ALL {
            let (english, russian) = (text(Locale::English, item), text(Locale::Russian, item));
            assert!(!english.is_empty() && !russian.is_empty(), "{item:?}");
            assert_ne!(english, russian, "{item:?} is not translated");
        }
    }

    #[test]
    fn every_control_and_group_has_labels() {
        for locale in LOCALES {
            for id in ControlId::ALL {
                assert!(!control_label(locale, id).is_empty());
            }
            for group in [ControlGroup::Image, ControlGroup::Light, ControlGroup::Optics] {
                assert!(!group_label(locale, group).is_empty());
            }
        }
        assert_eq!(control_label(Locale::Russian, ControlId::WhiteBalance), "Баланс белого");
        assert_eq!(group_label(Locale::Russian, ControlGroup::Optics), "ОПТИКА");
    }

    #[test]
    fn power_line_and_values_are_formatted() {
        assert_eq!(power_line_label(Locale::English, 0), "Off");
        assert_eq!(power_line_label(Locale::Russian, 0), "Выкл");
        assert_eq!(power_line_label(Locale::Russian, 1), "50 Hz");
        assert_eq!(power_line_label(Locale::English, 2), "60 Hz");
        assert_eq!(power_line_label(Locale::English, 3), "3");
        assert_eq!(value_label(ControlId::WhiteBalance, 4600), "4600 K");
        assert_eq!(value_label(ControlId::Exposure, -6), "\u{2212}6");
        assert_eq!(value_label(ControlId::Exposure, 0), "0");
        assert_eq!(value_label(ControlId::Gamma, 5), "5");
    }
}
