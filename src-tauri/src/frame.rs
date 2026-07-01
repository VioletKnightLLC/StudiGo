//! Frame type definitions
//!
//! This module defines the Frame type used throughout the application for
//! representing video frames from various sources.

use serde::{Deserialize, Serialize};

/// Frame buffer containing RGB pixel data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Frame {
    /// Raw RGB pixel data (width * height * 3 bytes)
    pub data: Vec<u8>,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Frame timestamp in microseconds since UNIX epoch
    pub timestamp_us: u64,
    /// Source identifier (e.g., "gopro-uvc", "screen", "wifi")
    pub source_id: String,
}

impl Frame {
    /// Create a new frame with the given dimensions
    pub fn new(width: u32, height: u32, source_id: &str) -> Self {
        let data = vec![0u8; (width * height * 3) as usize];
        let timestamp_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        
        Frame {
            data,
            width,
            height,
            timestamp_us,
            source_id: source_id.to_string(),
        }
    }

    /// Create a frame from existing data
    pub fn from_data(data: Vec<u8>, width: u32, height: u32, source_id: &str) -> Self {
        let timestamp_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        
        assert_eq!(
            data.len() as u32,
            width * height * 3,
            "Frame data size must match dimensions"
        );
        
        Frame {
            data,
            width,
            height,
            timestamp_us,
            source_id: source_id.to_string(),
        }
    }

    /// Get the frame size in bytes
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Get frame rate based on timestamp difference from another frame
    pub fn fps(&self, prev: &Frame) -> f32 {
        let delta_us = self.timestamp_us.saturating_sub(prev.timestamp_us);
        if delta_us == 0 {
            return 0.0;
        }
        1_000_000.0 / delta_us as f32
    }
}

/// Default frame resolution
pub const DEFAULT_WIDTH: u32 = 1920;
pub const DEFAULT_HEIGHT: u32 = 1080;
pub const DEFAULT_FPS: f32 = 30.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_creation() {
        let frame = Frame::new(640, 480, "test");
        assert_eq!(frame.width, 640);
        assert_eq!(frame.height, 480);
        assert_eq!(frame.data.len(), 640 * 480 * 3);
    }

    #[test]
    fn test_frame_from_data() {
        let data = vec![0u8; 75]; // 5*5*3
        let frame = Frame::from_data(data.clone(), 5, 5, "test");
        assert_eq!(frame.data.len(), 75);
    }
}