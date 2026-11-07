//! `DeviceOverviewView` (spec task 14.8), ported from `DeviceOverviewView.swift`: the connected
//! Walkman's model/specs, per-drive storage gauges, quick stats, Refresh/Eject, a multi-device
//! switcher, and the Android MTP help sheet + custom-folder bridge (R3.5/R3.7).
//!
//! No real device model image is wired up yet (that needs a bundled asset keyed by
//! `WalkmanDeviceInfo::specific_model`, a packaging concern — task 19.1); the header shows a
//! placeholder glyph tile instead.

use eframe::egui;

use walkman_domain::models::WalkmanDevice;

use crate::theme::{button, color, font, glass, widgets};

/// Quick-stats summary for the active device's library (R3.5's "quick stats
/// (albums/artists/hi-res)"). Supplied by the caller — real numbers come from the precomputed
/// caches (task 7.3/18), not from this view.
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
    /// Index into the `devices` slice passed to [`render`] (R3.5's multi-device switcher).
    pub selected_device_index: usize,
    /// Whether the Android MTP help sheet is open.
    pub show_mtp_help: bool,
    pub commands: Vec<DeviceOverviewCommand>,
}

/// Renders the Device Overview for `devices[state.selected_device_index]` (clamped), or a
/// "no Walkman connected" placeholder if `devices` is empty.
pub fn render(ui: &mut egui::Ui, devices: &[WalkmanDevice], stats: DeviceQuickStats, state: &mut DeviceOverviewViewState) {
    if devices.is_empty() {
        render_disconnected(ui);
        return;
    }
    state.selected_device_index = state.selected_device_index.min(devices.len() - 1);

    if devices.len() > 1 {
        render_device_switcher(ui, devices, state);
        ui.add_space(8.0);
    }

    let device = &devices[state.selected_device_index];
    render_header(ui, device);
    ui.add_space(12.0);
    render_quick_stats(ui, stats);
    ui.add_space(12.0);
    render_drive_gauges(ui, device);
    ui.add_space(12.0);
    render_toolbar(ui, device, state);

    if device.device_info.is_android() {
        ui.add_space(12.0);
        render_mtp_banner(ui, state);
    }

    if state.show_mtp_help {
        render_mtp_help_sheet(ui, state);
    }
}

fn render_disconnected(ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(48.0);
        ui.label(egui::RichText::new("No Walkman Connected").font(font::title2()).color(color::SILVER_LIGHT));
        ui.label(
            egui::RichText::new("Connect a Sony Walkman via USB to see its overview here.")
                .font(font::callout())
                .color(color::SILVER_MUTED),
        );
    });
}

fn render_device_switcher(ui: &mut egui::Ui, devices: &[WalkmanDevice], state: &mut DeviceOverviewViewState) {
    ui.horizontal(|ui| {
        for (index, device) in devices.iter().enumerate() {
            let active = state.selected_device_index == index;
            let variant = if active { button::Variant::Capsule } else { button::Variant::Glass };
            if ui.add(button::Button::new(device.device_info.specific_model.as_str(), variant)).clicked() {
                state.selected_device_index = index;
            }
        }
    });
}

fn render_header(ui: &mut egui::Ui, device: &WalkmanDevice) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(egui::vec2(72.0, 72.0), egui::Sense::hover());
        ui.painter().rect_filled(rect, 10.0, color::INPUT);
        let galley = ui.painter().layout_no_wrap("\u{1F3B5}".to_string(), font::title1(), color::SILVER_MUTED);
        let pos = rect.center() - galley.size() * 0.5;
        ui.painter().galley(pos, galley, color::SILVER_MUTED);

        ui.vertical(|ui| {
            ui.label(egui::RichText::new(&device.device_info.display_name).font(font::title2()).color(color::SILVER_LIGHT));
            ui.label(
                egui::RichText::new(format!(
                    "{} \u{2022} {} \u{2022} FW {}",
                    device.device_info.series_name,
                    device.device_info.os_family(),
                    device.device_info.firmware_version
                ))
                .font(font::callout())
                .color(color::SILVER_MUTED),
            );
            ui.label(
                egui::RichText::new(format!(
                    "{} \u{2022} {:.0} GB internal",
                    device.device_info.display_resolution, device.device_info.internal_capacity_gb
                ))
                .font(font::caption())
                .color(color::SILVER_MUTED),
            );
        });
    });
}

fn render_quick_stats(ui: &mut egui::Ui, stats: DeviceQuickStats) {
    ui.horizontal(|ui| {
        for (label, value) in [
            ("Albums", stats.album_count),
            ("Artists", stats.artist_count),
            ("Hi-Res Tracks", stats.hi_res_count),
        ] {
            let frame = glass::liquid_glass(glass::State::Normal);
            frame.show(ui, |ui| {
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new(value.to_string()).font(font::title3()).color(color::SILVER_LIGHT));
                    ui.label(egui::RichText::new(label).font(font::caption()).color(color::SILVER_MUTED));
                });
            });
            ui.add_space(8.0);
        }
    });
}

fn render_drive_gauges(ui: &mut egui::Ui, device: &WalkmanDevice) {
    for drive in &device.drives {
        let frame = glass::liquid_glass(glass::State::Normal);
        frame.show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical(|ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(&drive.name).font(font::body_medium()).color(color::SILVER_LIGHT));
                    ui.label(
                        egui::RichText::new(format!(
                            "{} / {}",
                            walkman_services::walkman_engine::format_bytes(drive.used_bytes),
                            walkman_services::walkman_engine::format_bytes(drive.total_bytes)
                        ))
                        .font(font::caption())
                        .color(color::SILVER_MUTED),
                    );
                });
                widgets::StorageGauge::from_bytes(drive.used_bytes.max(0) as u64, drive.total_bytes.max(0) as u64).show(ui);
            });
        });
        ui.add_space(6.0);
    }
}

fn render_toolbar(ui: &mut egui::Ui, device: &WalkmanDevice, state: &mut DeviceOverviewViewState) {
    ui.horizontal(|ui| {
        if ui.add(button::Button::new("Refresh", button::Variant::Glass)).clicked() {
            state.commands.push(DeviceOverviewCommand::Refresh);
        }
        if let Some(drive) = device.internal_drive().or_else(|| device.drives.first()) {
            if ui.add(button::Button::new("Eject", button::Variant::Destructive)).clicked() {
                state.commands.push(DeviceOverviewCommand::Eject(drive.mount_point.clone()));
            }
        }
    });
}

fn render_mtp_banner(ui: &mut egui::Ui, state: &mut DeviceOverviewViewState) {
    let frame = glass::liquid_glass(glass::State::Normal);
    frame.show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("This Android Walkman exposes storage via MTP, not a drive letter.")
                    .font(font::callout())
                    .color(color::SILVER_MUTED),
            );
            if ui.add(button::Button::new("Learn More\u{2026}", button::Variant::Gold)).clicked() {
                state.show_mtp_help = true;
            }
        });
    });
}

fn render_mtp_help_sheet(ui: &mut egui::Ui, state: &mut DeviceOverviewViewState) {
    let ctx = ui.ctx().clone();
    let screen = ctx.screen_rect();
    egui::Area::new(egui::Id::new("android_mtp_help_sheet")).order(egui::Order::Foreground).fixed_pos(screen.min).show(
        &ctx,
        |ui| {
            ui.set_min_size(screen.size());
            ui.painter().rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));
            let panel_rect = egui::Rect::from_center_size(screen.center(), egui::vec2(520.0, 320.0));
            let mut panel_ui = ui.new_child(egui::UiBuilder::new().max_rect(panel_rect));
            glass::modal_glass_chassis().show(&mut panel_ui, |ui| {
                ui.label(
                    egui::RichText::new("Android Walkman Storage").font(font::title2()).color(color::SILVER_LIGHT),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(
                        "Android-based Walkmans expose their storage over MTP rather than as a mounted drive. \
                         Choose a custom folder (e.g. the device's music folder, mounted/synced via Windows) to \
                         use as the library root for transfers and browsing.",
                    )
                    .font(font::callout())
                    .color(color::SILVER_MUTED),
                );
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    if ui.add(button::Button::new("Choose Folder\u{2026}", button::Variant::Prominent)).clicked() {
                        state.commands.push(DeviceOverviewCommand::PickCustomFolder);
                    }
                    if ui.add(button::Button::new("Close", button::Variant::Glass)).clicked() {
                        state.show_mtp_help = false;
                    }
                });
            });
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkman_domain::models::{DriveType, WalkmanDeviceInfo, WalkmanDrive};

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
    fn renders_disconnected_placeholder_with_no_devices() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let mut state = DeviceOverviewViewState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render(ui, &[], DeviceQuickStats::default(), &mut state);
            });
        });
    }

    #[test]
    fn renders_a_single_sony_os_device_with_drive_gauges() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let devices = [device(true, vec![drive("Internal Storage", DriveType::Internal, 16_000_000_000, 4_000_000_000)])];
        let mut state = DeviceOverviewViewState::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render(ui, &devices, DeviceQuickStats { album_count: 5, artist_count: 3, hi_res_count: 1 }, &mut state);
            });
        });
        assert!(!state.show_mtp_help);
    }

    #[test]
    fn android_device_shows_mtp_banner_and_help_sheet() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let devices = [device(false, vec![drive("Internal Storage", DriveType::Internal, 32_000_000_000, 1_000_000_000)])];
        let mut state = DeviceOverviewViewState { show_mtp_help: true, ..Default::default() };
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render(ui, &devices, DeviceQuickStats::default(), &mut state);
            });
        });
    }

    #[test]
    fn multi_device_switcher_clamps_out_of_range_selection() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);
        let devices = vec![
            device(true, vec![drive("Internal Storage", DriveType::Internal, 16_000_000_000, 1_000_000_000)]),
            device(true, vec![drive("Internal Storage", DriveType::Internal, 32_000_000_000, 1_000_000_000)]),
        ];
        let mut state = DeviceOverviewViewState { selected_device_index: 99, ..Default::default() };
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                render(ui, &devices, DeviceQuickStats::default(), &mut state);
            });
        });
        assert_eq!(state.selected_device_index, 1);
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
