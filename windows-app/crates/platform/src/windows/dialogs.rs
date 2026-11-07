//! [`FileDialogs`] for Windows (spec task 6.6).
//!
//! Uses the classic shell/common-dialog APIs (`SHBrowseForFolderW`, `GetSaveFileNameW`) rather
//! than the newer `IFileOpenDialog`/`IFileSaveDialog` COM interfaces: same user-facing dialogs,
//! far fewer moving parts (no COM activation/lifetime management), which keeps this port's first
//! Windows dialog implementation easy to reason about and maintain.

use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::UI::Controls::Dialogs::{GetSaveFileNameW, OPENFILENAMEW, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST};
use windows::Win32::UI::Shell::{
    SHBrowseForFolderW, SHGetPathFromIDListW, BIF_NEWDIALOGSTYLE, BIF_RETURNONLYFSDIRS,
    BROWSEINFOW,
};
use windows::core::PCWSTR;

use crate::windows::util::{from_wide, to_wide};
use crate::FileDialogs;

const MAX_PATH_WIDE: usize = 32768;

/// Real `FileDialogs` backed by the Win32 shell/common dialogs.
#[derive(Debug, Default)]
pub struct WindowsFileDialogs;

impl WindowsFileDialogs {
    pub fn new() -> Self {
        Self
    }
}

impl FileDialogs for WindowsFileDialogs {
    fn pick_folder(&self, title: &str) -> Option<PathBuf> {
        let title_w = to_wide(title);
        let mut display_name = [0u16; 260];

        let info = BROWSEINFOW {
            lpszTitle: PCWSTR(title_w.as_ptr()),
            pszDisplayName: windows::core::PWSTR(display_name.as_mut_ptr()),
            ulFlags: BIF_RETURNONLYFSDIRS | BIF_NEWDIALOGSTYLE,
            ..Default::default()
        };

        let pidl = unsafe { SHBrowseForFolderW(&info) };
        if pidl.is_null() {
            return None;
        }

        let mut path_buf = [0u16; 260];
        let ok = unsafe { SHGetPathFromIDListW(pidl, &mut path_buf) };
        unsafe {
            windows::Win32::System::Com::CoTaskMemFree(Some(pidl as *const core::ffi::c_void));
        }
        if !ok.as_bool() {
            return None;
        }

        let path = from_wide(&path_buf);
        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    }

    fn save_file(&self, suggested_name: &str, filters: &[(&str, &str)]) -> Option<PathBuf> {
        let mut file_buf = [0u16; MAX_PATH_WIDE];
        let suggested_w = to_wide(suggested_name);
        let copy_len = suggested_w.len().min(file_buf.len());
        file_buf[..copy_len].copy_from_slice(&suggested_w[..copy_len]);

        // GetSaveFileNameW expects pairs of NUL-terminated strings ("Description", "*.ext"),
        // terminated by a final empty string (i.e. a double NUL at the very end).
        let mut filter_w: Vec<u16> = Vec::new();
        for (description, pattern) in filters {
            filter_w.extend(description.encode_utf16());
            filter_w.push(0);
            filter_w.extend(pattern.encode_utf16());
            filter_w.push(0);
        }
        filter_w.push(0);
        if filter_w.len() == 1 {
            // No filters supplied: fall back to "all files".
            filter_w.clear();
            filter_w.extend("All Files\0*.*\0".encode_utf16());
            filter_w.push(0);
        }

        let mut ofn = OPENFILENAMEW {
            lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
            lpstrFilter: PCWSTR(filter_w.as_ptr()),
            lpstrFile: windows::core::PWSTR(file_buf.as_mut_ptr()),
            nMaxFile: file_buf.len() as u32,
            Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
            ..Default::default()
        };

        let ok = unsafe { GetSaveFileNameW(&mut ofn) };
        if !ok.as_bool() {
            return None;
        }

        let path = from_wide(&file_buf);
        if path.is_empty() {
            None
        } else {
            Some(PathBuf::from(path))
        }
    }

    fn reveal_in_file_manager(&self, path: &Path) {
        let _ = Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .spawn();
    }
}
