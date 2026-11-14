use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

/// Connects standard output to the terminal that launched this windowed program, if any.
pub fn attach_parent_console() {
    // SAFETY: attaching fails harmlessly when there is no parent console.
    let _ = unsafe { AttachConsole(ATTACH_PARENT_PROCESS) };
}
