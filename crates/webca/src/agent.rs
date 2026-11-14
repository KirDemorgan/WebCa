use std::{
    sync::mpsc::{self, RecvTimeoutError, Sender},
    thread,
    time::{Duration, SystemTime},
};

use webca_core::{
    Action, CameraDevice, ControlId, Guard, GuardConfig, Profile, ProfileBook, ProfileStore, Signal, apply,
};
use webca_windows::{
    ActivityMonitor, CameraEntry, ControlMonitor, MediaSession, WindowsCamera, list_cameras,
};

const FILE_POLL_INTERVAL: Duration = Duration::from_secs(2);

/// Requests from the interface to the background agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCommand {
    /// The settings window is open: the user is editing live, so the agent must not interfere.
    Pause,
    Resume,
    Reload,
}

/// Handle to the agent thread; the thread ends with the process.
#[derive(Debug, Clone)]
pub struct Agent {
    inputs: Sender<Input>,
}

#[derive(Clone, Copy)]
enum Input {
    Command(AgentCommand),
    Streaming(bool),
    ControlChanged(ControlId),
}

impl Agent {
    pub fn spawn(store: ProfileStore) -> Self {
        let (inputs, inbox) = mpsc::channel();
        let sender = inputs.clone();
        thread::Builder::new()
            .name("webca-agent".into())
            .spawn(move || run(&store, &sender, &inbox))
            .expect("the agent thread starts");
        Self { inputs }
    }

    pub fn send(&self, command: AgentCommand) {
        let _ = self.inputs.send(Input::Command(command));
    }
}

struct Session {
    entry: CameraEntry,
    camera: WindowsCamera,
    guard: Guard,
    lock_monitor: Option<ControlMonitor>,
    paused: bool,
}

fn run(store: &ProfileStore, sender: &Sender<Input>, inbox: &mpsc::Receiver<Input>) {
    let Ok(_media) = MediaSession::start() else { return };
    let Some(entry) = list_cameras().ok().and_then(|cameras| cameras.into_iter().next()) else { return };
    let Ok(camera) = WindowsCamera::open(&entry) else { return };
    let own_process = std::process::id();
    let device_path = entry.device_path.to_lowercase();
    let activity_sender = sender.clone();
    let Ok(_activity) = ActivityMonitor::start(move |report| {
        if report.device_path == device_path {
            let others = report.streaming_processes.iter().any(|process| *process != own_process);
            let _ = activity_sender.send(Input::Streaming(others));
        }
    }) else {
        return;
    };

    let (config, profile) = load(store, &entry);
    let mut session =
        Session { entry, camera, guard: Guard::new(config, profile), lock_monitor: None, paused: false };
    session.refresh_lock_monitor(sender);
    let mut stamp = modified(store);
    loop {
        let input = match inbox.recv_timeout(FILE_POLL_INTERVAL) {
            Ok(input) => input,
            Err(RecvTimeoutError::Timeout) => {
                let current = modified(store);
                if current == stamp {
                    continue;
                }
                stamp = current;
                Input::Command(AgentCommand::Reload)
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        session.handle(input, store, sender);
    }
}

impl Session {
    fn handle(&mut self, input: Input, store: &ProfileStore, sender: &Sender<Input>) {
        let signal = match input {
            Input::Command(AgentCommand::Pause) => {
                self.paused = true;
                return;
            }
            Input::Command(AgentCommand::Resume) => {
                self.paused = false;
                return self.reload(store, sender);
            }
            Input::Command(AgentCommand::Reload) => return self.reload(store, sender),
            Input::Streaming(true) if !self.guard.is_streaming() => Signal::StreamStarted,
            Input::Streaming(false) if self.guard.is_streaming() => Signal::StreamStopped,
            Input::Streaming(_) => return,
            Input::ControlChanged(id) => match self.camera.read(id) {
                Ok(setting) => Signal::ControlChanged { id, setting },
                Err(_) => return,
            },
        };
        let action = self.guard.observe(signal);
        self.perform(action);
    }

    fn reload(&mut self, store: &ProfileStore, sender: &Sender<Input>) {
        let (config, profile) = load(store, &self.entry);
        let first = self.guard.observe(Signal::ConfigChanged(config));
        let second = self.guard.observe(Signal::ProfileChanged(profile));
        self.refresh_lock_monitor(sender);
        self.perform(first.or(second));
    }

    fn perform(&self, action: Option<Action>) {
        if self.paused {
            return;
        }
        match action {
            Some(Action::ApplyProfile) => {
                let _ = apply(&self.camera, self.guard.profile());
            }
            Some(Action::Restore(writes)) => {
                for (id, setting) in writes {
                    let _ = self.camera.write(id, setting);
                }
            }
            None => {}
        }
    }

    fn refresh_lock_monitor(&mut self, sender: &Sender<Input>) {
        self.lock_monitor = None;
        if !self.guard.config().lock {
            return;
        }
        let controls: Vec<ControlId> = self.guard.profile().settings().map(|(id, _)| id).collect();
        let changes = sender.clone();
        self.lock_monitor = ControlMonitor::start(&self.entry.device_path, &controls, move |id| {
            let _ = changes.send(Input::ControlChanged(id));
        })
        .ok();
    }
}

fn load(store: &ProfileStore, entry: &CameraEntry) -> (GuardConfig, Profile) {
    let book = store.load().unwrap_or_else(|_| ProfileBook::new());
    let settings = book.agent();
    let profile = book.camera(&entry.info.key).map_or_else(
        || Profile::new("Default").expect("default profile name is valid"),
        |camera| camera.active().clone(),
    );
    (GuardConfig { native: settings.native, lock: settings.lock }, profile)
}

fn modified(store: &ProfileStore) -> Option<SystemTime> {
    std::fs::metadata(store.path()).and_then(|metadata| metadata.modified()).ok()
}
