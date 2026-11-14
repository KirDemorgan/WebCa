#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod command;
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
pub const ASSOCIATE_FLAG: &str = "--associate";
const DISSOCIATE_FLAG: &str = "--dissociate";

/// Holds the single-instance lock for as long as the application runs.
struct InstanceLock(#[allow(dead_code)] Option<PrimaryInstance>);

impl Global for InstanceLock {}

fn profile_path() -> PathBuf {
    std::env::var_os("APPDATA")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join("WebCa")
        .join("profiles.toml")
}

/// Runs a maintenance request from an elevated copy of WebCa and returns the process exit code.
fn run_elevated_request(arguments: &[String]) -> Option<i32> {
    let result = match arguments {
        [_, flag, device, family] if flag == ASSOCIATE_FLAG => webca_windows::associate(device, family),
        [_, flag, device] if flag == DISSOCIATE_FLAG => webca_windows::dissociate(device),
        _ => return None,
    };
    Some(i32::from(result.is_err()))
}

fn main() {
    let arguments: Vec<String> = std::env::args().collect();
    if let Some(code) = run_elevated_request(&arguments) {
        std::process::exit(code);
    }
    if let Some(request) = webca_core::cli::parse(arguments.get(1..).unwrap_or_default())
        .filter(|_| arguments.get(1).is_none_or(|first| first != AGENT_FLAG))
    {
        let code = match request {
            Ok(request) => command::run(&request, &ProfileStore::new(profile_path())),
            Err(error) => {
                webca_windows::attach_parent_console();
                eprintln!(
                    "webca: {error}

{}",
                    webca_core::cli::USAGE
                );
                2
            }
        };
        std::process::exit(code);
    }
    let start_hidden = arguments.iter().any(|argument| argument == AGENT_FLAG);
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
