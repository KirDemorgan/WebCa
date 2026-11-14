use windows::Win32::{
    Media::MediaFoundation::{MF_VERSION, MFSTARTUP_FULL, MFShutdown, MFStartup},
    System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize},
};

use crate::error::platform_error;
use webca_core::DeviceError;

/// Keeps COM and Media Foundation initialized on the current thread while alive.
#[derive(Debug)]
pub struct MediaSession {
    uninitialize_com: bool,
}

impl MediaSession {
    pub fn start() -> Result<Self, DeviceError> {
        // SAFETY: called on the owning thread; balanced by `CoUninitialize` in `Drop` when it succeeded.
        let uninitialize_com = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) }.is_ok();
        // SAFETY: MFStartup is reference counted and balanced by MFShutdown in `Drop`.
        if let Err(error) = unsafe { MFStartup(MF_VERSION, MFSTARTUP_FULL) } {
            if uninitialize_com {
                // SAFETY: balances the successful CoInitializeEx above.
                unsafe { CoUninitialize() };
            }
            return Err(platform_error(&error));
        }
        Ok(Self { uninitialize_com })
    }
}

impl Drop for MediaSession {
    fn drop(&mut self) {
        // SAFETY: balances the MFStartup in `start`.
        let _ = unsafe { MFShutdown() };
        if self.uninitialize_com {
            // SAFETY: balances the CoInitializeEx in `start`.
            unsafe { CoUninitialize() };
        }
    }
}
