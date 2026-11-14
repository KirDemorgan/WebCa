//! Windows camera backend for WebCa, built on Media Foundation and the DirectShow control interfaces.

mod mapping;

#[cfg(windows)]
mod camera;
#[cfg(windows)]
mod defaults;
#[cfg(windows)]
mod error;
#[cfg(windows)]
mod session;

#[cfg(windows)]
pub use camera::{CameraEntry, WindowsCamera, list_cameras};
#[cfg(windows)]
pub use defaults::{DefaultsError, DefaultsStore, companion_app};
pub use mapping::{
    Location, RawRange, Timing, capability, device_error, device_parameters_key, flags, identify, locate,
    setting, timing,
};
#[cfg(windows)]
pub use session::MediaSession;
