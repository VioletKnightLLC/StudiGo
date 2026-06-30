//! Frame capture traits and implementations

use anyhow::Result;
use image::{ImageBuffer, Rgb};
use log::info;
use std::path::Path;

/// Frame dimensions for capture
pub const WIDTH: u32 = 1920;
pub const HEIGHT: u32 = 1080;

/// Trait for frame sources (real UVC or fake)
pub trait FrameSource {
    /// Get the next frame as RGB bytes
    fn next_frame(&mut self) -> Result<Vec<u8>>;
}

/// Real UVC source - attempts to capture from actual camera
pub struct RealUvcSource {
    // Note: Direct UVC capture on Windows requires complex setup with
    // either DirectShow, MF (Media Foundation), or libuvc.
    // For this spike, we attempt a basic approach.
}

impl RealUvcSource {
    pub fn new() -> Result<Self> {
        info!("Initializing real UVC source...");
        Ok(RealUvcSource {})
    }
}

impl FrameSource for RealUvcSource {
    fn next_frame(&mut self) -> Result<Vec<u8>> {
        // Real UVC capture would require complex Windows API integration
        // For now, this falls back to error - the fake source will be used
        anyhow::bail!("Real UVC capture not fully implemented - use FakeUvcSource")
    }
}

/// Capture frames from a source and write them as PNG files
pub fn capture_frames(source: &mut dyn FrameSource, output_dir: &Path, count: usize) -> Result<()> {
    info!("Capturing {} frames to {:?}", count, output_dir);

    // Ensure output directory exists
    std::fs::create_dir_all(output_dir)?;

    let frame_duration_ms = 1000 / 30; // 30 fps
    let mut frame_num = 0;

    while frame_num < count {
        let start = std::time::Instant::now();

        // Get frame data
        let frame_data = source.next_frame()?;

        // Create image from RGB data
        let img: ImageBuffer<Rgb<u8>, Vec<u8>> = 
            ImageBuffer::from_raw(WIDTH, HEIGHT, frame_data)
                .ok_or_else(|| anyhow::anyhow!("Failed to create image buffer"))?;

        // Write PNG
        let filename = output_dir.join(format!("frame_{:03}.png", frame_num));
        img.save(&filename)?;
        
        info!("Wrote frame {} to {:?}", frame_num, filename);
        frame_num += 1;

        // Frame timing for 30fps
        let elapsed = start.elapsed().as_millis() as u64;
        if elapsed < frame_duration_ms {
            std::thread::sleep(std::time::Duration::from_millis(frame_duration_ms - elapsed));
        }
    }

    Ok(())
}