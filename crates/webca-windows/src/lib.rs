//! Windows camera backend for WebCa, built on Media Foundation and the DirectShow control interfaces.

mod frame;
mod mapping;

#[cfg(windows)]
mod camera;
#[cfg(windows)]
mod defaults;
#[cfg(windows)]
mod error;
#[cfg(windows)]
mod preview;
#[cfg(windows)]
mod session;

#[cfg(windows)]
pub use camera::{CameraEntry, WindowsCamera, list_cameras};
#[cfg(windows)]
pub use defaults::{DefaultsError, DefaultsStore, companion_app};
pub use frame::{PreviewFrame, VideoFormat, pack_bgra, pick_preview_format};
pub use mapping::{
    Location, RawRange, Timing, capability, device_error, device_parameters_key, flags, identify, locate,
    setting, timing,
};
#[cfg(windows)]
pub use preview::PreviewReader;
#[cfg(windows)]
pub use session::MediaSession;
