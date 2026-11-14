#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod labels;
mod shell;
mod studio;
mod theme;
mod tray;
mod worker;

use std::path::PathBuf;

use futures::channel::mpsc::unbounded;
use gpui_kit::{Global, QuitMode};
use webca_core::ProfileStore;
use webca_windows::{Instance, PrimaryInstance};

use crate::{
    agent::Agent,
    shell::{Shell, ShellEvent},
};

const AGENT_FLAG: &str = "--agent";

/// Holds the single-instance lock for as long as the application runs.
struct InstanceLock(#[allow(dead_code)] Option<PrimaryInstance>);

impl Global for InstanceLock {}

fn profile_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join("WebCa")
        .join("profiles.toml")
}

fn main() {
    let start_hidden = std::env::args().any(|argument| argument == AGENT_FLAG);
    let instance = match Instance::acquire() {
        Ok(Instance::Secondary) => return,
        Ok(Instance::Primary(primary)) => Some(primary),
        Err(_) => None,
    };
    gpui_kit::application().with_assets(gpui_kit::assets::Assets).with_quit_mode(QuitMode::Explicit).run(
        move |cx| {
            gpui_kit::init(cx);
            theme::install(cx);
            let store = ProfileStore::new(profile_path());
            let agent = Agent::spawn(store.clone());
            let (events, inbox) = unbounded();
            if let Some(instance) = instance.as_ref() {
                let show = events.clone();
                instance.on_show_request(move || {
                    let _ = show.unbounded_send(ShellEvent::Open);
                });
            }
            cx.set_global(InstanceLock(instance));
            Shell::install(store, agent, events, inbox, cx);
            if !start_hidden {
                shell::open_window(cx);
            }
        },
    );
}
