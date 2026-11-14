use std::thread;

use windows::{
    Win32::{
        Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE, WAIT_OBJECT_0},
        System::Threading::{
            CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, INFINITE, OpenEventW, SetEvent,
            WaitForSingleObject,
        },
    },
    core::HSTRING,
};

use webca_core::DeviceError;

use crate::error::platform_error;

const MUTEX_NAME: &str = r"Local\WebCa.Instance";
const SHOW_EVENT_NAME: &str = r"Local\WebCa.Show";

/// Ownership of the per-user WebCa instance; released when dropped.
#[derive(Debug)]
pub struct PrimaryInstance {
    mutex: HANDLE,
    show_event: HANDLE,
}

// SAFETY: kernel object handles may be used and closed from any thread.
unsafe impl Send for PrimaryInstance {}

/// Result of trying to become the only running WebCa.
#[derive(Debug)]
pub enum Instance {
    Primary(PrimaryInstance),
    /// Another WebCa is running and was asked to show its window.
    Secondary,
}

impl Instance {
    pub fn acquire() -> Result<Self, DeviceError> {
        let name = HSTRING::from(MUTEX_NAME);
        // SAFETY: the name is a NUL-terminated HSTRING; the handle is closed in `Drop`.
        let created = unsafe { CreateMutexW(None, true, &name) };
        // SAFETY: read before any other call can overwrite the last error of this thread.
        let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        let mutex = created.map_err(|error| platform_error(&error))?;
        if already_running {
            // SAFETY: closing the handle we just received.
            unsafe { CloseHandle(mutex) }.map_err(|error| platform_error(&error))?;
            signal_show()?;
            return Ok(Self::Secondary);
        }
        // SAFETY: auto-reset, initially unsignaled named event; closed in `Drop`.
        let show_event = unsafe { CreateEventW(None, false, false, &HSTRING::from(SHOW_EVENT_NAME)) }
            .map_err(|error| platform_error(&error))?;
        Ok(Self::Primary(PrimaryInstance { mutex, show_event }))
    }
}

fn signal_show() -> Result<(), DeviceError> {
    // SAFETY: opening a named event for signaling only; the handle is closed right after.
    let event = unsafe { OpenEventW(EVENT_MODIFY_STATE, false, &HSTRING::from(SHOW_EVENT_NAME)) }
        .map_err(|error| platform_error(&error))?;
    // SAFETY: `event` is a valid handle with EVENT_MODIFY_STATE access.
    let result = unsafe { SetEvent(event) }.map_err(|error| platform_error(&error));
    // SAFETY: closing the handle opened above.
    let _ = unsafe { CloseHandle(event) };
    result
}

impl PrimaryInstance {
    /// Calls `on_show` on a background thread each time another launch asks for the window.
    pub fn on_show_request(&self, on_show: impl Fn() + Send + 'static) {
        let event = self.show_event.0 as usize;
        thread::Builder::new()
            .name("webca-instance".into())
            .spawn(move || {
                // SAFETY: the event outlives this thread for the lifetime of the process.
                while unsafe { WaitForSingleObject(HANDLE(event as *mut _), INFINITE) } == WAIT_OBJECT_0 {
                    on_show();
                }
            })
            .expect("the instance thread starts");
    }
}

impl Drop for PrimaryInstance {
    fn drop(&mut self) {
        // SAFETY: closing handles we own; the process is shutting the instance down.
        unsafe {
            let _ = CloseHandle(self.show_event);
            let _ = CloseHandle(self.mutex);
        }
    }
}
