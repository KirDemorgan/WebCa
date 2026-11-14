use crate::{ControlId, ControlSetting, LiveWrite, Profile};

/// How the background agent keeps the camera on the saved profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GuardConfig {
    /// Windows applies the profile itself through the camera defaults store.
    pub native: bool,
    /// Undo changes other apps make to controls while the camera streams.
    pub lock: bool,
}

/// Something the agent observed about the camera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Signal {
    StreamStarted,
    StreamStopped,
    DeviceArrived,
    ControlChanged { id: ControlId, setting: ControlSetting },
    ProfileChanged(Profile),
    ConfigChanged(GuardConfig),
}

/// What the agent should do in response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    ApplyProfile,
    Restore(Vec<LiveWrite>),
}

/// Decides when the agent touches the camera, so it never fights the user or itself.
#[derive(Debug, Clone)]
pub struct Guard {
    config: GuardConfig,
    profile: Profile,
    streaming: bool,
}

impl Guard {
    #[must_use]
    pub const fn new(config: GuardConfig, profile: Profile) -> Self {
        Self { config, profile, streaming: false }
    }

    #[must_use]
    pub const fn config(&self) -> GuardConfig {
        self.config
    }

    #[must_use]
    pub const fn profile(&self) -> &Profile {
        &self.profile
    }

    #[must_use]
    pub const fn is_streaming(&self) -> bool {
        self.streaming
    }

    pub fn observe(&mut self, signal: Signal) -> Option<Action> {
        match signal {
            Signal::StreamStarted => {
                self.streaming = true;
                self.enforces_on_start().then_some(Action::ApplyProfile)
            }
            Signal::StreamStopped => {
                self.streaming = false;
                None
            }
            Signal::DeviceArrived => (!self.config.native).then_some(Action::ApplyProfile),
            Signal::ControlChanged { id, setting } => self.restore(id, setting),
            Signal::ProfileChanged(profile) => {
                self.profile = profile;
                self.streaming.then_some(Action::ApplyProfile)
            }
            Signal::ConfigChanged(config) => {
                let newly_enforced = !self.enforces_on_start();
                self.config = config;
                (self.streaming && newly_enforced && self.enforces_on_start()).then_some(Action::ApplyProfile)
            }
        }
    }

    const fn enforces_on_start(&self) -> bool {
        !self.config.native || self.config.lock
    }

    fn restore(&self, id: ControlId, setting: ControlSetting) -> Option<Action> {
        if !self.config.lock || !self.streaming {
            return None;
        }
        let stored = self.profile.get(id)?;
        (stored != setting).then(|| Action::Restore(vec![(id, stored)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NATIVE: GuardConfig = GuardConfig { native: true, lock: false };
    const FALLBACK: GuardConfig = GuardConfig { native: false, lock: false };
    const LOCKED: GuardConfig = GuardConfig { native: true, lock: true };

    fn profile(contrast: i32) -> Profile {
        let mut profile = Profile::new("Day").unwrap();
        profile.set(ControlId::Contrast, ControlSetting::manual(contrast));
        profile
    }

    fn changed(value: i32) -> Signal {
        Signal::ControlChanged { id: ControlId::Contrast, setting: ControlSetting::manual(value) }
    }

    #[test]
    fn native_mode_leaves_stream_start_to_windows() {
        let mut guard = Guard::new(NATIVE, profile(140));
        assert_eq!(guard.observe(Signal::StreamStarted), None);
        assert!(guard.is_streaming());
        assert_eq!(guard.observe(Signal::DeviceArrived), None);
        assert_eq!(guard.observe(changed(10)), None);
    }

    #[test]
    fn fallback_mode_applies_on_start_and_arrival() {
        let mut guard = Guard::new(FALLBACK, profile(140));
        assert_eq!(guard.observe(Signal::DeviceArrived), Some(Action::ApplyProfile));
        assert_eq!(guard.observe(Signal::StreamStarted), Some(Action::ApplyProfile));
        assert_eq!(guard.observe(Signal::StreamStopped), None);
        assert!(!guard.is_streaming());
    }

    #[test]
    fn lock_restores_foreign_changes_only_while_streaming() {
        let mut guard = Guard::new(LOCKED, profile(140));
        assert_eq!(guard.observe(changed(10)), None);
        assert_eq!(guard.observe(Signal::StreamStarted), Some(Action::ApplyProfile));
        assert_eq!(
            guard.observe(changed(10)),
            Some(Action::Restore(vec![(ControlId::Contrast, ControlSetting::manual(140))]))
        );
        assert_eq!(guard.observe(changed(140)), None);
        let foreign = Signal::ControlChanged { id: ControlId::Zoom, setting: ControlSetting::manual(3) };
        assert_eq!(guard.observe(foreign), None);
    }

    #[test]
    fn profile_changes_apply_immediately_while_streaming() {
        let mut guard = Guard::new(NATIVE, profile(140));
        assert_eq!(guard.observe(Signal::ProfileChanged(profile(90))), None);
        assert_eq!(guard.profile().get(ControlId::Contrast), Some(ControlSetting::manual(90)));
        guard.observe(Signal::StreamStarted);
        assert_eq!(guard.observe(Signal::ProfileChanged(profile(60))), Some(Action::ApplyProfile));
    }

    #[test]
    fn enabling_enforcement_mid_stream_applies_once() {
        let mut guard = Guard::new(NATIVE, profile(140));
        guard.observe(Signal::StreamStarted);
        assert_eq!(guard.observe(Signal::ConfigChanged(LOCKED)), Some(Action::ApplyProfile));
        assert_eq!(guard.config(), LOCKED);
        assert_eq!(guard.observe(Signal::ConfigChanged(FALLBACK)), None);
        assert_eq!(guard.observe(Signal::ConfigChanged(NATIVE)), None);
        guard.observe(Signal::StreamStopped);
        assert_eq!(guard.observe(Signal::ConfigChanged(LOCKED)), None);
    }
}
