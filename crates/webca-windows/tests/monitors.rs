//! Tests of the camera activity and control monitors. Run with `cargo test -p webca-windows -- --ignored`.
#![cfg(windows)]

use std::{sync::mpsc, time::Duration};

use webca_core::{CameraDevice, ControlId, ControlSetting};
use webca_windows::{
    ActivityMonitor, ControlMonitor, MediaSession, PreviewReader, WindowsCamera, list_cameras,
};

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
#[ignore = "requires a connected camera"]
fn activity_monitor_sees_this_process_streaming() {
    let _session = MediaSession::start().unwrap();
    let entry = list_cameras().unwrap().into_iter().next().expect("a camera is connected");
    let (sender, reports) = mpsc::channel();
    let _monitor = ActivityMonitor::start(move |report| {
        let _ = sender.send(report);
    })
    .unwrap();

    let camera = WindowsCamera::open(&entry).unwrap();
    let preview = PreviewReader::start(&camera).unwrap();
    let _ = preview.next_frame();

    let own = std::process::id();
    let path = entry.device_path.to_lowercase();
    let seen = std::iter::from_fn(|| reports.recv_timeout(TIMEOUT).ok())
        .any(|report| report.device_path == path && report.streaming_processes.contains(&own));
    assert!(seen, "no streaming report for this process");
}

#[test]
#[ignore = "requires a connected camera"]
fn control_monitor_reports_writes() {
    let _session = MediaSession::start().unwrap();
    let entry = list_cameras().unwrap().into_iter().next().expect("a camera is connected");
    let camera = WindowsCamera::open(&entry).unwrap();
    let (sender, changes) = mpsc::channel();
    let _monitor = ControlMonitor::start(&entry.device_path, &[ControlId::Saturation], move |id| {
        let _ = sender.send(id);
    })
    .unwrap();

    let original = camera.read(ControlId::Saturation).unwrap();
    let target = if original.value > 0 { original.value - 1 } else { original.value + 1 };
    camera.write(ControlId::Saturation, ControlSetting::manual(target)).unwrap();
    let change = changes.recv_timeout(TIMEOUT);
    camera.write(ControlId::Saturation, original).unwrap();

    assert_eq!(change, Ok(ControlId::Saturation));
    assert_eq!(camera.read(ControlId::Saturation).unwrap(), original);
}
