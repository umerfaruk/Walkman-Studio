//! Pure types/logic behind `DeviceOverviewView`, ported from `DeviceOverviewView.swift`.
//! The rendering itself lives in `iced_ui::views::device_overview` (the egui `render`
//! this module used to carry was removed once the iced migration reached parity — see
//! Milestone 8 of the egui-to-iced migration plan).

/// Quick-stats summary for the active device's library (R3.5's "quick stats
/// (albums/artists/hi-res)"). Supplied by the caller — real numbers come from the precomputed
/// caches, not from this module.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DeviceQuickStats {
    pub album_count: usize,
    pub artist_count: usize,
    pub hi_res_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DeviceOverviewCommand {
    Refresh,
    Eject(String),
    PickCustomFolder,
}

#[derive(Debug, Default)]
pub struct DeviceOverviewViewState {
    /// Index into the `devices` slice the view is given (R3.5's multi-device switcher).
    pub selected_device_index: usize,
    /// Whether the Android MTP help sheet is open.
    pub show_mtp_help: bool,
    pub commands: Vec<DeviceOverviewCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkman_domain::models::{DriveType, WalkmanDevice, WalkmanDeviceInfo, WalkmanDrive};

    fn drive(name: &str, ty: DriveType, total: i64, used: i64) -> WalkmanDrive {
        WalkmanDrive {
            name: name.into(),
            mount_point: format!("/Volumes/{name}"),
            drive_type: ty,
            total_bytes: total,
            used_bytes: used,
            free_bytes: total - used,
            percent_used: if total > 0 { used as f64 / total as f64 * 100.0 } else { 0.0 },
            has_cfw: false,
            music_dir: format!("/Volumes/{name}/MUSIC"),
        }
    }

    fn device(is_non_android: bool, drives: Vec<WalkmanDrive>) -> WalkmanDevice {
        WalkmanDevice::new(
            WalkmanDeviceInfo {
                model_raw: "NW-A105".into(),
                marketing_name: "WALKMAN NW-A100Series".into(),
                display_name: "Sony Walkman NW-A105".into(),
                series_name: "NW-A100 Series".into(),
                specific_model: "NW-A105".into(),
                vendor: "Sony Corporation".into(),
                firmware_version: "2.00".into(),
                is_non_android,
                has_cfw: false,
                display_resolution: "720x1280".into(),
                internal_capacity_gb: 16.0,
                supported_audio_formats: vec!["FLAC".into(), "MP3".into()],
            },
            drives,
        )
    }

    #[test]
    fn eject_command_targets_the_internal_drives_mount_point() {
        let devices = [device(
            true,
            vec![
                drive("microSD Card", DriveType::Microsd, 64_000_000_000, 1_000_000_000),
                drive("Internal Storage", DriveType::Internal, 16_000_000_000, 1_000_000_000),
            ],
        )];
        assert_eq!(devices[0].internal_drive().unwrap().mount_point, "/Volumes/Internal Storage");
    }
}
