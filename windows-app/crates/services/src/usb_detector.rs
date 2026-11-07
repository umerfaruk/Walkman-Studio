//! Publishes connected Sony Walkmans from raw USB enumeration (spec task 9.2).
//!
//! Thin layer over [`walkman_platform::UsbEnumerator`]: filters to Sony's vendor id and resolves
//! each matching device against the [`crate::device_detector`] catalog, refreshed by the caller
//! on a [`walkman_platform::DeviceEvents`] notification plus the periodic safety poll (R3.6) —
//! this module itself is a pure `scan() -> Vec<_>` snapshot with no polling of its own.

use walkman_platform::{UsbDevice, UsbEnumerator};

use crate::device_detector::{resolve_model, CatalogEntry, SONY_VENDOR_ID};

/// A currently-connected USB device recognized as a Sony Walkman.
#[derive(Debug, Clone)]
pub struct ConnectedUsbWalkman {
    pub vendor_id: u16,
    pub product_id: u16,
    pub serial: Option<String>,
    pub product_name: Option<String>,
    pub catalog_entry: &'static CatalogEntry,
    pub specific_model: String,
}

/// Enumerates USB devices via `usb` and resolves every Sony-vendor-id device against the
/// catalog. Devices with Sony's vendor id but an unrecognized product id/string (R3.4,
/// "unsupported model") are intentionally omitted here — the caller surfaces those separately
/// so the UI can still flag "Sony device detected but unsupported" rather than silently
/// dropping it; see [`scan_unsupported`].
pub fn scan_connected_walkmans(usb: &dyn UsbEnumerator) -> Vec<ConnectedUsbWalkman> {
    usb.scan()
        .into_iter()
        .filter(|d| d.vendor_id == SONY_VENDOR_ID)
        .filter_map(|d| {
            let (entry, specific_model) = resolve_model(Some(d.product_id), d.product_name.as_deref(), None)?;
            Some(ConnectedUsbWalkman {
                vendor_id: d.vendor_id,
                product_id: d.product_id,
                serial: d.serial,
                product_name: d.product_name,
                catalog_entry: entry,
                specific_model,
            })
        })
        .collect()
}

/// Every Sony-vendor-id USB device that did *not* resolve to a catalog entry (R3.4).
pub fn scan_unsupported(usb: &dyn UsbEnumerator) -> Vec<UsbDevice> {
    usb.scan()
        .into_iter()
        .filter(|d| d.vendor_id == SONY_VENDOR_ID)
        .filter(|d| resolve_model(Some(d.product_id), d.product_name.as_deref(), None).is_none())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use walkman_platform::fake::FakeUsbEnumerator;

    fn device(vendor_id: u16, product_id: u16, product_name: Option<&str>) -> UsbDevice {
        UsbDevice {
            vendor_id,
            product_id,
            serial: Some("SERIAL123".into()),
            product_name: product_name.map(str::to_string),
        }
    }

    #[test]
    fn finds_a_recognized_sony_walkman_and_ignores_other_vendors() {
        let usb = FakeUsbEnumerator::new();
        usb.set_devices(vec![
            device(SONY_VENDOR_ID, 0x0D00, Some("NW-A105")), // A100 series
            device(0x1234, 0x5678, Some("Some Other Vendor's Gadget")),
        ]);

        let found = scan_connected_walkmans(&usb);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].specific_model, "NW-A105");
        assert_eq!(found[0].catalog_entry.series_name, "NW-A100 Series");
    }

    #[test]
    fn reports_a_sony_device_with_an_unrecognized_pid_as_unsupported_not_dropped() {
        let usb = FakeUsbEnumerator::new();
        usb.set_devices(vec![device(SONY_VENDOR_ID, 0xBEEF, Some("Future Walkman"))]);

        assert!(scan_connected_walkmans(&usb).is_empty());
        let unsupported = scan_unsupported(&usb);
        assert_eq!(unsupported.len(), 1);
        assert_eq!(unsupported[0].product_id, 0xBEEF);
    }

    #[test]
    fn no_devices_connected_returns_empty() {
        let usb = FakeUsbEnumerator::new();
        assert!(scan_connected_walkmans(&usb).is_empty());
        assert!(scan_unsupported(&usb).is_empty());
    }
}
