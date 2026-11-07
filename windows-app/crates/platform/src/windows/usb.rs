//! [`UsbEnumerator`] for Windows (spec task 6.4).
//!
//! Enumerates present USB devices via `Get-PnpDevice` (PowerShell/CIM), which exposes each
//! device's PnP instance id (`USB\VID_054C&PID_0D00\<serial>`) and friendly name without the
//! unsafe SetupAPI device-info-set bookkeeping. The VID/PID filtering against the Sony table
//! lives in `usb_detector.rs` (spec task 9.2); this enumerator reports every USB device it sees.

use std::process::Command;

use crate::{UsbDevice, UsbEnumerator};

/// Real `UsbEnumerator` backed by `Get-PnpDevice`.
#[derive(Debug, Default)]
pub struct WindowsUsbEnumerator;

impl WindowsUsbEnumerator {
    pub fn new() -> Self {
        Self
    }
}

impl UsbEnumerator for WindowsUsbEnumerator {
    fn scan(&self) -> Vec<UsbDevice> {
        let output = Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-PnpDevice -PresentOnly | Where-Object { $_.InstanceId -like 'USB\\VID_*' } \
                 | ForEach-Object { \"$($_.InstanceId)|$($_.FriendlyName)\" }",
            ])
            .output();

        let Ok(output) = output else {
            log::warn!("usb scan: failed to spawn powershell");
            return Vec::new();
        };
        if !output.status.success() {
            log::warn!(
                "usb scan: Get-PnpDevice exited with {:?}",
                output.status.code()
            );
            return Vec::new();
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(parse_pnp_line)
            .collect()
    }
}

/// Parses one `InstanceId|FriendlyName` line, e.g.
/// `USB\VID_054C&PID_0D00\0123456789AB|NW-A105`.
fn parse_pnp_line(line: &str) -> Option<UsbDevice> {
    let (instance_id, friendly_name) = line.split_once('|').unwrap_or((line, ""));
    let mut segments = instance_id.split('\\');
    segments.next()?; // "USB"
    let ids = segments.next()?; // "VID_054C&PID_0D00&REV_0100"
    let serial = segments.next().map(str::to_string);

    let vendor_id = extract_hex_field(ids, "VID_")?;
    let product_id = extract_hex_field(ids, "PID_")?;
    let product_name = if friendly_name.trim().is_empty() {
        None
    } else {
        Some(friendly_name.trim().to_string())
    };

    Some(UsbDevice {
        vendor_id,
        product_id,
        serial: serial.filter(|s| !s.is_empty()),
        product_name,
    })
}

/// Extracts a 4-hex-digit field such as `VID_054C` out of an `&`-joined id string.
fn extract_hex_field(ids: &str, prefix: &str) -> Option<u16> {
    let start = ids.find(prefix)? + prefix.len();
    let rest = &ids[start..];
    let end = rest.find('&').unwrap_or(rest.len());
    u16::from_str_radix(&rest[..end], 16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_sony_walkman_instance_line() {
        let device = parse_pnp_line("USB\\VID_054C&PID_0D00\\0123456789AB|NW-A105").unwrap();
        assert_eq!(device.vendor_id, 0x054c);
        assert_eq!(device.product_id, 0x0d00);
        assert_eq!(device.serial, Some("0123456789AB".to_string()));
        assert_eq!(device.product_name, Some("NW-A105".to_string()));
    }

    #[test]
    fn parses_a_line_with_revision_suffix_and_no_friendly_name() {
        let device = parse_pnp_line("USB\\VID_054C&PID_0E6E&REV_0100\\SERIAL123|").unwrap();
        assert_eq!(device.vendor_id, 0x054c);
        assert_eq!(device.product_id, 0x0e6e);
        assert_eq!(device.product_name, None);
    }

    #[test]
    fn rejects_a_non_usb_malformed_line() {
        assert!(parse_pnp_line("not a pnp line").is_none());
    }
}
