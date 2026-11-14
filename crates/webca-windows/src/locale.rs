use windows::Win32::Globalization::GetUserDefaultLocaleName;

/// BCP 47 tag of the user's locale, such as `ru-RU`; empty when Windows cannot tell.
pub fn user_locale() -> String {
    let mut buffer = [0u16; 85];
    // SAFETY: the buffer is large enough for LOCALE_NAME_MAX_LENGTH characters.
    let length = unsafe { GetUserDefaultLocaleName(&mut buffer) };
    usize::try_from(length)
        .ok()
        .filter(|length| *length > 0)
        .map_or_else(String::new, |length| String::from_utf16_lossy(&buffer[..length - 1]))
}
