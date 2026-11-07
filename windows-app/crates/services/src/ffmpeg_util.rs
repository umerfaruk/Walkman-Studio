//! Bundled-first `ffmpeg`/`ffprobe` discovery, shared by task 8.3 (M4A tagging) and the future
//! task 10.1 (transcoding): look for the binary bundled under `assets/bin/` next to the running
//! executable first, then fall back to `PATH` (design.md: "discovery prefers bundled, then
//! PATH"). Bundling the binaries themselves is task 19.1's job; this module only locates them.

use std::path::PathBuf;

pub fn find_ffmpeg() -> Option<PathBuf> {
    find_binary("ffmpeg.exe", "ffmpeg")
}

pub fn find_ffprobe() -> Option<PathBuf> {
    find_binary("ffprobe.exe", "ffprobe")
}

fn find_binary(bundled_name: &str, path_name: &str) -> Option<PathBuf> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("assets").join("bin").join(bundled_name);
            if bundled.is_file() {
                return Some(bundled);
            }
            let next_to_exe = dir.join(bundled_name);
            if next_to_exe.is_file() {
                return Some(next_to_exe);
            }
        }
    }
    find_on_path(path_name)
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var).find_map(|dir| {
        let candidate = dir.join(name).with_extension("exe");
        candidate.is_file().then_some(candidate)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_binary_on_a_custom_path() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("ffmpeg.exe");
        std::fs::write(&exe, b"not a real binary").unwrap();

        let original_path = std::env::var_os("PATH");
        // SAFETY: tests in this module run serially within this process's `cargo test` thread
        // pool only insofar as no other test in this crate reads/writes `PATH`; scoping the
        // mutation to this single call keeps the window as small as possible.
        unsafe {
            std::env::set_var("PATH", dir.path());
        }
        let found = find_on_path("ffmpeg");
        if let Some(path) = original_path {
            unsafe {
                std::env::set_var("PATH", path);
            }
        }

        assert_eq!(found, Some(exe));
    }

    #[test]
    fn returns_none_when_not_found_anywhere() {
        let original_path = std::env::var_os("PATH");
        unsafe {
            std::env::set_var("PATH", "");
        }
        let found = find_on_path("a-binary-that-definitely-does-not-exist-anywhere");
        if let Some(path) = original_path {
            unsafe {
                std::env::set_var("PATH", path);
            }
        }
        assert_eq!(found, None);
    }
}
