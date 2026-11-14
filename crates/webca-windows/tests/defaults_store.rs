//! Tests of the Windows camera defaults store from an unpackaged process.
//! Run with `cargo test -p webca-windows -- --ignored`.
#![cfg(windows)]

use webca_core::{ControlId, ControlSetting};
use webca_windows::{DefaultsError, DefaultsStore, MediaSession, companion_app, list_cameras};

#[test]
#[ignore = "requires a connected camera"]
fn unpackaged_process_can_read_but_not_write_defaults() {
    let _session = MediaSession::start().unwrap();
    let cameras = list_cameras().unwrap();
    let entry = cameras.first().expect("a camera is connected");
    let store = DefaultsStore::open().unwrap();

    let before = store.load(entry).unwrap();
    let result = store.save(entry, &[(ControlId::Saturation, ControlSetting::manual(128))]);

    assert_eq!(result, Err(DefaultsError::NotAuthorized));
    assert_eq!(store.load(entry).unwrap(), before);
}

#[test]
#[ignore = "requires a connected camera"]
fn companion_app_lookup_succeeds() {
    let _session = MediaSession::start().unwrap();
    let cameras = list_cameras().unwrap();
    let entry = cameras.first().expect("a camera is connected");
    let companion = companion_app(&entry.device_path).unwrap();
    println!("companion app: {companion:?}");
    assert_eq!(companion_app(r"\\?\not-a-camera").unwrap(), None);
}
