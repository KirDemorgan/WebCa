use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use futures::channel::mpsc::UnboundedSender;
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem},
};

use crate::shell::ShellEvent;

const ICON_SIZE: u32 = 32;
const ACCENT: [u8; 3] = [0x6E, 0x8B, 0xFF];

/// What the tray menu currently shows.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TrayState {
    pub profiles: Vec<String>,
    pub active: Option<String>,
    pub lock: bool,
}

/// The notification-area icon and its menu.
pub struct Tray {
    icon: TrayIcon,
    actions: Arc<Mutex<HashMap<MenuId, ShellEvent>>>,
}

impl Tray {
    pub fn new(
        events: UnboundedSender<ShellEvent>,
        state: &TrayState,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let actions: Arc<Mutex<HashMap<MenuId, ShellEvent>>> = Arc::default();
        let menu_actions = Arc::clone(&actions);
        let menu_events = events.clone();
        MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
            let action = menu_actions.lock().ok().and_then(|actions| actions.get(&event.id).cloned());
            if let Some(action) = action {
                let _ = menu_events.unbounded_send(action);
            }
        }));
        TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let _ = events.unbounded_send(ShellEvent::Open);
            }
        }));
        let icon = TrayIconBuilder::new()
            .with_tooltip("WebCa")
            .with_icon(Icon::from_rgba(icon_pixels(ICON_SIZE), ICON_SIZE, ICON_SIZE)?)
            .with_menu_on_left_click(false)
            .build()?;
        let tray = Self { icon, actions };
        tray.update(state);
        Ok(tray)
    }

    /// Rebuilds the menu from `state`.
    pub fn update(&self, state: &TrayState) {
        let menu = Menu::new();
        let mut actions = HashMap::new();
        for name in &state.profiles {
            let checked = state.active.as_deref() == Some(name.as_str());
            let item = CheckMenuItem::new(name, true, checked, None);
            actions.insert(item.id().clone(), ShellEvent::SelectProfile(name.clone()));
            let _ = menu.append(&item);
        }
        if !state.profiles.is_empty() {
            let _ = menu.append(&PredefinedMenuItem::separator());
        }
        let lock = CheckMenuItem::new("Lock values", true, state.lock, None);
        actions.insert(lock.id().clone(), ShellEvent::ToggleLock);
        let open = MenuItem::new("Open WebCa", true, None);
        actions.insert(open.id().clone(), ShellEvent::Open);
        let quit = MenuItem::new("Quit", true, None);
        actions.insert(quit.id().clone(), ShellEvent::Quit);
        let _ = menu.append_items(&[&lock, &PredefinedMenuItem::separator(), &open, &quit]);
        if let Ok(mut current) = self.actions.lock() {
            *current = actions;
        }
        self.icon.set_menu(Some(Box::new(menu)));
    }
}

/// A rounded accent square with a camera lens, as RGBA pixels.
pub fn icon_pixels(size: u32) -> Vec<u8> {
    let extent = f64::from(size);
    let center = extent / 2.0;
    let corner = extent * 0.22;
    let mut pixels = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let (px, py) = (f64::from(x) + 0.5, f64::from(y) + 0.5);
            let dx = (px - center).abs() - (center - corner);
            let dy = (py - center).abs() - (center - corner);
            let outside_corner = dx > 0.0 && dy > 0.0 && dx.hypot(dy) > corner;
            let distance = (px - center).hypot(py - center);
            let lens_ring = distance > extent * 0.18 && distance < extent * 0.30;
            let rgba = match (outside_corner, lens_ring) {
                (true, _) => [0, 0, 0, 0],
                (false, true) => [0xFF, 0xFF, 0xFF, 0xFF],
                (false, false) => [ACCENT[0], ACCENT[1], ACCENT[2], 0xFF],
            };
            pixels.extend_from_slice(&rgba);
        }
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(pixels: &[u8], size: u32, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * size + x) * 4) as usize;
        pixels[index..index + 4].try_into().unwrap()
    }

    #[test]
    fn icon_has_transparent_corners_lens_and_accent_fill() {
        let pixels = icon_pixels(ICON_SIZE);
        assert_eq!(pixels.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
        assert_eq!(pixel(&pixels, ICON_SIZE, 0, 0)[3], 0);
        assert_eq!(pixel(&pixels, ICON_SIZE, 16, 8), [0xFF, 0xFF, 0xFF, 0xFF]);
        assert_eq!(pixel(&pixels, ICON_SIZE, 16, 16), [0x6E, 0x8B, 0xFF, 0xFF]);
        assert_eq!(pixel(&pixels, ICON_SIZE, 2, 16), [0x6E, 0x8B, 0xFF, 0xFF]);
    }
}
