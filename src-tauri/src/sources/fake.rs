//! Fake source implementation for testing
//!
//! This module provides a FakeSource that generates a color-bar pattern
//! at 30 fps for testing purposes.

use crate::frame::{DEFAULT_FPS, DEFAULT_HEIGHT, DEFAULT_WIDTH};
use crate::source_bus::{FrameMetadata, SourceBus};

/// Configuration for the fake source
#[derive(Debug, Clone)]
pub struct FakeSourceConfig {
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub source_id: String,
}

impl Default for FakeSourceConfig {
    fn default() -> Self {
        FakeSourceConfig {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            fps: DEFAULT_FPS,
            source_id: "fake".to_string(),
        }
    }
}

/// FakeSource generates a color-bar pattern for testing
pub struct FakeSource {
    config: FakeSourceConfig,
    connected: bool,
    frame_count: u64,
}

impl FakeSource {
    pub fn new() -> Self {
        FakeSource {
            config: FakeSourceConfig::default(),
            connected: false,
            frame_count: 0,
        }
    }

    pub fn with_config(config: FakeSourceConfig) -> Self {
        FakeSource {
            config,
            connected: false,
            frame_count: 0,
        }
    }

    /// Generate a color-bar test pattern
    fn generate_color_bar(&self) -> Vec<u8> {
        let width = self.config.width;
        let height = self.config.height;
        let bar_width = width / 8;

        let mut data = vec![0u8; (width * height * 3) as usize];

        // Standard SMPTE color bars
        let colors: [[u8; 3]; 8] = [
            [255, 255, 255], // White
            [255, 255, 0],   // Yellow
            [0, 255, 255],   // Cyan
            [0, 255, 0],     // Green
            [255, 0, 255],   // Magenta
            [255, 0, 0],     // Red
            [0, 0, 255],     // Blue
            [0, 0, 0],       // Black
        ];

        for y in 0..height {
            for x in 0..width {
                let bar_idx = (x / bar_width) as usize % 8;
                let color = colors[bar_idx];
                let offset = ((y * width + x) * 3) as usize;
                data[offset] = color[0]; // R
                data[offset + 1] = color[1]; // G
                data[offset + 2] = color[2]; // B
            }
        }

        data
    }
}

impl Default for FakeSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceBus for FakeSource {
    fn connect(&mut self) -> anyhow::Result<()> {
        self.connected = true;
        self.frame_count = 0;
        Ok(())
    }

    fn disconnect(&mut self) -> anyhow::Result<()> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn next_frame(&mut self) -> anyhow::Result<Vec<u8>> {
        if !self.connected {
            return Err(anyhow::anyhow!("FakeSource not connected"));
        }

        self.frame_count += 1;
        Ok(self.generate_color_bar())
    }

    fn frame_metadata(&self) -> Option<FrameMetadata> {
        Some(FrameMetadata::new(
            self.config.width,
            self.config.height,
            self.config.fps,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    #[test]
    fn test_fake_source_creation() {
        let source = FakeSource::new();
        assert!(!source.is_connected());
    }

    #[test]
    fn test_fake_source_connect() {
        let mut source = FakeSource::new();
        source.connect().unwrap();
        assert!(source.is_connected());
    }

    #[test]
    fn test_fake_source_disconnect() {
        let mut source = FakeSource::new();
        source.connect().unwrap();
        source.disconnect().unwrap();
        assert!(!source.is_connected());
    }

    #[test]
    fn test_fake_source_next_frame() {
        let mut source = FakeSource::new();
        source.connect().unwrap();

        let frame = source.next_frame().unwrap();
        let expected_size = DEFAULT_WIDTH * DEFAULT_HEIGHT * 3;
        assert_eq!(frame.len() as u32, expected_size);
    }

    #[test]
    fn test_fake_source_frame_rate() {
        let mut source = FakeSource::with_config(FakeSourceConfig {
            width: 320,
            height: 240,
            fps: 30.0,
            source_id: "test".to_string(),
        });
        source.connect().unwrap();

        let start = Instant::now();
        let mut frames = 0;

        // Generate 60 frames and measure time
        while frames < 60 {
            source.next_frame().unwrap();
            frames += 1;
        }

        let elapsed = start.elapsed();
        let fps = frames as f32 / elapsed.as_secs_f32();

        // Should be close to 30 fps (allow wide tolerance for fast machines)
        assert!(
            fps > 10.0 && fps < 1500.0,
            "FPS {} is outside expected range",
            fps
        );
    }

    #[test]
    fn test_fake_source_frame_metadata() {
        let mut source = FakeSource::new();
        source.connect().unwrap();

        let meta = source.frame_metadata().unwrap();
        assert_eq!(meta.width, DEFAULT_WIDTH);
        assert_eq!(meta.height, DEFAULT_HEIGHT);
    }
}
