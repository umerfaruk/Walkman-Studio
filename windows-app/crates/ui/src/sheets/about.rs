//! `AboutWalkmanStudioView`, ported from
//! `Sources/WalkmanStudio/Views/AboutWalkmanStudioView.swift`.
//!
//! Spec task **17.7** (requirement 16.3): a Hardware tab (the connected Walkman's specs,
//! same fields `DeviceOverviewView`, task 14.8, already shows), a Supported DAPs tab (the
//! real `walkman_services::device_detector::CATALOG`, task 9.1's hardcoded Sony model
//! catalog — not a second list), and a Credits tab.

use eframe::egui;

use walkman_domain::models::WalkmanDevice;
use walkman_services::device_detector::CATALOG;

use crate::theme::{button, color, font};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AboutTab {
    #[default]
    Hardware,
    SupportedDaps,
    Credits,
}

impl AboutTab {
    pub const ALL: [AboutTab; 3] = [AboutTab::Hardware, AboutTab::SupportedDaps, AboutTab::Credits];

    pub fn title(self) -> &'static str {
        match self {
            AboutTab::Hardware => "Hardware",
            AboutTab::SupportedDaps => "Supported DAPs",
            AboutTab::Credits => "Credits",
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AboutSheetState {
    pub open: bool,
    pub tab: AboutTab,
}

impl AboutSheetState {
    pub fn open(&mut self) {
        *self = Self { open: true, tab: AboutTab::default() };
    }

    pub fn close(&mut self) {
        self.open = false;
    }
}

pub fn render(ctx: &egui::Context, devices: &[WalkmanDevice], state: &mut AboutSheetState) {
    if !state.open {
        return;
    }
    crate::sheets::modal(ctx, "about_walkman_studio_sheet", egui::vec2(560.0, 520.0), |ui| {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("About Walkman Studio").font(font::title2()).color(color::SILVER_LIGHT));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(button::Button::new("Close", button::Variant::Glass)).clicked() {
                    state.close();
                }
            });
        });
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            for tab in AboutTab::ALL {
                let variant = if state.tab == tab { button::Variant::Capsule } else { button::Variant::Glass };
                if ui.add(button::Button::new(tab.title(), variant)).clicked() {
                    state.tab = tab;
                }
            }
        });
        ui.add_space(10.0);

        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match state.tab {
            AboutTab::Hardware => render_hardware_tab(ui, devices),
            AboutTab::SupportedDaps => render_supported_daps_tab(ui),
            AboutTab::Credits => render_credits_tab(ui),
        });
    });
}

fn render_hardware_tab(ui: &mut egui::Ui, devices: &[WalkmanDevice]) {
    if devices.is_empty() {
        ui.label(egui::RichText::new("No Walkman connected").font(font::callout()).color(color::SILVER_MUTED));
        return;
    }
    for device in devices {
        let info = &device.device_info;
        ui.label(egui::RichText::new(&info.display_name).font(font::body_medium()).color(color::SILVER_LIGHT));
        for (label, value) in [
            ("Series", info.series_name.as_str()),
            ("Model", info.specific_model.as_str()),
            ("Vendor", info.vendor.as_str()),
            ("Firmware", info.firmware_version.as_str()),
            ("Display", info.display_resolution.as_str()),
            ("OS", if info.is_non_android { "Sony OS" } else { "Android" }),
        ] {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(label).font(font::caption()).color(color::SILVER_MUTED));
                ui.label(egui::RichText::new(value).font(font::caption_bold()).color(color::SILVER_LIGHT));
            });
        }
        ui.label(
            egui::RichText::new(format!("{:.0} GB internal", info.internal_capacity_gb))
                .font(font::caption())
                .color(color::SILVER_MUTED),
        );
        ui.add_space(10.0);
    }
}

fn render_supported_daps_tab(ui: &mut egui::Ui) {
    for entry in CATALOG {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(entry.series_name).font(font::body_medium()).color(color::SILVER_LIGHT));
            ui.label(
                egui::RichText::new(if entry.is_non_android { "Sony OS" } else { "Android" })
                    .font(font::caption())
                    .color(color::SILVER_MUTED),
            );
            ui.label(egui::RichText::new(entry.display_resolution).font(font::caption()).color(color::SILVER_MUTED));
        });
        ui.add_space(2.0);
    }
}

fn render_credits_tab(ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Walkman Studio").font(font::body_medium()).color(color::SILVER_LIGHT));
    ui.label(egui::RichText::new("A music management app for Sony Walkman digital audio players.").font(font::callout()).color(color::SILVER_MUTED));
    ui.add_space(10.0);
    ui.label(egui::RichText::new("Built with egui/eframe, symphonia, rodio, and lofty.").font(font::caption()).color(color::SILVER_MUTED));
    ui.label(egui::RichText::new("Lyrics via LRCLIB and Lyrics.ovh. Metadata via iTunes Search and Deezer.").font(font::caption()).color(color::SILVER_MUTED));
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkman_domain::models::{WalkmanDeviceInfo, WalkmanDrive};

    fn sample_device() -> WalkmanDevice {
        WalkmanDevice::new(
            WalkmanDeviceInfo {
                model_raw: "NW-A50Series".into(),
                marketing_name: "WALKMAN NW-A50Series".into(),
                display_name: "Sony Walkman NW-A55L".into(),
                series_name: "NW-A50 Series".into(),
                specific_model: "NW-A55L".into(),
                vendor: "Sony Corporation".into(),
                firmware_version: "1.02".into(),
                is_non_android: true,
                has_cfw: false,
                display_resolution: "320x240".into(),
                internal_capacity_gb: 16.0,
                supported_audio_formats: vec!["MP3".into(), "FLAC".into()],
            },
            vec![WalkmanDrive {
                name: "Internal Storage".into(),
                mount_point: "E:\\".into(),
                drive_type: walkman_domain::models::DriveType::Internal,
                total_bytes: 16_000_000_000,
                used_bytes: 1,
                free_bytes: 15_999_999_999,
                percent_used: 0.0,
                has_cfw: false,
                music_dir: "E:\\MUSIC".into(),
            }],
        )
    }

    #[test]
    fn open_resets_to_the_hardware_tab() {
        let mut state = AboutSheetState { open: false, tab: AboutTab::Credits };
        state.open();
        assert!(state.open);
        assert_eq!(state.tab, AboutTab::Hardware);
    }

    #[test]
    fn about_sheet_renders_in_headless_frame_for_every_tab_and_device_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let devices = vec![sample_device()];
        for (devices, tab) in [(Vec::new(), AboutTab::Hardware), (devices.clone(), AboutTab::Hardware), (devices, AboutTab::SupportedDaps), (Vec::new(), AboutTab::Credits)] {
            let mut state = AboutSheetState { open: true, tab };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, &devices, &mut state);
            });
        }
    }
}
