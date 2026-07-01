//! GoPro UVC/USB Video Capture Source
//!
//! This module provides UVC (USB Video Class) capture for GoPro cameras
//! connected via USB. On Windows, it uses the MSMF (Microsoft Media Foundation)
//! backend through the nokhwa library.
//!
//! When a GoPro is connected in webcam mode, it appears as a standard UVC
//! camera device to the system.

use anyhow::{Context, Result};
use log::{info, warn};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType};
use rusb::UsbContext;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::source_bus::{FrameMetadata, SourceBus};

/// GoPro USB Vendor ID
const GOPRO_VID: u16 = 0x0ae4;

/// UVC Source configuration
#[derive(Debug, Clone)]
pub struct UvcConfig {
    /// Camera index (0 = first camera in system)
    pub camera_index: u32,
    /// Requested frame width
    pub width: u32,
    /// Requested frame height
    pub height: u32,
    /// Requested frames per second
    pub fps: u32,
    /// Source identifier for frames
    pub source_id: String,
}

impl Default for UvcConfig {
    fn default() -> Self {
        UvcConfig {
            camera_index: 0,
            width: 1920,
            height: 1080,
            fps: 30,
            source_id: "gopro-uvc".to_string(),
        }
    }
}

/// UVC Source for capturing from GoPro via USB
pub struct UvcSource {
    config: UvcConfig,
    connected: bool,
    camera: Option<nokhwa::Camera>,
    current_frame: Option<Vec<u8>>,
    current_metadata: Option<FrameMetadata>,
    stop_flag: Arc<AtomicBool>,
}

impl UvcSource {
    /// Create a new UVC source with default configuration
    pub fn new() -> Self {
        UvcSource {
            config: UvcConfig::default(),
            connected: false,
            camera: None,
            current_frame: None,
            current_metadata: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new UVC source with custom configuration
    pub fn with_config(config: UvcConfig) -> Self {
        UvcSource {
            config,
            connected: false,
            camera: None,
            current_frame: None,
            current_metadata: None,
            stop_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new UVC source targeting a specific camera index
    pub fn with_index(index: u32) -> Self {
        let config = UvcConfig {
            camera_index: index,
            ..Default::default()
        };
        Self::with_config(config)
    }

    /// Query available cameras in the system
    pub fn list_cameras() -> Result<Vec<String>> {
        info!("Querying available cameras...");
        let cameras = nokhwa::query(ApiBackend::Auto).context("Failed to query cameras")?;

        let names: Vec<String> = cameras
            .iter()
            .map(|cam| cam.human_name().to_string())
            .collect();

        info!("Found {} camera(s)", names.len());
        for (i, name) in names.iter().enumerate() {
            info!("  [{}] {}", i, name);
        }

        Ok(names)
    }

    /// Find GoPro camera by searching for known patterns in camera names
    pub fn find_gopro_camera() -> Result<Option<u32>> {
        let cameras = nokhwa::query(ApiBackend::Auto).context("Failed to query cameras")?;

        for cam in &cameras {
            let name = cam.human_name().to_lowercase();
            // Match common GoPro name patterns
            if name.contains("gopro") || name.contains("hero") {
                let index: u32 = match cam.index() {
                    CameraIndex::Index(i) => *i,
                    CameraIndex::String(_) => continue,
                };
                info!("Found GoPro camera at index {}", index);
                return Ok(Some(index));
            }
        }

        Ok(None)
    }

    /// Initialize the camera on Windows/MSMF
    fn init_camera(&mut self) -> Result<()> {
        let index = CameraIndex::Index(self.config.camera_index);

        // Request RGB format with absolute highest frame rate
        let requested =
            RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);

        info!(
            "Initializing camera at index {} with format {:?}",
            self.config.camera_index, requested
        );

        let camera = nokhwa::Camera::new(index, requested).context("Failed to create camera")?;

        self.camera = Some(camera);
        info!("Camera initialized successfully");

        Ok(())
    }

    /// Open the camera stream
    fn open_stream(&mut self) -> Result<()> {
        if let Some(ref mut camera) = self.camera {
            info!("Opening camera stream...");

            // Open with blocking frame capture
            camera
                .open_stream()
                .context("Failed to open camera stream")?;

            info!("Camera stream opened successfully");
            Ok(())
        } else {
            anyhow::bail!("Camera not initialized")
        }
    }
}

impl Default for UvcSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceBus for UvcSource {
    fn connect(&mut self) -> Result<()> {
        info!(
            "Connecting to UVC source (camera index {})...",
            self.config.camera_index
        );

        // Initialize and open camera
        self.init_camera()?;
        self.open_stream()?;

        self.connected = true;
        info!("UVC source connected successfully");

        Ok(())
    }

    fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting UVC source...");

        self.stop_flag.store(true, Ordering::SeqCst);

        // In nokhwa 0.10, don't need to explicitly close stream; dropping camera handles it
        self.camera = None;
        self.connected = false;
        self.current_frame = None;
        self.current_metadata = None;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn next_frame(&mut self) -> Result<Vec<u8>> {
        if !self.connected {
            return Err(anyhow::anyhow!(
                "UVC source not connected. Call connect() first."
            ));
        }

        // Capture frame from camera
        let frame = match &mut self.camera {
            Some(cam) => cam.frame().context("Failed to capture frame")?,
            None => return Err(anyhow::anyhow!("Camera not initialized")),
        };

        // Decode to RGB
        let decoded = frame
            .decode_image::<RgbFormat>()
            .context("Failed to decode frame to RGB")?;

        // Get frame metadata
        let width = decoded.width();
        let height = decoded.height();
        // Convert ImageBuffer to raw bytes - use as_raw() to get underlying Vec
        let data: Vec<u8> = decoded.into_raw();

        // Store current frame and metadata
        self.current_frame = Some(data.clone());
        self.current_metadata = Some(FrameMetadata::new(width, height, self.config.fps as f32));

        Ok(data)
    }

    fn frame_metadata(&self) -> Option<FrameMetadata> {
        self.current_metadata.clone()
    }
}

/// Enumerate USB devices and check for GoPro
/// Note: This uses rusb to detect GoPro devices at USB level
pub fn detect_gopro_usb() -> bool {
    info!("Enumerating USB devices for GoPro...");

    let ctx = match rusb::Context::new() {
        Ok(c) => c,
        Err(e) => {
            info!("Could not create USB context: {}", e);
            return false;
        }
    };

    let devices = match ctx.devices() {
        Ok(d) => d,
        Err(e) => {
            info!(
                "Could not enumerate USB devices (may need admin/permissions): {}",
                e
            );
            return false;
        }
    };

    for device in devices.iter() {
        let desc = match device.device_descriptor() {
            Ok(d) => d,
            Err(_) => continue,
        };

        let vid = desc.vendor_id();
        let pid = desc.product_id();

        if vid == GOPRO_VID {
            info!("Found GoPro device: VID={:04x} PID={:04x}", vid, pid);
            return true;
        }
    }

    info!("No GoPro detected via USB enumeration");
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uvc_source_creation() {
        let source = UvcSource::new();
        assert!(!source.is_connected());
    }

    #[test]
    fn test_uvc_source_with_config() {
        let config = UvcConfig {
            camera_index: 0,
            width: 640,
            height: 480,
            fps: 30,
            source_id: "test-uvc".to_string(),
        };
        let source = UvcSource::with_config(config);
        assert!(!source.is_connected());
    }

    #[test]
    fn test_uvc_source_with_index() {
        let source = UvcSource::with_index(1);
        assert!(!source.is_connected());
    }

    #[test]
    fn test_uvc_source_disconnect_not_connected() {
        let mut source = UvcSource::new();
        // Disconnect should work even when not connected
        let result = source.disconnect();
        assert!(result.is_ok());
    }

    #[test]
    fn test_uvc_source_next_frame_not_connected() {
        let mut source = UvcSource::new();
        let result = source.next_frame();
        assert!(result.is_err());
    }
}