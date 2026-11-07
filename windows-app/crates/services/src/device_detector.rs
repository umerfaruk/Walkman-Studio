//! Sony Walkman model catalog and volume/USB signature classification (spec task 9.1).
//!
//! The authoritative identification signal is the hardcoded catalog below (USB VID/PID plus
//! product-string substrings), matching R3.1/R3.3's "same hardcoded Sony catalog as the macOS
//! app". `inspect_volume` additionally looks for the on-device signature files/folders R3.2
//! lists to confirm a mounted volume is an authentic Sony drive and to tell its Internal vs.
//! MicroSD partition apart, and does a best-effort scan of `default-capability.xml` for a model
//! string as a secondary signal when present — real device capability-XML schemas vary and this
//! repo has no captured sample to parse exactly, so it looks for an `NW-XXXX`-shaped token
//! anywhere in the file rather than a specific tag name.

use std::path::Path;

use walkman_domain::models::DriveType;

/// Sony's registered USB vendor id (R3.1).
pub const SONY_VENDOR_ID: u16 = 0x054C;

/// One entry in the hardcoded Sony Walkman catalog (R3.3): a product series plus every
/// identifying signal the macOS app's own catalog carries for it.
#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry {
    pub series_name: &'static str,
    pub marketing_name: &'static str,
    /// `true` for the classic Sony-OS (Linux DAP) line, `false` for Android Walkman OS.
    pub is_non_android: bool,
    pub product_ids: &'static [u16],
    /// Substrings matched case-insensitively against a capability-XML model token or a USB
    /// product string, to resolve a *specific* model within the series.
    pub model_tokens: &'static [&'static str],
    pub display_resolution: &'static str,
    pub icon_key: &'static str,
    /// `(internal_capacity_gb, specific_model_name)`, nearest-match, for series whose members
    /// share one USB PID and are only told apart by capacity (R3.3's "capacity-based ... split"
    /// — e.g. the NW-A50 series ships as the A55/A56/A57 purely by storage size).
    pub capacity_models: &'static [(f64, &'static str)],
}

/// The hardcoded Sony Walkman catalog (R3.3): NW-A10/A20/A30/A40/A50, NW-ZX100/ZX300,
/// NW-WM1A/WM1Z, NW-WS/E/S/B, and the Android NW-A100/A300, NW-ZX500/ZX700, NW-WM1AM2/WM1ZM2.
pub const CATALOG: &[CatalogEntry] = &[
    CatalogEntry {
        series_name: "NW-A50 Series",
        marketing_name: "WALKMAN NW-A50Series",
        is_non_android: true,
        product_ids: &[],
        model_tokens: &["NW-A50", "NW-A55", "NW-A56", "NW-A57"],
        display_resolution: "320x240",
        icon_key: "hifispeaker.fill",
        capacity_models: &[(16.0, "NW-A55"), (32.0, "NW-A56"), (64.0, "NW-A57")],
    },
    CatalogEntry {
        series_name: "NW-A100 Series",
        marketing_name: "WALKMAN NW-A100Series",
        is_non_android: false,
        product_ids: &[0x0D00, 0x0CFF, 0x0D02],
        model_tokens: &["NW-A100", "NW-A105", "NW-A106", "NW-A107"],
        display_resolution: "720x1280",
        icon_key: "hifispeaker.fill",
        capacity_models: &[(16.0, "NW-A105"), (32.0, "NW-A106"), (64.0, "NW-A107")],
    },
    CatalogEntry {
        series_name: "NW-A300 Series",
        marketing_name: "WALKMAN NW-A300Series",
        is_non_android: false,
        product_ids: &[0x0E6E, 0x0E72],
        model_tokens: &["NW-A300", "NW-A306", "NW-A307", "NW-A308"],
        display_resolution: "720x1280",
        icon_key: "hifispeaker.fill",
        capacity_models: &[(32.0, "NW-A306"), (64.0, "NW-A307"), (128.0, "NW-A308")],
    },
    CatalogEntry {
        series_name: "NW-ZX100 Series",
        marketing_name: "WALKMAN NW-ZX100Series",
        is_non_android: true,
        product_ids: &[],
        model_tokens: &["NW-ZX100"],
        display_resolution: "320x240",
        icon_key: "hifispeaker.fill",
        capacity_models: &[],
    },
    CatalogEntry {
        series_name: "NW-ZX300 Series",
        marketing_name: "WALKMAN NW-ZX300Series",
        is_non_android: true,
        product_ids: &[],
        model_tokens: &["NW-ZX300"],
        display_resolution: "480x234",
        icon_key: "hifispeaker.fill",
        capacity_models: &[],
    },
    CatalogEntry {
        series_name: "NW-ZX500 Series",
        marketing_name: "WALKMAN NW-ZX500Series",
        is_non_android: false,
        product_ids: &[0x0D01, 0x0D03],
        model_tokens: &["NW-ZX500", "NW-ZX507"],
        display_resolution: "720x1280",
        icon_key: "hifispeaker.fill",
        capacity_models: &[],
    },
    CatalogEntry {
        series_name: "NW-ZX700 Series",
        marketing_name: "WALKMAN NW-ZX700Series",
        is_non_android: false,
        product_ids: &[0x0E6F, 0x0E73],
        model_tokens: &["NW-ZX700", "NW-ZX707"],
        display_resolution: "720x1280",
        icon_key: "hifispeaker.fill",
        capacity_models: &[],
    },
    CatalogEntry {
        series_name: "NW-WM1 Series",
        marketing_name: "WALKMAN NW-WM1Series",
        is_non_android: true,
        product_ids: &[],
        model_tokens: &["NW-WM1A", "NW-WM1Z"],
        display_resolution: "480x234",
        icon_key: "hifispeaker.fill",
        capacity_models: &[],
    },
    CatalogEntry {
        series_name: "NW-WM1AM2 Series",
        marketing_name: "WALKMAN NW-WM1Series",
        is_non_android: false,
        product_ids: &[0x0E70, 0x0E74],
        model_tokens: &["NW-WM1AM2"],
        display_resolution: "720x1280",
        icon_key: "hifispeaker.fill",
        capacity_models: &[],
    },
    CatalogEntry {
        series_name: "NW-WM1ZM2 Series",
        marketing_name: "WALKMAN NW-WM1Series",
        is_non_android: false,
        product_ids: &[0x0E71, 0x0E75],
        model_tokens: &["NW-WM1ZM2"],
        display_resolution: "720x1280",
        icon_key: "hifispeaker.fill",
        capacity_models: &[],
    },
    CatalogEntry {
        series_name: "NW-WS/E/S/B Series",
        marketing_name: "WALKMAN NW-WSSeries",
        is_non_android: true,
        product_ids: &[],
        model_tokens: &["NW-WS", "NW-WE", "NW-S", "NW-B"],
        display_resolution: "none",
        icon_key: "waveform",
        capacity_models: &[],
    },
];

/// Resolves a detected (series, specific-model) pair from whatever identifying signals are
/// available. `product_id` (from USB enumeration) is tried first since it's unambiguous for any
/// series that has one; `model_token` (from a capability-XML scan or a USB product string) is
/// the fallback for series with no fixed PID. `capacity_gb`, if given, picks the specific model
/// within a capacity-differentiated series.
pub fn resolve_model(
    product_id: Option<u16>,
    model_token: Option<&str>,
    capacity_gb: Option<f64>,
) -> Option<(&'static CatalogEntry, String)> {
    let entry = product_id
        .and_then(|pid| CATALOG.iter().find(|e| e.product_ids.contains(&pid)))
        .or_else(|| {
            let token = model_token?.to_uppercase();
            CATALOG.iter().find(|e| e.model_tokens.iter().any(|t| token.contains(&t.to_uppercase())))
        })?;

    let specific = model_token
        .and_then(|token| {
            let upper = token.to_uppercase();
            entry.model_tokens.iter().find(|t| upper.contains(&t.to_uppercase())).copied()
        })
        .or_else(|| nearest_capacity_model(entry, capacity_gb))
        .unwrap_or(entry.series_name)
        .to_string();

    Some((entry, specific))
}

fn nearest_capacity_model(entry: &CatalogEntry, capacity_gb: Option<f64>) -> Option<&'static str> {
    let capacity = capacity_gb?;
    entry
        .capacity_models
        .iter()
        .min_by(|(a, _), (b, _)| (a - capacity).abs().total_cmp(&(b - capacity).abs()))
        .map(|(_, name)| *name)
}

pub fn is_supported_model(model_token: &str) -> bool {
    let upper = model_token.to_uppercase();
    CATALOG.iter().any(|e| e.model_tokens.iter().any(|t| upper.contains(&t.to_uppercase())))
}

pub fn is_android_model(model_token: &str) -> bool {
    let upper = model_token.to_uppercase();
    CATALOG
        .iter()
        .find(|e| e.model_tokens.iter().any(|t| upper.contains(&t.to_uppercase())))
        .map(|e| !e.is_non_android)
        .unwrap_or(false)
}

pub fn icon_key(model_token: &str) -> &'static str {
    let upper = model_token.to_uppercase();
    CATALOG
        .iter()
        .find(|e| e.model_tokens.iter().any(|t| upper.contains(&t.to_uppercase())))
        .map(|e| e.icon_key)
        .unwrap_or("questionmark.circle")
}

/// What [`inspect_volume`] found on a mounted drive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeSignature {
    /// Any of the Sony/Android signature files or folders (R3.2) was present.
    pub is_authentic: bool,
    pub drive_type: DriveType,
    /// A capability-XML `NW-XXXX`-shaped token, if `default-capability.xml` was present and
    /// contained one.
    pub model_token: Option<String>,
}

/// Looks for the signature files/folders R3.2 lists at `mount`'s root and classifies it as
/// Internal or MicroSD. `Sony_Card.IND` is the microSD-specific marker; its absence alongside
/// any other Sony signature is treated as Internal.
pub fn inspect_volume(mount: &Path) -> VolumeSignature {
    let has = |name: &str| mount.join(name).exists();
    let has_capability_xml = has("default-capability.xml") || has("DEFAULT-CAPABILITY.XML");
    let has_devicon = has("DevIcon.fil");
    let has_sony_card = has("Sony_Card.IND");
    let has_music_dir = has("MUSIC") || has("Music");
    let has_android_package = mount.join("Android/data").read_dir().is_ok_and(|mut entries| {
        entries.any(|e| {
            e.ok()
                .map(|e| e.file_name().to_string_lossy().to_lowercase().starts_with("com.sony.walkman"))
                .unwrap_or(false)
        })
    });

    let is_authentic = has_capability_xml || has_devicon || has_sony_card || has_music_dir || has_android_package;
    let drive_type = if has_sony_card { DriveType::Microsd } else { DriveType::Internal };
    let model_token = has_capability_xml.then(|| scan_for_model_token(mount)).flatten();

    VolumeSignature { is_authentic, drive_type, model_token }
}

/// Reads `default-capability.xml` (case-insensitive filename) and returns the first
/// `NW-`-prefixed token found anywhere in its contents, independent of the surrounding XML tag
/// structure (see the module doc for why).
fn scan_for_model_token(mount: &Path) -> Option<String> {
    let path = ["default-capability.xml", "DEFAULT-CAPABILITY.XML"]
        .iter()
        .map(|n| mount.join(n))
        .find(|p| p.exists())?;
    let content = std::fs::read_to_string(path).ok()?;
    find_nw_token(&content)
}

fn find_nw_token(content: &str) -> Option<String> {
    let bytes = content.as_bytes();
    let upper = content.to_uppercase();
    let idx = upper.find("NW-")?;
    let start = idx;
    let mut end = start;
    while end < bytes.len() {
        let c = bytes[end];
        if c.is_ascii_alphanumeric() || c == b'-' {
            end += 1;
        } else {
            break;
        }
    }
    Some(content[start..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_a_model_by_product_id_alone() {
        let (entry, specific) = resolve_model(Some(0x0E6E), None, None).unwrap();
        assert_eq!(entry.series_name, "NW-A300 Series");
        assert_eq!(specific, "NW-A300 Series"); // no token/capacity to narrow further
    }

    #[test]
    fn resolves_a_specific_model_by_capacity_within_a_pid_matched_series() {
        let (_, specific) = resolve_model(Some(0x0E6E), None, Some(63.5)).unwrap();
        assert_eq!(specific, "NW-A307");
    }

    #[test]
    fn resolves_by_model_token_when_no_pid_is_known() {
        let (entry, specific) = resolve_model(None, Some("WALKMAN NW-A55L"), None).unwrap();
        assert_eq!(entry.series_name, "NW-A50 Series");
        assert_eq!(specific, "NW-A55");
    }

    #[test]
    fn distinguishes_wm1am2_from_wm1zm2_by_pid_not_capacity() {
        let (_, a) = resolve_model(Some(0x0E70), None, None).unwrap();
        let (_, z) = resolve_model(Some(0x0E71), None, None).unwrap();
        assert_ne!(a, z);
    }

    #[test]
    fn unknown_product_and_token_resolves_to_none() {
        assert!(resolve_model(Some(0xFFFF), Some("Totally Unknown Device"), None).is_none());
    }

    #[test]
    fn is_supported_and_is_android_checks() {
        assert!(is_supported_model("NW-A105"));
        assert!(is_android_model("NW-A105"));
        assert!(is_supported_model("NW-WM1A"));
        assert!(!is_android_model("NW-WM1A"));
        assert!(!is_supported_model("Some Random MP3 Player"));
    }

    #[test]
    fn inspect_volume_detects_internal_drive_signature() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("MUSIC")).unwrap();
        std::fs::write(dir.path().join("DevIcon.fil"), b"").unwrap();

        let sig = inspect_volume(dir.path());
        assert!(sig.is_authentic);
        assert_eq!(sig.drive_type, DriveType::Internal);
    }

    #[test]
    fn inspect_volume_detects_microsd_via_sony_card_marker() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Sony_Card.IND"), b"").unwrap();

        let sig = inspect_volume(dir.path());
        assert!(sig.is_authentic);
        assert_eq!(sig.drive_type, DriveType::Microsd);
    }

    #[test]
    fn inspect_volume_reports_no_signature_for_an_unrelated_drive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("readme.txt"), b"hello").unwrap();

        let sig = inspect_volume(dir.path());
        assert!(!sig.is_authentic);
    }

    #[test]
    fn inspect_volume_scans_capability_xml_for_a_model_token() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("default-capability.xml"),
            b"<capability><device model=\"NW-A105\" firmware=\"2.00\"/></capability>",
        )
        .unwrap();

        let sig = inspect_volume(dir.path());
        assert!(sig.is_authentic);
        assert_eq!(sig.model_token, Some("NW-A105".to_string()));
    }

    #[test]
    fn find_nw_token_matches_case_insensitively_but_preserves_source_casing() {
        assert_eq!(find_nw_token("prefix nw-zx507l suffix"), Some("nw-zx507l".to_string()));
        assert_eq!(find_nw_token("no token here"), None);
    }
}
