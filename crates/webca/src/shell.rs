use futures::{
    StreamExt,
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
};
use gpui_kit::{
    AnyWindowHandle, App, AppContext, Bounds, Global, WindowBounds, WindowOptions, component::TitleBar, px,
    size,
};
use webca_core::{AgentSettings, ProfileBook, ProfileStore};

use crate::{
    agent::{Agent, AgentCommand},
    studio::Studio,
    tray::{Tray, TrayState},
};

/// Requests handled by the application shell on the main thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEvent {
    Open,
    Quit,
    ToggleLock,
    SelectProfile(String),
    ProfilesChanged,
}

/// Process-wide services shared by the tray and the settings window.
pub struct Shell {
    pub store: ProfileStore,
    pub agent: Agent,
    pub events: UnboundedSender<ShellEvent>,
    window: Option<AnyWindowHandle>,
    tray: Option<Tray>,
}

impl Global for Shell {}

impl Shell {
    pub fn install(
        store: ProfileStore,
        agent: Agent,
        events: UnboundedSender<ShellEvent>,
        inbox: UnboundedReceiver<ShellEvent>,
        cx: &mut App,
    ) {
        let tray = Tray::new(events.clone(), &tray_state(&store)).ok();
        cx.set_global(Self { store, agent, events, window: None, tray });
        cx.spawn(async move |cx| {
            let mut inbox = inbox;
            while let Some(event) = inbox.next().await {
                cx.update(|cx| handle(event, cx));
            }
        })
        .detach();
    }

    pub fn notify(cx: &App, event: ShellEvent) {
        let _ = cx.global::<Self>().events.unbounded_send(event);
    }
}

fn handle(event: ShellEvent, cx: &mut App) {
    match event {
        ShellEvent::Open => open_window(cx),
        ShellEvent::Quit => cx.quit(),
        ShellEvent::ToggleLock => edit_book(cx, |book| {
            let settings = book.agent();
            book.set_agent(AgentSettings { lock: !settings.lock, ..settings });
        }),
        ShellEvent::SelectProfile(name) => edit_book(cx, |book| {
            let first_camera = book.cameras().next().map(|(key, _)| key.clone());
            if let Some(key) = first_camera {
                let _ = book.camera_mut(&key).select(&name);
            }
        }),
        ShellEvent::ProfilesChanged => refresh_tray(cx),
    }
}

fn edit_book(cx: &mut App, change: impl FnOnce(&mut ProfileBook)) {
    let shell = cx.global::<Shell>();
    let Ok(mut book) = shell.store.load() else { return };
    change(&mut book);
    if shell.store.save(&book).is_ok() {
        shell.agent.send(AgentCommand::Reload);
    }
    refresh_tray(cx);
}

fn refresh_tray(cx: &mut App) {
    let shell = cx.global::<Shell>();
    if let Some(tray) = &shell.tray {
        tray.update(&tray_state(&shell.store));
    }
}

fn tray_state(store: &ProfileStore) -> TrayState {
    let book = store.load().unwrap_or_else(|_| ProfileBook::new());
    let camera = book.cameras().next().map(|(_, camera)| camera);
    TrayState {
        profiles: camera
            .map(|camera| camera.profiles().map(|p| p.name().to_owned()).collect())
            .unwrap_or_default(),
        active: camera.map(|camera| camera.active().name().to_owned()),
        lock: book.agent().lock,
    }
}

/// Brings the settings window to the front, opening it when needed.
pub fn open_window(cx: &mut App) {
    if let Some(handle) = cx.global::<Shell>().window
        && handle.update(cx, |_, window, _| window.activate_window()).is_ok()
    {
        return;
    }
    let bounds = Bounds::centered(None, size(px(1040.), px(660.)), cx);
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(860.), px(560.))),
        app_id: Some("WebCa".into()),
        ..TitleBar::window_options()
    };
    let store = cx.global::<Shell>().store.clone();
    let agent = cx.global::<Shell>().agent.clone();
    if let Ok((handle, _)) =
        gpui_kit::open_window(options, cx, |window, cx| cx.new(|cx| Studio::new(store, agent, window, cx)))
    {
        cx.global_mut::<Shell>().window = Some(handle);
        cx.activate(true);
    }
}
