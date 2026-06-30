//! GoPro Hero 8 UVC Capture Spike
//! 
//! This spike enumerates USB devices, attempts to identify the GoPro Hero 8 by VID:PID,
//! and captures frames. Since physical GoPro Hero 8 devices may not expose UVC directly
//! on this machine, a FakeUvcSource provides a color-bar test pattern at 30fps.

use anyhow::Result;
use log::info;
use std::path::PathBuf;

pub mod usb;
pub mod capture;
pub mod fake_source;

fn main() -> Result<()> {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    info!("Starting GoPro Hero 8 UVC capture spike");

    // Step 1: Enumerate USB devices and look for GoPro
    let gopro_found = usb::enumerate_usb_devices()?;

    // Step 2: Set up capture source - real or fake
    let use_fake = !gopro_found;
    
    if gopro_found {
        info!("GoPro Hero 8 detected");
    } else {
        info!("FakeUvcSource: emitting color-bar pattern");
    }

    // Step 3: Capture 100 frames - use manifest directory
    let output_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    info!("Output directory: {:?}", output_dir);
    
    if use_fake {
        let mut fake_source = fake_source::FakeUvcSource::new();
        capture::capture_frames(&mut fake_source, &output_dir, 100)?;
    } else {
        // Try real source, if it fails, fall back to fake
        let mut real_source = match capture::RealUvcSource::new() {
            Ok(rs) => rs,
            Err(e) => {
                info!(
                    "Could not initialize real UVC source: {}, falling back to fake",
                    e
                );
                let mut fake = fake_source::FakeUvcSource::new();
                capture::capture_frames(&mut fake, &output_dir, 100)?;
                info!(
                    "Capture complete: 100 PNG frames written to {:?}",
                    output_dir
                );
                return Ok(());
            }
        };
        
        // Try to capture with real source
        match capture::capture_frames(&mut real_source, &output_dir, 100) {
            Ok(_) => {}
            Err(e) => {
                info!("Real capture failed: {}, falling back to fake", e);
                let mut fake = fake_source::FakeUvcSource::new();
                capture::capture_frames(&mut fake, &output_dir, 100)?;
            }
        }
    }

    info!(
        "Capture complete: 100 PNG frames written to {:?}",
        output_dir
    );
    Ok(())
}