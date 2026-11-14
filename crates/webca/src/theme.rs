use std::rc::Rc;

use gpui_kit::{
    App,
    component::{Theme, ThemeConfig, ThemeMode, ThemeSet},
};

const THEME_SET: &str = include_str!("../assets/theme.json");

/// Installs the WebCa palette for both appearances and follows the system appearance.
pub fn install(cx: &mut App) {
    let set: ThemeSet = serde_json::from_str(THEME_SET).expect("the bundled theme is valid JSON");
    let pick = |mode: ThemeMode| -> Rc<ThemeConfig> {
        Rc::new(
            set.themes.iter().find(|theme| theme.mode == mode).expect("both appearances are bundled").clone(),
        )
    };
    let (light, dark) = (pick(ThemeMode::Light), pick(ThemeMode::Dark));
    Theme::update(cx, |theme| {
        theme.light_theme = light;
        theme.dark_theme = dark;
    });
    Theme::sync_system_appearance(None, cx);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_theme_has_both_appearances() {
        let set: ThemeSet = serde_json::from_str(THEME_SET).unwrap();
        let modes: Vec<_> = set.themes.iter().map(|theme| theme.mode).collect();
        assert_eq!(modes, [ThemeMode::Light, ThemeMode::Dark]);
    }
}
