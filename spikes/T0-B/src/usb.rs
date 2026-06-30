//! USB device enumeration module

use anyhow::Result;
use log::info;
use rusb::UsbContext;

const GOPRO_VID: u16 = 0x0ae4;
const GOPRO_PID_HERO8: u16 = 0x0f01;

/// Enumerate USB devices and check for GoPro Hero 8
pub fn enumerate_usb_devices() -> Result<bool> {
    info!("Enumerating USB devices...");

    // Use rusb to enumerate
    let ctx = match rusb::Context::new() {
        Ok(c) => c,
        Err(e) => {
            info!("Could not create USB context: {}", e);
            return Ok(false);
        }
    };

    let devices = match ctx.devices() {
        Ok(d) => d,
        Err(e) => {
            info!(
                "Could not enumerate USB devices (may need admin/permission): {}",
                e
            );
            return Ok(false);
        }
    };

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };
        
        let vid = desc.vendor_id();
        let pid = desc.product_id();
        
        info!("USB device: {:04x}:{:04x}", vid, pid);
        
        // Check for GoPro (various PIDs)
        if vid == GOPRO_VID {
            info!("  -> GoPro device found! PID: {:04x}", pid);
            // Hero 8 in UVC mode typically has different PID
            if pid == GOPRO_PID_HERO8 {
                return Ok(true);
            }
        }
    }

    info!("No GoPro Hero 8 detected via USB enumeration");
    Ok(false)
}