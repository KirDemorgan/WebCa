use crate::{
    CameraInfo, CameraProfiles, ControlGroup, ControlId, ControlSetting, ControlState, Profile, ProfileError,
};

/// A control value to send to the camera right away, so the preview follows the user's edits.
pub type LiveWrite = (ControlId, ControlSetting);

/// State of the settings screen for one camera: what the user is editing and what is saved.
#[derive(Debug, Clone)]
pub struct Editor {
    camera: CameraInfo,
    controls: Vec<ControlState>,
    working: Vec<ControlSetting>,
    profiles: CameraProfiles,
}

impl Editor {
    /// Starts editing the active profile; controls the profile does not mention keep the camera's
    /// current values.
    #[must_use]
    pub fn new(camera: CameraInfo, mut controls: Vec<ControlState>, profiles: CameraProfiles) -> Self {
        controls.sort_by_key(|state| state.capability.id);
        let working = controls.iter().map(|state| state.current).collect();
        let mut editor = Self { camera, controls, working, profiles };
        editor.load_active();
        editor
    }

    #[must_use]
    pub fn camera(&self) -> &CameraInfo {
        &self.camera
    }

    #[must_use]
    pub fn profiles(&self) -> &CameraProfiles {
        &self.profiles
    }

    #[must_use]
    pub fn active_profile(&self) -> &str {
        self.profiles.active().name()
    }

    /// Controls of `group` in display order.
    pub fn group(&self, group: ControlGroup) -> impl Iterator<Item = &ControlState> {
        self.controls.iter().filter(move |state| state.capability.id.group() == group)
    }

    #[must_use]
    pub fn has_group(&self, group: ControlGroup) -> bool {
        self.group(group).next().is_some()
    }

    #[must_use]
    pub fn setting(&self, id: ControlId) -> Option<ControlSetting> {
        self.index(id).map(|index| self.working[index])
    }

    /// Sets a manual value; returns the write for the camera when something changed.
    pub fn set_value(&mut self, id: ControlId, value: i32) -> Option<LiveWrite> {
        let index = self.index(id)?;
        let capability = self.controls[index].capability;
        if !capability.supports_manual {
            return None;
        }
        self.update(index, ControlSetting::manual(capability.range.snap(value)))
    }

    /// Switches between automatic and manual mode, keeping the last value.
    pub fn set_auto(&mut self, id: ControlId, auto: bool) -> Option<LiveWrite> {
        let index = self.index(id)?;
        let capability = self.controls[index].capability;
        let value = self.working[index].value;
        let setting = if auto { ControlSetting::auto(value) } else { ControlSetting::manual(value) };
        if !capability.accepts(setting) {
            return None;
        }
        self.update(index, setting)
    }

    /// Whether the edited values differ from the saved active profile.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        let saved = self.profiles.active();
        self.controls
            .iter()
            .zip(&self.working)
            .any(|(state, working)| saved.get(state.capability.id).is_none_or(|stored| stored != *working))
    }

    /// Discards edits and returns to the saved active profile.
    pub fn revert(&mut self) -> Vec<LiveWrite> {
        self.transition(Self::load_active)
    }

    /// Puts every control back to the driver's factory value in manual mode.
    pub fn reset_to_factory(&mut self) -> Vec<LiveWrite> {
        self.transition(|editor| {
            for (state, working) in editor.controls.iter().zip(editor.working.iter_mut()) {
                let factory = ControlSetting::manual(state.capability.range.default());
                if state.capability.accepts(factory) {
                    *working = factory;
                }
            }
        })
    }

    /// Switches to another profile, discarding unsaved edits.
    pub fn select_profile(&mut self, name: &str) -> Result<Vec<LiveWrite>, ProfileError> {
        self.profiles.select(name)?;
        Ok(self.transition(Self::load_active))
    }

    /// Creates a profile from the current edits and makes it active.
    pub fn create_profile(&mut self, name: &str) -> Result<(), ProfileError> {
        let working = self.snapshot_profile();
        let created = self.profiles.add(name)?.name().to_owned();
        self.profiles.select(&created).expect("a just-created profile can be selected");
        self.store_into_active(&working);
        Ok(())
    }

    /// Removes a profile; when it was active, the editor moves to the new active profile.
    pub fn delete_profile(&mut self, name: &str) -> Result<Vec<LiveWrite>, ProfileError> {
        let was_active = self.profiles.active().name().eq_ignore_ascii_case(name.trim());
        self.profiles.remove(name)?;
        Ok(if was_active { self.transition(Self::load_active) } else { Vec::new() })
    }

    pub fn rename_profile(&mut self, from: &str, to: &str) -> Result<(), ProfileError> {
        self.profiles.rename(from, to)
    }

    /// Stores the edited values in the active profile and returns it for persisting.
    pub fn save(&mut self) -> &Profile {
        let working = self.snapshot_profile();
        self.store_into_active(&working);
        self.profiles.active()
    }

    fn snapshot_profile(&self) -> Vec<(ControlId, ControlSetting)> {
        self.controls
            .iter()
            .zip(&self.working)
            .map(|(state, working)| (state.capability.id, *working))
            .collect()
    }

    fn store_into_active(&mut self, settings: &[(ControlId, ControlSetting)]) {
        let active = self.profiles.active_mut();
        for (id, setting) in settings {
            active.set(*id, *setting);
        }
    }

    fn load_active(&mut self) {
        let active = self.profiles.active().clone();
        for (state, working) in self.controls.iter().zip(self.working.iter_mut()) {
            if let Some(stored) = active.get(state.capability.id)
                && state.capability.accepts(stored)
            {
                *working = stored.fitted_to(&state.capability.range);
            }
        }
    }

    fn transition(&mut self, change: impl FnOnce(&mut Self)) -> Vec<LiveWrite> {
        let before = self.working.clone();
        change(self);
        self.controls
            .iter()
            .zip(before.iter().zip(&self.working))
            .filter(|(_, (old, new))| old != new)
            .map(|(state, (_, new))| (state.capability.id, *new))
            .collect()
    }

    fn update(&mut self, index: usize, setting: ControlSetting) -> Option<LiveWrite> {
        if self.working[index] == setting {
            return None;
        }
        self.working[index] = setting;
        Some((self.controls[index].capability.id, setting))
    }

    fn index(&self, id: ControlId) -> Option<usize> {
        self.controls.iter().position(|state| state.capability.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CameraDevice, snapshot,
        testing::{FakeCamera, sample_capabilities},
    };

    fn editor_with(profiles: CameraProfiles) -> Editor {
        let camera = FakeCamera::new(sample_capabilities());
        Editor::new(camera.info().clone(), snapshot(&camera).unwrap(), profiles)
    }

    fn editor() -> Editor {
        editor_with(CameraProfiles::default())
    }

    #[test]
    fn starts_from_camera_values_when_profile_is_empty() {
        let editor = editor();
        assert_eq!(editor.camera().name, "Fake Camera");
        assert_eq!(editor.active_profile(), "Default");
        assert_eq!(editor.setting(ControlId::Brightness), Some(ControlSetting::manual(128)));
        assert_eq!(editor.setting(ControlId::Focus), None);
        assert!(editor.is_dirty());
    }

    #[test]
    fn starts_from_profile_values_fitted_to_the_camera() {
        let mut profiles = CameraProfiles::default();
        profiles.active_mut().set(ControlId::Brightness, ControlSetting::manual(999));
        profiles.active_mut().set(ControlId::Gamma, ControlSetting::auto(3));
        profiles.active_mut().set(ControlId::Focus, ControlSetting::manual(1));
        let editor = editor_with(profiles);
        assert_eq!(editor.setting(ControlId::Brightness), Some(ControlSetting::manual(255)));
        assert_eq!(editor.setting(ControlId::Gamma), Some(ControlSetting::manual(5)));
    }

    #[test]
    fn groups_list_controls_in_order() {
        let editor = editor();
        let image: Vec<_> = editor.group(ControlGroup::Image).map(|s| s.capability.id).collect();
        assert_eq!(
            image,
            [
                ControlId::Brightness,
                ControlId::Contrast,
                ControlId::Hue,
                ControlId::Saturation,
                ControlId::Gamma
            ]
        );
        assert!(editor.has_group(ControlGroup::Light));
        assert!(!editor.has_group(ControlGroup::Optics));
    }

    #[test]
    fn set_value_snaps_and_reports_changes_once() {
        let mut editor = editor();
        assert_eq!(
            editor.set_value(ControlId::WhiteBalance, 4703),
            Some((ControlId::WhiteBalance, ControlSetting::manual(4700)))
        );
        assert_eq!(editor.set_value(ControlId::WhiteBalance, 4700), None);
        assert_eq!(editor.set_value(ControlId::Focus, 1), None);
    }

    #[test]
    fn set_value_is_ignored_for_auto_only_controls() {
        let camera = FakeCamera::new(vec![crate::ControlCapability {
            supports_manual: false,
            ..crate::ControlCapability::manual(
                ControlId::Exposure,
                crate::ControlRange::new(-13, -1, 1, -6).unwrap(),
            )
            .with_auto()
        }]);
        let mut editor =
            Editor::new(camera.info().clone(), snapshot(&camera).unwrap(), CameraProfiles::default());
        assert_eq!(editor.set_value(ControlId::Exposure, -3), None);
    }

    #[test]
    fn set_auto_toggles_supported_controls_only() {
        let mut editor = editor();
        assert_eq!(
            editor.set_auto(ControlId::Exposure, true),
            Some((ControlId::Exposure, ControlSetting::auto(-6)))
        );
        assert_eq!(editor.set_auto(ControlId::Exposure, true), None);
        assert_eq!(
            editor.set_auto(ControlId::Exposure, false),
            Some((ControlId::Exposure, ControlSetting::manual(-6)))
        );
        assert_eq!(editor.set_auto(ControlId::Contrast, true), None);
        assert_eq!(editor.set_auto(ControlId::Focus, true), None);
    }

    #[test]
    fn save_clears_dirty_state_and_revert_restores_it() {
        let mut editor = editor();
        let saved = editor.save().clone();
        assert_eq!(saved.get(ControlId::Contrast), Some(ControlSetting::manual(128)));
        assert!(!editor.is_dirty());

        editor.set_value(ControlId::Contrast, 140);
        assert!(editor.is_dirty());
        assert_eq!(editor.revert(), [(ControlId::Contrast, ControlSetting::manual(128))]);
        assert!(!editor.is_dirty());
        assert_eq!(editor.revert(), []);
    }

    #[test]
    fn reset_to_factory_returns_changed_controls() {
        let mut editor = editor();
        editor.set_value(ControlId::Saturation, 10);
        editor.set_auto(ControlId::Exposure, true);
        let writes = editor.reset_to_factory();
        assert_eq!(
            writes,
            [
                (ControlId::Saturation, ControlSetting::manual(128)),
                (ControlId::Exposure, ControlSetting::manual(-6)),
            ]
        );
    }

    #[test]
    fn reset_to_factory_skips_controls_without_manual_mode() {
        let auto_only = crate::ControlCapability {
            supports_manual: false,
            ..crate::ControlCapability::manual(
                ControlId::Focus,
                crate::ControlRange::new(0, 10, 1, 5).unwrap(),
            )
            .with_auto()
        };
        let state = ControlState { capability: auto_only, current: ControlSetting::auto(7) };
        let mut editor =
            Editor::new(FakeCamera::new(vec![]).info().clone(), vec![state], CameraProfiles::default());
        assert_eq!(editor.reset_to_factory(), []);
    }

    #[test]
    fn profiles_can_be_created_selected_renamed_and_deleted() {
        let mut editor = editor();
        editor.save();
        editor.set_value(ControlId::Gamma, 9);
        editor.create_profile("Evening").unwrap();
        assert_eq!(editor.active_profile(), "Evening");
        assert!(!editor.is_dirty());

        let writes = editor.select_profile("Default").unwrap();
        assert_eq!(writes, [(ControlId::Gamma, ControlSetting::manual(5))]);
        assert_eq!(editor.select_profile("Ghost"), Err(ProfileError::NotFound("Ghost".into())));

        editor.rename_profile("Evening", "Night").unwrap();
        assert_eq!(editor.profiles().profiles().count(), 2);
        assert_eq!(editor.delete_profile("Night").unwrap(), []);

        editor.create_profile("Late").unwrap();
        editor.set_value(ControlId::Gamma, 2);
        editor.save();
        let writes = editor.delete_profile(" late ").unwrap();
        assert_eq!(editor.active_profile(), "Default");
        assert_eq!(writes, [(ControlId::Gamma, ControlSetting::manual(5))]);
        assert_eq!(editor.delete_profile("Default"), Err(ProfileError::LastProfile));
        assert_eq!(editor.create_profile("default"), Err(ProfileError::DuplicateName("default".into())));
    }
}
