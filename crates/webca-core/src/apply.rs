use crate::{CameraDevice, ControlCapability, ControlId, ControlSetting, DeviceError, Profile};

/// Why a stored setting was not sent to the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkipReason {
    Unsupported,
    ModeUnavailable,
}

/// The writes needed to bring a camera to a profile.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyPlan {
    pub writes: Vec<(ControlId, ControlSetting)>,
    pub skipped: Vec<(ControlId, SkipReason)>,
}

impl ApplyPlan {
    /// Plans writes for every setting of `profile` the camera can take, fitted to its range.
    #[must_use]
    pub fn new(profile: &Profile, capabilities: &[ControlCapability]) -> Self {
        let mut plan = Self::default();
        for (id, setting) in profile.settings() {
            match capabilities.iter().find(|capability| capability.id == id) {
                None => plan.skipped.push((id, SkipReason::Unsupported)),
                Some(capability) if !capability.accepts(setting) => {
                    plan.skipped.push((id, SkipReason::ModeUnavailable));
                }
                Some(capability) => plan.writes.push((id, setting.fitted_to(&capability.range))),
            }
        }
        plan
    }
}

/// The outcome of applying a profile to a camera.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    pub applied: Vec<ControlId>,
    pub skipped: Vec<(ControlId, SkipReason)>,
    pub failed: Vec<(ControlId, DeviceError)>,
}

impl ApplyReport {
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.skipped.is_empty() && self.failed.is_empty()
    }
}

/// Writes `profile` to `device`, continuing past individual control failures.
pub fn apply(device: &dyn CameraDevice, profile: &Profile) -> Result<ApplyReport, DeviceError> {
    let plan = ApplyPlan::new(profile, &device.capabilities()?);
    let mut report = ApplyReport { skipped: plan.skipped, ..ApplyReport::default() };
    for (id, setting) in plan.writes {
        match device.write(id, setting) {
            Ok(()) => report.applied.push(id),
            Err(DeviceError::Disconnected) => return Err(DeviceError::Disconnected),
            Err(error) => report.failed.push((id, error)),
        }
    }
    Ok(report)
}

/// A control with its capability and the value the camera currently uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ControlState {
    pub capability: ControlCapability,
    pub current: ControlSetting,
}

/// Reads every supported control, in the order of [`ControlId::ALL`].
pub fn snapshot(device: &dyn CameraDevice) -> Result<Vec<ControlState>, DeviceError> {
    let mut capabilities = device.capabilities()?;
    capabilities.sort_by_key(|capability| capability.id);
    capabilities
        .into_iter()
        .map(|capability| Ok(ControlState { capability, current: device.read(capability.id)? }))
        .collect()
}

/// A profile holding exactly the values the camera currently uses.
pub fn capture(states: &[ControlState], name: &str) -> Result<Profile, crate::ProfileError> {
    let mut profile = Profile::new(name)?;
    for state in states {
        profile.set(state.capability.id, state.current);
    }
    Ok(profile)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ControlRange, ProfileError,
        testing::{FakeCamera, sample_capabilities},
    };

    fn profile(settings: &[(ControlId, ControlSetting)]) -> Profile {
        let mut profile = Profile::new("Test").unwrap();
        for (id, setting) in settings {
            profile.set(*id, *setting);
        }
        profile
    }

    #[test]
    fn plan_fits_values_and_skips_what_the_camera_cannot_do() {
        let profile = profile(&[
            (ControlId::Brightness, ControlSetting::manual(400)),
            (ControlId::Hue, ControlSetting::auto(0)),
            (ControlId::Focus, ControlSetting::manual(10)),
            (ControlId::WhiteBalance, ControlSetting::auto(4600)),
        ]);
        let plan = ApplyPlan::new(&profile, &sample_capabilities());
        assert_eq!(
            plan.writes,
            [
                (ControlId::Brightness, ControlSetting::manual(255)),
                (ControlId::WhiteBalance, ControlSetting::auto(4600)),
            ]
        );
        assert_eq!(
            plan.skipped,
            [(ControlId::Hue, SkipReason::ModeUnavailable), (ControlId::Focus, SkipReason::Unsupported)]
        );
    }

    #[test]
    fn apply_writes_planned_settings() {
        let camera = FakeCamera::new(sample_capabilities());
        let profile = profile(&[
            (ControlId::Contrast, ControlSetting::manual(140)),
            (ControlId::Zoom, ControlSetting::manual(2)),
        ]);
        let report = apply(&camera, &profile).unwrap();
        assert_eq!(report.applied, [ControlId::Contrast]);
        assert_eq!(report.skipped, [(ControlId::Zoom, SkipReason::Unsupported)]);
        assert!(!report.is_complete());
        assert_eq!(camera.read(ControlId::Contrast), Ok(ControlSetting::manual(140)));
    }

    #[test]
    fn apply_collects_failures_and_continues() {
        let camera = FakeCamera::new(sample_capabilities());
        camera.fail_writes(ControlId::Contrast, DeviceError::Busy);
        let profile = profile(&[
            (ControlId::Brightness, ControlSetting::manual(10)),
            (ControlId::Contrast, ControlSetting::manual(10)),
        ]);
        let report = apply(&camera, &profile).unwrap();
        assert_eq!(report.applied, [ControlId::Brightness]);
        assert_eq!(report.failed, [(ControlId::Contrast, DeviceError::Busy)]);
    }

    #[test]
    fn apply_stops_when_the_camera_disappears() {
        let camera = FakeCamera::new(sample_capabilities());
        camera.fail_writes(ControlId::Brightness, DeviceError::Disconnected);
        let profile = profile(&[
            (ControlId::Brightness, ControlSetting::manual(10)),
            (ControlId::Contrast, ControlSetting::manual(10)),
        ]);
        assert_eq!(apply(&camera, &profile), Err(DeviceError::Disconnected));
        assert_eq!(camera.writes(), Vec::new());
    }

    #[test]
    fn apply_fails_when_capabilities_are_unavailable() {
        let camera = FakeCamera::new(sample_capabilities());
        camera.disconnect();
        assert_eq!(apply(&camera, &profile(&[])), Err(DeviceError::Disconnected));
    }

    #[test]
    fn complete_report_has_nothing_skipped_or_failed() {
        let camera = FakeCamera::new(sample_capabilities());
        let report = apply(&camera, &profile(&[(ControlId::Gamma, ControlSetting::manual(5))])).unwrap();
        assert!(report.is_complete());
    }

    #[test]
    fn snapshot_reads_every_control_in_order() {
        let mut capabilities = sample_capabilities();
        capabilities.reverse();
        let camera = FakeCamera::new(capabilities);
        let states = snapshot(&camera).unwrap();
        let ids: Vec<_> = states.iter().map(|state| state.capability.id).collect();
        let mut expected = ids.clone();
        expected.sort();
        assert_eq!(ids, expected);
        let brightness = states.iter().find(|s| s.capability.id == ControlId::Brightness).unwrap();
        assert_eq!(brightness.current, ControlSetting::manual(128));
    }

    #[test]
    fn snapshot_propagates_read_errors() {
        let camera = FakeCamera::new(sample_capabilities());
        camera.fail_reads(ControlId::Gamma, DeviceError::Busy);
        assert_eq!(snapshot(&camera), Err(DeviceError::Busy));
        camera.disconnect();
        assert_eq!(snapshot(&camera), Err(DeviceError::Disconnected));
    }

    #[test]
    fn capture_turns_current_values_into_a_profile() {
        let camera = FakeCamera::new(sample_capabilities());
        let states = snapshot(&camera).unwrap();
        let profile = capture(&states, "Now").unwrap();
        assert_eq!(profile.name(), "Now");
        assert_eq!(profile.settings().count(), states.len());
        assert_eq!(capture(&states, ""), Err(ProfileError::EmptyName));
    }

    #[test]
    fn capabilities_outside_the_profile_are_left_alone() {
        let range = ControlRange::new(0, 1, 1, 0).unwrap();
        let plan = ApplyPlan::new(&profile(&[]), &[ControlCapability::manual(ControlId::Pan, range)]);
        assert_eq!(plan, ApplyPlan::default());
    }
}
