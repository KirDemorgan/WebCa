//! Tests against the first physical camera. Run with `cargo test -p webca-windows -- --ignored`.
#![cfg(windows)]

use webca_core::{CameraDevice, ControlId, ControlSetting, snapshot};
use webca_windows::{MediaSession, WindowsCamera, list_cameras};

fn first_camera() -> WindowsCamera {
    let cameras = list_cameras().expect("cameras can be listed");
    let entry = cameras.first().expect("a camera is connected");
    WindowsCamera::open(entry).expect("the camera opens")
}

#[test]
#[ignore = "requires a connected camera"]
fn lists_cameras_with_stable_keys() {
    let _session = MediaSession::start().unwrap();
    let cameras = list_cameras().unwrap();
    assert!(!cameras.is_empty());
    for camera in &cameras {
        assert!(!camera.info.name.is_empty());
        assert_eq!(camera.info.key.as_str(), camera.device_path.to_lowercase());
    }
}

#[test]
#[ignore = "requires a connected camera"]
fn reports_capabilities_and_current_values() {
    let _session = MediaSession::start().unwrap();
    let camera = first_camera();
    let states = snapshot(&camera).unwrap();
    assert!(!states.is_empty());
    for state in &states {
        let range = state.capability.range;
        assert!(range.min() <= range.max(), "{}", state.capability.id);
        println!(
            "{:<24} {:>6}..{:<6} step {:<4} default {:<6} current {:?}",
            state.capability.id,
            range.min(),
            range.max(),
            range.step(),
            range.default(),
            state.current
        );
    }
}

#[test]
#[ignore = "requires a connected camera"]
fn writes_and_restores_controls_on_both_interfaces() {
    let _session = MediaSession::start().unwrap();
    let camera = first_camera();
    assert!(!camera.info().name.is_empty());
    let capabilities = camera.capabilities().unwrap();
    for id in [ControlId::Saturation, ControlId::Zoom] {
        let Some(capability) = capabilities.iter().find(|c| c.id == id && c.supports_manual) else {
            continue;
        };
        let original = camera.read(id).unwrap();
        let direction = if original.value > capability.range.min() { -1 } else { 1 };
        let target = capability.range.nudge(original.value, direction);

        camera.write(id, ControlSetting::manual(target)).unwrap();
        let written = camera.read(id);
        camera.write(id, original).unwrap();

        assert_eq!(written.unwrap().value, target, "{id}");
        assert_eq!(camera.read(id).unwrap(), original, "{id}");
    }
}
