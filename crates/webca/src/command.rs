use std::io::{BufRead, Write};

use webca_core::{
    CameraDevice, CameraProfiles, ControlSetting, ProfileBook, ProfileStore, apply,
    cli::{self, Request, SetValue},
    mcp::McpServer,
    snapshot,
};
use webca_windows::{MediaSession, WindowsCamera, list_cameras};

/// Runs a command-line request and returns the process exit code.
pub fn run(request: &Request, store: &ProfileStore) -> i32 {
    if *request != Request::Mcp {
        webca_windows::attach_parent_console();
    }
    if *request == Request::Help {
        println!("{}", cli::USAGE);
        return 0;
    }
    match execute(request, store) {
        Ok(()) => 0,
        Err(message) => {
            eprintln!("webca: {message}");
            1
        }
    }
}

fn execute(request: &Request, store: &ProfileStore) -> Result<(), String> {
    let _session = MediaSession::start().map_err(|error| error.to_string())?;
    let entry = list_cameras()
        .map_err(|error| error.to_string())?
        .into_iter()
        .next()
        .ok_or("no camera is connected")?;
    let camera = WindowsCamera::open(&entry).map_err(|error| error.to_string())?;
    let book = store.load().unwrap_or_else(|_| ProfileBook::new());
    let profiles = book.camera(&entry.info.key).cloned();
    match request {
        Request::List => {
            print!("{}", cli::format_controls(&snapshot(&camera).map_err(|error| error.to_string())?));
        }
        Request::Get(id) => {
            let setting = camera.read(*id).map_err(|error| error.to_string())?;
            println!("{}{}", setting.value, if setting.is_auto() { " auto" } else { "" });
        }
        Request::Set(id, value) => {
            let setting = match value {
                SetValue::Value(value) => ControlSetting::manual(*value),
                SetValue::Auto => {
                    ControlSetting::auto(camera.read(*id).map_err(|error| error.to_string())?.value)
                }
            };
            camera.write(*id, setting).map_err(|error| error.to_string())?;
        }
        Request::Profiles => print!("{}", cli::format_profiles(&profiles.unwrap_or_default())),
        Request::Apply(name) => apply_profile(&camera, profiles, name.as_deref())?,
        Request::Mcp => serve_mcp(&camera, profiles),
        Request::Help => {}
    }
    Ok(())
}

fn apply_profile(
    camera: &WindowsCamera,
    profiles: Option<CameraProfiles>,
    name: Option<&str>,
) -> Result<(), String> {
    let mut profiles = profiles.ok_or("no profiles are saved for this camera")?;
    if let Some(name) = name {
        profiles.select(name).map_err(|error| error.to_string())?;
    }
    let report = apply(camera, profiles.active()).map_err(|error| error.to_string())?;
    println!("Applied `{}` to {} controls", profiles.active().name(), report.applied.len());
    Ok(())
}

fn serve_mcp(camera: &dyn CameraDevice, profiles: Option<CameraProfiles>) {
    let mut server = McpServer::new(camera, profiles);
    let stdout = std::io::stdout();
    for line in std::io::stdin().lock().lines().map_while(Result::ok) {
        if let Some(response) = server.handle(&line) {
            let mut out = stdout.lock();
            let _ = writeln!(out, "{response}");
            let _ = out.flush();
        }
    }
}
