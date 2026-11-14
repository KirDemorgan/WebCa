//! Windows camera backend for WebCa, built on Media Foundation and the DirectShow control interfaces.

mod mapping;

#[cfg(windows)]
mod camera;
#[cfg(windows)]
mod error;
#[cfg(windows)]
mod session;

#[cfg(windows)]
pub use camera::{CameraEntry, WindowsCamera, list_cameras};
pub use mapping::{Location, RawRange, capability, device_error, flags, locate, setting};
#[cfg(windows)]
pub use session::MediaSession;
