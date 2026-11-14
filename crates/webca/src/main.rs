#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod labels;
mod studio;
mod worker;

use std::path::PathBuf;

use gpui_kit::{
    AppContext, Bounds, WindowBounds, WindowOptions,
    component::{Theme, TitleBar},
    px, size,
};
use webca_core::ProfileStore;

use crate::studio::Studio;

fn profile_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join("WebCa")
        .join("profiles.toml")
}

fn main() {
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).run(|cx| {
        gpui_kit::init(cx);
        Theme::sync_system_appearance(None, cx);
        let bounds = Bounds::centered(None, size(px(1040.), px(660.)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(860.), px(560.))),
            app_id: Some("WebCa".into()),
            ..TitleBar::window_options()
        };
        let store = ProfileStore::new(profile_path());
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Studio::new(store, window, cx)))
            .expect("the main window opens");
        cx.activate(true);
    });
}
