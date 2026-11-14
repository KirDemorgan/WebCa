//! Camera control model, profiles and apply planning for WebCa.
//!
//! This crate has no platform dependencies: every type here can be exercised in tests on any OS.

mod apply;
mod control;
mod device;
mod editor;
mod guard;
mod profile;
mod range;
mod setting;
mod store;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use apply::{ApplyPlan, ApplyReport, ControlState, SkipReason, apply, capture, snapshot};
pub use control::{ControlGroup, ControlId, UnknownControl};
pub use device::{CameraDevice, CameraInfo, ControlCapability, DeviceError};
pub use editor::{Editor, LiveWrite};
pub use guard::{Action, Guard, GuardConfig, Signal};
pub use profile::{CameraKey, CameraProfiles, Profile, ProfileBook, ProfileError};
pub use range::{ControlRange, RangeError};
pub use setting::{ControlMode, ControlSetting};
pub use store::{ProfileStore, StoreError};
