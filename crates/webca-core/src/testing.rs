//! An in-memory camera for tests of code that talks to [`CameraDevice`].

use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};

use crate::{
    CameraDevice, CameraInfo, CameraKey, ControlCapability, ControlId, ControlRange, ControlSetting,
    DeviceError,
};

/// A camera that keeps control values in memory and can be told to fail.
#[derive(Debug)]
pub struct FakeCamera {
    info: CameraInfo,
    capabilities: Vec<ControlCapability>,
    values: RefCell<BTreeMap<ControlId, ControlSetting>>,
    read_failures: RefCell<BTreeMap<ControlId, DeviceError>>,
    write_failures: RefCell<BTreeMap<ControlId, DeviceError>>,
    connected: Cell<bool>,
    log: RefCell<Vec<(ControlId, ControlSetting)>>,
}

impl FakeCamera {
    #[must_use]
    pub fn new(capabilities: Vec<ControlCapability>) -> Self {
        let values = capabilities
            .iter()
            .map(|capability| (capability.id, ControlSetting::manual(capability.range.default())))
            .collect();
        Self {
            info: CameraInfo { key: CameraKey::new("fake-camera"), name: "Fake Camera".to_owned() },
            capabilities,
            values: RefCell::new(values),
            read_failures: RefCell::default(),
            write_failures: RefCell::default(),
            connected: Cell::new(true),
            log: RefCell::default(),
        }
    }

    pub fn disconnect(&self) {
        self.connected.set(false);
    }

    pub fn fail_reads(&self, id: ControlId, error: DeviceError) {
        self.read_failures.borrow_mut().insert(id, error);
    }

    pub fn fail_writes(&self, id: ControlId, error: DeviceError) {
        self.write_failures.borrow_mut().insert(id, error);
    }

    /// Every successful write, in order.
    #[must_use]
    pub fn writes(&self) -> Vec<(ControlId, ControlSetting)> {
        self.log.borrow().clone()
    }

    fn ensure_connected(&self) -> Result<(), DeviceError> {
        if self.connected.get() { Ok(()) } else { Err(DeviceError::Disconnected) }
    }
}

impl CameraDevice for FakeCamera {
    fn info(&self) -> &CameraInfo {
        &self.info
    }

    fn capabilities(&self) -> Result<Vec<ControlCapability>, DeviceError> {
        self.ensure_connected()?;
        Ok(self.capabilities.clone())
    }

    fn read(&self, id: ControlId) -> Result<ControlSetting, DeviceError> {
        self.ensure_connected()?;
        if let Some(error) = self.read_failures.borrow().get(&id) {
            return Err(error.clone());
        }
        self.values.borrow().get(&id).copied().ok_or(DeviceError::Unsupported(id))
    }

    fn write(&self, id: ControlId, setting: ControlSetting) -> Result<(), DeviceError> {
        self.ensure_connected()?;
        if let Some(error) = self.write_failures.borrow().get(&id) {
            return Err(error.clone());
        }
        let capability = self
            .capabilities
            .iter()
            .find(|capability| capability.id == id && capability.accepts(setting))
            .ok_or(DeviceError::Unsupported(id))?;
        let fitted = setting.fitted_to(&capability.range);
        self.values.borrow_mut().insert(id, fitted);
        self.log.borrow_mut().push((id, fitted));
        Ok(())
    }
}

/// Capabilities resembling a typical 1080p UVC webcam.
#[must_use]
pub fn sample_capabilities() -> Vec<ControlCapability> {
    let range =
        |min, max, step, default| ControlRange::new(min, max, step, default).expect("valid sample range");
    vec![
        ControlCapability::manual(ControlId::Brightness, range(0, 255, 1, 128)),
        ControlCapability::manual(ControlId::Contrast, range(0, 255, 1, 128)),
        ControlCapability::manual(ControlId::Saturation, range(0, 255, 1, 128)),
        ControlCapability::manual(ControlId::Hue, range(-180, 180, 1, 0)),
        ControlCapability::manual(ControlId::Gamma, range(1, 10, 1, 5)),
        ControlCapability::manual(ControlId::WhiteBalance, range(2800, 6500, 10, 4600)).with_auto(),
        ControlCapability::manual(ControlId::PowerLineFrequency, range(0, 2, 1, 1)),
        ControlCapability::manual(ControlId::Exposure, range(-13, -1, 1, -6)).with_auto(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_at_driver_defaults() {
        let camera = FakeCamera::new(sample_capabilities());
        assert_eq!(camera.info().name, "Fake Camera");
        assert_eq!(camera.read(ControlId::Exposure), Ok(ControlSetting::manual(-6)));
        assert_eq!(camera.capabilities().unwrap().len(), 8);
    }

    #[test]
    fn writes_are_fitted_and_logged() {
        let camera = FakeCamera::new(sample_capabilities());
        camera.write(ControlId::WhiteBalance, ControlSetting::auto(4603)).unwrap();
        assert_eq!(camera.read(ControlId::WhiteBalance), Ok(ControlSetting::auto(4600)));
        assert_eq!(camera.writes(), [(ControlId::WhiteBalance, ControlSetting::auto(4600))]);
    }

    #[test]
    fn rejects_unknown_controls_and_unavailable_modes() {
        let camera = FakeCamera::new(sample_capabilities());
        assert_eq!(camera.read(ControlId::Focus), Err(DeviceError::Unsupported(ControlId::Focus)));
        assert_eq!(
            camera.write(ControlId::Focus, ControlSetting::manual(1)),
            Err(DeviceError::Unsupported(ControlId::Focus))
        );
        assert_eq!(
            camera.write(ControlId::Gamma, ControlSetting::auto(1)),
            Err(DeviceError::Unsupported(ControlId::Gamma))
        );
    }

    #[test]
    fn injected_failures_and_disconnects_surface() {
        let camera = FakeCamera::new(sample_capabilities());
        camera.fail_writes(ControlId::Gamma, DeviceError::Busy);
        camera.fail_reads(ControlId::Gamma, DeviceError::Busy);
        assert_eq!(camera.write(ControlId::Gamma, ControlSetting::manual(2)), Err(DeviceError::Busy));
        assert_eq!(camera.read(ControlId::Gamma), Err(DeviceError::Busy));
        camera.disconnect();
        assert_eq!(camera.capabilities(), Err(DeviceError::Disconnected));
        assert_eq!(camera.read(ControlId::Hue), Err(DeviceError::Disconnected));
        assert_eq!(camera.write(ControlId::Hue, ControlSetting::manual(1)), Err(DeviceError::Disconnected));
    }
}
