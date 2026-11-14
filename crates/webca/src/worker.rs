use std::{sync::mpsc, thread};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use webca_core::{CameraInfo, ControlState, DeviceError, LiveWrite, snapshot};
use webca_windows::{CameraEntry, DefaultsError, DefaultsStore, MediaSession, WindowsCamera, list_cameras};

/// Requests for the thread that owns the camera.
#[derive(Debug)]
pub enum Command {
    Write(Vec<LiveWrite>),
    SaveDefaults(Vec<LiveWrite>),
}

/// Results reported back to the interface.
#[derive(Debug)]
pub enum Event {
    Opened { info: CameraInfo, controls: Vec<ControlState> },
    NoCamera,
    Failed(String),
    DefaultsSaved,
    DefaultsUnavailable,
    DefaultsFailed(String),
    WriteFailed(String),
}

/// Handle to the camera thread; dropping it stops the thread.
#[derive(Debug)]
pub struct CameraWorker {
    commands: mpsc::Sender<Command>,
}

impl CameraWorker {
    pub fn spawn() -> (Self, UnboundedReceiver<Event>) {
        let (commands, inbox) = mpsc::channel();
        let (events, outbox) = unbounded();
        thread::Builder::new()
            .name("webca-camera".into())
            .spawn(move || run(&inbox, &events))
            .expect("the camera thread starts");
        (Self { commands }, outbox)
    }

    pub fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }
}

fn run(inbox: &mpsc::Receiver<Command>, events: &UnboundedSender<Event>) {
    let emit = |event| {
        let _ = events.unbounded_send(event);
    };
    let session = match MediaSession::start() {
        Ok(session) => session,
        Err(error) => return emit(Event::Failed(error.to_string())),
    };
    let entry = match list_cameras() {
        Ok(cameras) => match cameras.into_iter().next() {
            Some(entry) => entry,
            None => return emit(Event::NoCamera),
        },
        Err(error) => return emit(Event::Failed(error.to_string())),
    };
    let camera = match open(&entry) {
        Ok((camera, controls)) => {
            emit(Event::Opened { info: entry.info.clone(), controls });
            camera
        }
        Err(error) => return emit(Event::Failed(error.to_string())),
    };
    while let Ok(command) = inbox.recv() {
        match command {
            Command::Write(writes) => {
                let failures: Vec<_> = writes
                    .into_iter()
                    .filter_map(|(id, setting)| webca_core::CameraDevice::write(&camera, id, setting).err())
                    .collect();
                if let Some(error) = failures.first() {
                    emit(Event::WriteFailed(error.to_string()));
                }
            }
            Command::SaveDefaults(settings) => emit(save_defaults(&entry, &settings)),
        }
    }
    drop(camera);
    drop(session);
}

fn open(entry: &CameraEntry) -> Result<(WindowsCamera, Vec<ControlState>), DeviceError> {
    let camera = WindowsCamera::open(entry)?;
    let controls = snapshot(&camera)?;
    Ok((camera, controls))
}

fn save_defaults(entry: &CameraEntry, settings: &[LiveWrite]) -> Event {
    let result = DefaultsStore::open().and_then(|store| store.save(entry, settings));
    match result {
        Ok(()) => Event::DefaultsSaved,
        Err(DefaultsError::NotAuthorized) => Event::DefaultsUnavailable,
        Err(error) => Event::DefaultsFailed(error.to_string()),
    }
}
