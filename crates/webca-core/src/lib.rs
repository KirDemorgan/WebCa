//! Camera control model, profiles and apply planning for WebCa.
//!
//! This crate has no platform dependencies: every type here can be exercised in tests on any OS.

mod control;
mod profile;
mod range;
mod setting;
mod store;

pub use control::{ControlGroup, ControlId, UnknownControl};
pub use profile::{CameraKey, CameraProfiles, Profile, ProfileBook, ProfileError};
pub use range::{ControlRange, RangeError};
pub use setting::{ControlMode, ControlSetting};
pub use store::{ProfileStore, StoreError};
