//! Windows camera backend for WebCa, built on Media Foundation and the DirectShow control interfaces.

mod frame;
mod mapping;

#[cfg(windows)]
mod autostart;
#[cfg(windows)]
mod camera;
#[cfg(windows)]
mod companion;
#[cfg(windows)]
mod console;
#[cfg(windows)]
mod defaults;
#[cfg(windows)]
mod error;
#[cfg(windows)]
mod instance;
#[cfg(windows)]
mod monitor;
#[cfg(windows)]
mod preview;
#[cfg(windows)]
mod session;

#[cfg(windows)]
pub use autostart::{autostart_command, autostart_entry, disable_autostart, enable_autostart};
#[cfg(windows)]
pub use camera::{CameraEntry, WindowsCamera, list_cameras};
#[cfg(windows)]
pub use companion::{associate, current_package_family, dissociate, run_elevated};
#[cfg(windows)]
pub use console::attach_parent_console;
#[cfg(windows)]
pub use defaults::{DefaultsError, DefaultsStore, companion_app};
pub use frame::{PreviewFrame, VideoFormat, pack_bgra, pick_preview_format};
#[cfg(windows)]
pub use instance::{Instance, PrimaryInstance};
pub use mapping::{
    Location, RawRange, Timing, capability, device_error, device_instance_id, device_parameters_key, flags,
    identify, locate, setting, timing,
};
#[cfg(windows)]
pub use monitor::{ActivityMonitor, ActivityReport, ControlMonitor};
#[cfg(windows)]
pub use preview::PreviewReader;
#[cfg(windows)]
pub use session::MediaSession;
