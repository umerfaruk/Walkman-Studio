//! Small shared helpers for the Win32-backed platform implementations.

/// Encodes `s` as a NUL-terminated UTF-16 buffer suitable for `PCWSTR`/`PWSTR` Win32 calls.
pub(super) fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Decodes a NUL-terminated (or fully-filled) UTF-16 buffer into a `String`, stopping at the
/// first NUL.
pub(super) fn from_wide(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

/// Formats a drive letter (`'E'`) as the canonical root path Win32 expects (`E:\`).
pub(super) fn drive_root(letter: char) -> String {
    format!("{letter}:\\")
}
