//! `AboutWalkmanStudioView`, ported from
//! `Sources/WalkmanStudio/Views/AboutWalkmanStudioView.swift`.
//!
//! Spec task **17.7** (requirement 16.3), corrected under task 19.4's parity check: the real
//! macOS view has 4 tabs (`AboutTab`: `.hardware`, `.supportedDAPs`, `.engineRules`,
//! `.developerSupport`) — this originally shipped with a 3rd tab named "Credits" that didn't
//! match either real tab, caught by comparing against `Sources/WalkmanStudio/Views/
//! AboutWalkmanStudioView.swift` directly. Hardware reuses the same `WalkmanDeviceInfo`
//! fields `DeviceOverviewView` (task 14.8) already shows; Supported DAPs iterates the real
//! `device_detector::CATALOG` (task 9.1); Engine Rules and Developer & Support now carry the
//! real card text/copy from the Swift source instead of invented placeholder copy.

use eframe::egui;

use walkman_domain::models::WalkmanDevice;
use walkman_services::device_detector::CATALOG;

use crate::theme::{button, color, font, glass};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AboutTab {
    #[default]
    Hardware,
    SupportedDaps,
    EngineRules,
    DeveloperSupport,
}

impl AboutTab {
    pub const ALL: [AboutTab; 4] = [AboutTab::Hardware, AboutTab::SupportedDaps, AboutTab::EngineRules, AboutTab::DeveloperSupport];

    pub fn title(self) -> &'static str {
        match self {
            AboutTab::Hardware => "Hardware",
            AboutTab::SupportedDaps => "Supported DAPs",
            AboutTab::EngineRules => "Engine Rules",
            AboutTab::DeveloperSupport => "Developer & Support",
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
            AboutTab::EngineRules => render_engine_rules_tab(ui),
            AboutTab::DeveloperSupport => render_developer_support_tab(ui),
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

/// The 4 compliance "rule cards" from `AboutWalkmanStudioView.swift`'s `engineRulesTabView`,
/// carried over verbatim (title + detail text) rather than paraphrased.
const ENGINE_RULES: [(&str, &str); 4] = [
    (
        "Baseline RGB JPEG Artwork",
        "Strips progressive passes, CMYK color spaces, and limits resolution to 800\u{d7}800 to prevent display engine freezes.",
    ),
    (
        "Strict ID3v2.3 (UTF-16)",
        "Enforces UTF-16 with BOM (encoding=1) and binary ID3v2.3 tags, avoiding ID3v2.4 database corruption.",
    ),
    (
        "CRLF Relative .M3U8 Playlists",
        "Writes Windows-compliant CRLF (\\r\\n) line endings and relative POSIX paths directly into /MUSIC/.",
    ),
    (
        "macOS Shadow File Purge",
        "Merges AppleDouble forks with dot_clean -m and removes ._* and .DS_Store ghost files before safe eject.",
    ),
];

fn render_engine_rules_tab(ui: &mut egui::Ui) {
    for (title, detail) in ENGINE_RULES {
        glass::liquid_glass(glass::State::Normal).show(ui, |ui| {
            ui.set_min_width(ui.available_width() - 16.0);
            ui.label(egui::RichText::new(title).font(font::body_medium()).color(color::SILVER_LIGHT));
            ui.label(egui::RichText::new(detail).font(font::caption()).color(color::SILVER_MUTED));
        });
        ui.add_space(8.0);
    }
}

fn render_developer_support_tab(ui: &mut egui::Ui) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("John Lloyd Malabanan").font(font::title2()).color(color::SILVER_LIGHT));
        ui.label(egui::RichText::new("Creator").font(font::badge()).color(color::HIRES_GOLD));
    });
    ui.label(egui::RichText::new("GitHub: @Jhay-EL \u{2022} UX/UI Designer & Software Developer").font(font::subheadline_medium()).color(color::SILVER_MUTED));
    ui.add_space(8.0);
    ui.label(
        egui::RichText::new(
            "Walkman Studio was designed and engineered from the ground up to solve the challenges of using \
             modern macOS with Sony Walkman digital audio players. It automates strict ID3v2.3 & FLAC tagging, \
             Baseline RGB JPEG cover art conversion, Sony 12 Tone / SenseMe acoustic analysis, synchronized .LRC \
             lyrics, batch metadata editing, dual-partition storage routing, and AppleDouble ghost file cleanup.",
        )
        .font(font::callout())
        .color(color::SILVER_MUTED),
    );
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
        let mut state = AboutSheetState { open: false, tab: AboutTab::DeveloperSupport };
        state.open();
        assert!(state.open);
        assert_eq!(state.tab, AboutTab::Hardware);
    }

    #[test]
    fn about_sheet_renders_in_headless_frame_for_every_tab_and_device_state() {
        let ctx = egui::Context::default();
        crate::theme::install_fonts(&ctx);

        let devices = vec![sample_device()];
        for (devices, tab) in [
            (Vec::new(), AboutTab::Hardware),
            (devices.clone(), AboutTab::Hardware),
            (devices, AboutTab::SupportedDaps),
            (Vec::new(), AboutTab::EngineRules),
            (Vec::new(), AboutTab::DeveloperSupport),
        ] {
            let mut state = AboutSheetState { open: true, tab };
            let _ = ctx.run(egui::RawInput::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |_ui| {});
                render(ctx, &devices, &mut state);
            });
        }
    }
}
