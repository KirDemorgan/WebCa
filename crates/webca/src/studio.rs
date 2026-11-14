use std::{collections::BTreeMap, sync::Arc};

use futures::{StreamExt, channel::mpsc::UnboundedReceiver};
use gpui_kit::component::{
    ActiveTheme, Disableable, Sizable, Theme, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    slider::{Slider, SliderEvent, SliderState},
    switch::Switch,
    v_flex,
};
use gpui_kit::{
    AnyElement, App, AppContext, Context, Entity, FontWeight, IntoElement, ObjectFit, ParentElement, Render,
    RenderImage, SharedString, Styled, StyledImage, Subscription, Window, div, img, prelude::*, px,
};
use webca_core::{
    AgentSettings, CameraProfiles, ControlGroup, ControlId, ControlState, Editor, LiveWrite, ProfileBook,
    ProfileStore,
};

use crate::{
    agent::{Agent, AgentCommand},
    labels,
    shell::{Shell, ShellEvent},
    worker::{CameraWorker, Command, Event, FrameSlot},
};

const SIDE_PANEL_WIDTH: f32 = 360.;

enum Screen {
    Connecting,
    Message(SharedString),
    Ready(Box<Editor>),
}

/// The main window: live preview on the left, grouped controls on the right.
pub struct Studio {
    screen: Screen,
    worker: CameraWorker,
    store: ProfileStore,
    agent: Agent,
    book: ProfileBook,
    sliders: BTreeMap<ControlId, Entity<SliderState>>,
    status: Option<SharedString>,
    frames: FrameSlot,
    preview: Option<Arc<RenderImage>>,
    preview_note: Option<SharedString>,
    slider_subscriptions: Vec<Subscription>,
    _appearance: Subscription,
    _release: Subscription,
}

impl Studio {
    pub fn new(store: ProfileStore, agent: Agent, window: &mut Window, cx: &mut Context<Self>) -> Self {
        agent.send(AgentCommand::Pause);
        let release = cx.on_release(|studio: &mut Self, _| studio.agent.send(AgentCommand::Resume));
        let (worker, events, frames) = CameraWorker::spawn();
        let (book, status) = match store.load() {
            Ok(book) => (book, None),
            Err(error) => (ProfileBook::new(), Some(error.to_string().into())),
        };
        Self::listen(events, window, cx);
        let appearance = cx.observe_window_appearance(window, |_, window, cx| {
            Theme::sync_system_appearance(Some(window), cx);
        });
        Self {
            screen: Screen::Connecting,
            worker,
            store,
            agent,
            book,
            sliders: BTreeMap::new(),
            status,
            frames,
            preview: None,
            preview_note: None,
            slider_subscriptions: Vec::new(),
            _appearance: appearance,
            _release: release,
        }
    }

    fn listen(mut events: UnboundedReceiver<Event>, window: &mut Window, cx: &mut Context<Self>) {
        let window_handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            while let Some(event) = events.next().await {
                let updated = window_handle.update(cx, |_, window, cx| {
                    this.update(cx, |studio, cx| studio.handle(event, window, cx)).is_ok()
                });
                if !matches!(updated, Ok(true)) {
                    break;
                }
            }
        })
        .detach();
    }

    fn handle(&mut self, event: Event, window: &mut Window, cx: &mut Context<Self>) {
        match event {
            Event::Opened { info, controls } => self.open(info, controls, window, cx),
            Event::NoCamera => self.screen = Screen::Message("No camera is connected.".into()),
            Event::Failed(message) => {
                self.screen = Screen::Message(format!("Cannot open the camera: {message}").into());
            }
            Event::DefaultsSaved => self.finish_save(true, cx),
            Event::DefaultsUnavailable => self.finish_save(false, cx),
            Event::DefaultsFailed(message) => {
                self.status = Some(format!("Saved, but Windows refused: {message}").into());
            }
            Event::WriteFailed(message) => self.status = Some(message.into()),
            Event::FrameReady => self.show_latest_frame(window),
            Event::PreviewUnavailable(message) => {
                self.preview_note = Some(format!("Preview is unavailable: {message}").into());
            }
        }
        cx.notify();
    }

    fn open(
        &mut self,
        info: webca_core::CameraInfo,
        controls: Vec<ControlState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let profiles = self.book.camera(&info.key).cloned().unwrap_or_else(CameraProfiles::default);
        let editor = Editor::new(info, controls, profiles);
        self.sliders.clear();
        self.slider_subscriptions.clear();
        let continuous: Vec<_> = [ControlGroup::Image, ControlGroup::Light, ControlGroup::Optics]
            .into_iter()
            .flat_map(|group| editor.group(group).copied().collect::<Vec<_>>())
            .filter(|state| !state.capability.id.is_choice())
            .collect();
        for state in continuous {
            let id = state.capability.id;
            let range = state.capability.range;
            let value = editor.setting(id).map_or(range.default(), |setting| setting.value);
            #[allow(clippy::cast_precision_loss)]
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(range.min() as f32)
                    .max(range.max() as f32)
                    .step(range.step() as f32)
                    .default_value(value as f32)
            });
            let subscription =
                cx.subscribe_in(&slider, window, move |this, _, event: &SliderEvent, _, cx| {
                    let SliderEvent::Change(value) = event else { return };
                    #[allow(clippy::cast_possible_truncation)]
                    let value = value.start().round() as i32;
                    this.edit(cx, |editor| editor.set_value(id, value).into_iter().collect());
                });
            self.sliders.insert(id, slider);
            self.slider_subscriptions.push(subscription);
        }
        self.screen = Screen::Ready(Box::new(editor));
        self.status = None;
    }

    fn show_latest_frame(&mut self, window: &mut Window) {
        let Some(frame) = self.frames.lock().ok().and_then(|mut latest| latest.take()) else { return };
        let Some(buffer) = image::RgbaImage::from_raw(frame.width, frame.height, frame.bgra) else { return };
        let next = Arc::new(RenderImage::new([image::Frame::new(buffer)]));
        if let Some(previous) = self.preview.replace(next) {
            let _ = window.drop_image(previous);
        }
    }

    fn edit(&mut self, cx: &mut Context<Self>, change: impl FnOnce(&mut Editor) -> Vec<LiveWrite>) {
        let Screen::Ready(editor) = &mut self.screen else { return };
        let writes = change(editor);
        if !writes.is_empty() {
            self.worker.send(Command::Write(writes));
            self.status = None;
        }
        cx.notify();
    }

    fn edit_and_sync(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Editor) -> Vec<LiveWrite>,
    ) {
        let Screen::Ready(editor) = &mut self.screen else { return };
        let writes = change(editor);
        for (id, setting) in &writes {
            if let Some(slider) = self.sliders.get(id) {
                #[allow(clippy::cast_precision_loss)]
                slider.update(cx, |slider, cx| slider.set_value(setting.value as f32, window, cx));
            }
        }
        if !writes.is_empty() {
            self.worker.send(Command::Write(writes));
        }
        cx.notify();
    }

    fn save(&mut self, cx: &mut Context<Self>) {
        let Screen::Ready(editor) = &mut self.screen else { return };
        let settings: Vec<_> = editor.save().settings().collect();
        self.book.set_camera(&editor.camera().key, editor.profiles().clone());
        match self.store.save(&self.book) {
            Ok(()) => {
                self.status = Some("Saving…".into());
                self.worker.send(Command::SaveDefaults(settings));
            }
            Err(error) => self.status = Some(error.to_string().into()),
        }
        cx.notify();
    }

    fn finish_save(&mut self, native: bool, cx: &mut Context<Self>) {
        let settings = self.book.agent();
        self.book.set_agent(AgentSettings { native, ..settings });
        let _ = self.store.save(&self.book);
        if webca_windows::current_package_family().is_none()
            && let Ok(executable) = std::env::current_exe()
        {
            let _ = webca_windows::enable_autostart(&executable);
        }
        Shell::notify(cx, ShellEvent::ProfilesChanged);
        self.status = Some(if native {
            "Saved. Windows applies it in every app.".into()
        } else {
            "Saved. WebCa keeps it applied from the tray.".into()
        });
    }

    fn enable_native_mode(&mut self, cx: &mut Context<Self>) {
        let Screen::Ready(editor) = &self.screen else { return };
        let Some(family) = webca_windows::current_package_family() else {
            self.status = Some("System-wide mode is available in the installed version of WebCa.".into());
            return cx.notify();
        };
        let Ok(executable) = std::env::current_exe() else { return };
        let arguments = format!("{} \"{}\" {family}", crate::ASSOCIATE_FLAG, editor.camera().key);
        self.status = Some("Waiting for permission…".into());
        cx.notify();
        let request =
            cx.background_spawn(async move { webca_windows::run_elevated(&executable, &arguments) });
        cx.spawn(async move |this, cx| {
            let result = request.await;
            let _ = this.update(cx, |studio, cx| match result {
                Ok(0) => studio.save(cx),
                Ok(_) => {
                    studio.status = Some("Windows did not accept WebCa as the camera companion.".into());
                    cx.notify();
                }
                Err(error) => {
                    studio.status = Some(error.to_string().into());
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn render_body(&self, cx: &mut Context<Self>) -> AnyElement {
        match &self.screen {
            Screen::Connecting => centered("Looking for your camera…", cx),
            Screen::Message(message) => centered(message.clone(), cx),
            Screen::Ready(editor) => h_flex()
                .size_full()
                .items_start()
                .child(self.render_stage(editor, cx))
                .child(self.render_panel(editor, cx))
                .into_any_element(),
        }
    }

    fn render_stage(&self, editor: &Editor, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let profiles: Vec<SharedString> =
            editor.profiles().profiles().map(|p| p.name().to_owned().into()).collect();
        let active = editor.active_profile().to_owned();
        v_flex()
            .flex_1()
            .min_w_0()
            .h_full()
            .gap_4()
            .px_6()
            .pt_2()
            .pb_6()
            .child(
                h_flex()
                    .justify_between()
                    .child(
                        h_flex()
                            .gap_2()
                            .px_3()
                            .py_1p5()
                            .rounded_lg()
                            .bg(theme.secondary)
                            .child(div().size_2().rounded_full().bg(theme.success))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::MEDIUM)
                                    .child(editor.camera().name.clone()),
                            ),
                    )
                    .child(h_flex().gap_1().children(profiles.into_iter().enumerate().map(
                        |(index, name)| {
                            let selected = name.as_ref() == active;
                            let target = name.clone();
                            let button = Button::new(("profile", index)).label(name).small();
                            let button = if selected { button.primary() } else { button.ghost() };
                            button.on_click(cx.listener(move |this, _, window, cx| {
                                let target = target.clone();
                                this.edit_and_sync(window, cx, |editor| {
                                    editor.select_profile(&target).unwrap_or_default()
                                });
                            }))
                        },
                    ))),
            )
            .child(
                div()
                    .w_full()
                    .flex_1()
                    .rounded_xl()
                    .bg(theme.muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .relative()
                    .min_h_0()
                    .overflow_hidden()
                    .map(|stage| match &self.preview {
                        Some(frame) => stage.child(
                            img(frame.clone())
                                .absolute()
                                .top_0()
                                .left_0()
                                .size_full()
                                .object_fit(ObjectFit::Cover),
                        ),
                        None => {
                            stage.child(div().text_sm().text_color(theme.muted_foreground).child(
                                self.preview_note.clone().unwrap_or_else(|| "Starting preview…".into()),
                            ))
                        }
                    }),
            )
            .child(self.render_status(editor, cx))
    }

    fn render_status(&self, editor: &Editor, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let dirty = editor.is_dirty();
        h_flex()
            .gap_2()
            .text_xs()
            .text_color(theme.muted_foreground)
            .child(div().size_2().rounded_full().bg(if dirty { theme.warning } else { theme.success }))
            .child(self.status.clone().unwrap_or_else(|| {
                if dirty { "Unsaved changes".into() } else { "Everything is saved".into() }
            }))
            .child(div().flex_1())
            .child(if self.book.agent().native {
                div().child("System-wide mode is on").into_any_element()
            } else {
                Button::new("native")
                    .label("Turn on system-wide mode")
                    .ghost()
                    .xsmall()
                    .on_click(cx.listener(|this, _, _, cx| this.enable_native_mode(cx)))
                    .into_any_element()
            })
    }

    fn render_panel(&self, editor: &Editor, cx: &mut Context<Self>) -> impl IntoElement {
        let dirty = editor.is_dirty();
        let sections: Vec<AnyElement> = [ControlGroup::Image, ControlGroup::Light, ControlGroup::Optics]
            .into_iter()
            .filter(|group| editor.has_group(*group))
            .map(|group| self.render_section(editor, group, cx))
            .collect();
        let theme = cx.theme();
        v_flex()
            .w(px(SIDE_PANEL_WIDTH))
            .h_full()
            .border_l_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(
                v_flex().id("controls").flex_1().overflow_y_scroll().px_6().py_5().gap_6().children(sections),
            )
            .child(
                h_flex()
                    .px_6()
                    .py_4()
                    .justify_between()
                    .border_t_1()
                    .border_color(theme.border)
                    .child(Button::new("reset").label("Reset").ghost().small().on_click(cx.listener(
                        |this, _, window, cx| {
                            this.edit_and_sync(window, cx, Editor::reset_to_factory);
                        },
                    )))
                    .child(
                        h_flex()
                            .gap_2()
                            .child(Button::new("revert").label("Revert").small().disabled(!dirty).on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.edit_and_sync(window, cx, Editor::revert);
                                }),
                            ))
                            .child(
                                Button::new("save")
                                    .label("Save")
                                    .primary()
                                    .small()
                                    .disabled(!dirty)
                                    .on_click(cx.listener(|this, _, _, cx| this.save(cx))),
                            ),
                    ),
            )
    }

    fn render_section(&self, editor: &Editor, group: ControlGroup, cx: &mut Context<Self>) -> AnyElement {
        let rows: Vec<AnyElement> =
            editor.group(group).map(|state| self.render_control(editor, state, cx)).collect();
        let theme = cx.theme();
        v_flex()
            .gap_4()
            .child(
                div()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.muted_foreground)
                    .child(labels::group(group)),
            )
            .children(rows)
            .into_any_element()
    }

    fn render_control(&self, editor: &Editor, state: &ControlState, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let id = state.capability.id;
        let setting = editor.setting(id).unwrap_or(state.current);
        let header = h_flex().justify_between().text_sm().child(labels::control(id)).child(
            h_flex()
                .gap_2()
                .when(state.capability.supports_auto && state.capability.supports_manual, |row| {
                    row.child(
                        Switch::new(("auto", id as usize))
                            .label("Auto")
                            .small()
                            .checked(setting.is_auto())
                            .on_click(cx.listener(move |this, checked: &bool, _, cx| {
                                let checked = *checked;
                                this.edit(cx, |editor| editor.set_auto(id, checked).into_iter().collect());
                            })),
                    )
                })
                .when(!id.is_choice(), |row| {
                    row.child(
                        div().text_color(theme.muted_foreground).child(labels::value(id, setting.value)),
                    )
                }),
        );
        let body: AnyElement = if id.is_choice() {
            let range = state.capability.range;
            h_flex()
                .gap_1()
                .children((range.min()..=range.max()).map(|value| {
                    let label = usize::try_from(value)
                        .ok()
                        .and_then(|index| labels::POWER_LINE_CHOICES.get(index).copied())
                        .map_or_else(|| value.to_string(), str::to_owned);
                    let button = Button::new(("choice", value.unsigned_abs() as usize)).label(label).small();
                    let button = if setting.value == value { button.primary() } else { button.ghost() };
                    button.on_click(cx.listener(move |this, _, _, cx| {
                        this.edit(cx, |editor| editor.set_value(id, value).into_iter().collect());
                    }))
                }))
                .into_any_element()
        } else if let Some(slider) = self.sliders.get(&id) {
            Slider::new(slider).disabled(setting.is_auto()).into_any_element()
        } else {
            div().into_any_element()
        };
        v_flex().gap_2().child(header).child(body).into_any_element()
    }
}

fn centered(text: impl Into<SharedString>, cx: &App) -> AnyElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .text_color(cx.theme().muted_foreground)
        .child(text.into())
        .into_any_element()
}

impl Render for Studio {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        v_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                TitleBar::new().child(
                    h_flex()
                        .gap_2()
                        .child(div().size_4().rounded_md().bg(theme.primary))
                        .child(div().text_xs().font_weight(FontWeight::MEDIUM).child("WebCa")),
                ),
            )
            .child(div().flex_1().min_h_0().child(self.render_body(cx)))
    }
}
