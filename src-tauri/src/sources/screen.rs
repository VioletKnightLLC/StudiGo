//! Screen Capture Source via getDisplayMedia
//!
//! This module provides a ScreenCaptureSource that receives screen capture frames
//! from the frontend via Tauri commands and exposes them through the SourceBus trait.

use crate::source_bus::{FrameMetadata, SourceBus};
use std::sync::{Arc, Mutex};

/// Screen capture source configuration
#[derive(Debug, Clone)]
pub struct ScreenCaptureConfig {
    /// Source identifier
    pub source_id: String,
    /// Expected frame width
    pub width: u32,
    /// Expected frame height
    pub height: u32,
    /// Expected frame rate
    pub fps: f32,
}

impl Default for ScreenCaptureConfig {
    fn default() -> Self {
        ScreenCaptureConfig {
            source_id: "screen".to_string(),
            width: 1920,
            height: 1080,
            fps: 30.0,
        }
    }
}

/// Internal frame buffer state
#[derive(Debug, Clone, Default)]
pub struct FrameBuffer {
    pub data: Option<Vec<u8>>,
    pub width: u32,
    pub height: u32,
    pub timestamp_us: u64,
}

/// ScreenCaptureSource receives frames from the frontend via Tauri commands
pub struct ScreenCaptureSource {
    config: ScreenCaptureConfig,
    connected: bool,
    frame_buffer: Arc<Mutex<FrameBuffer>>,
}

impl ScreenCaptureSource {
    /// Create a new screen capture source with default configuration
    pub fn new() -> Self {
        ScreenCaptureSource {
            config: ScreenCaptureConfig::default(),
            connected: false,
            frame_buffer: Arc::new(Mutex::new(FrameBuffer::default())),
        }
    }

    /// Create a new screen capture source with custom configuration
    pub fn with_config(config: ScreenCaptureConfig) -> Self {
        ScreenCaptureSource {
            config,
            connected: false,
            frame_buffer: Arc::new(Mutex::new(FrameBuffer::default())),
        }
    }

    /// Get the frame buffer for passing to Tauri commands
    pub fn frame_buffer(&self) -> Arc<Mutex<FrameBuffer>> {
        Arc::clone(&self.frame_buffer)
    }

    /// Pop the current frame and clear the buffer
    fn pop_frame(&self) -> Option<Vec<u8>> {
        self.frame_buffer
            .lock()
            .ok()
            .and_then(|mut buf| buf.data.take())
    }
}

impl Default for ScreenCaptureSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceBus for ScreenCaptureSource {
    fn connect(&mut self) -> anyhow::Result<()> {
        self.connected = true;
        Ok(())
    }

    fn disconnect(&mut self) -> anyhow::Result<()> {
        self.connected = false;
        // Clear the frame buffer on disconnect
        if let Ok(mut buf) = self.frame_buffer.lock() {
            buf.data = None;
        }
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn next_frame(&mut self) -> anyhow::Result<Vec<u8>> {
        if !self.connected {
            return Err(anyhow::anyhow!("ScreenCaptureSource not connected"));
        }

        match self.pop_frame() {
            Some(data) => Ok(data),
            None => {
                // Return empty frame if no data available yet
                let size = (self.config.width * self.config.height * 3) as usize;
                Ok(vec![0u8; size])
            }
        }
    }

    fn frame_metadata(&self) -> Option<FrameMetadata> {
        Some(FrameMetadata::new(
            self.config.width,
            self.config.height,
            self.config.fps,
        ))
    }
}

/// Input frame data from frontend
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ScreenFrameInput {
    /// Raw RGB pixel data
    pub data: Vec<u8>,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_source_creation() {
        let source = ScreenCaptureSource::new();
        assert!(!source.is_connected());
    }

    #[test]
    fn test_screen_source_connect() {
        let mut source = ScreenCaptureSource::new();
        source.connect().unwrap();
        assert!(source.is_connected());
    }

    #[test]
    fn test_screen_source_disconnect() {
        let mut source = ScreenCaptureSource::new();
        source.connect().unwrap();
        source.disconnect().unwrap();
        assert!(!source.is_connected());
    }

    #[test]
    fn test_screen_source_next_frame_not_connected() {
        let mut source = ScreenCaptureSource::new();
        let result = source.next_frame();
        assert!(result.is_err());
    }

    #[test]
    fn test_screen_source_next_frame_connected() {
        let mut source = ScreenCaptureSource::new();
        source.connect().unwrap();

        let frame = source.next_frame().unwrap();
        // Should return empty/zero frame when no data from frontend
        assert!(!frame.is_empty());
    }

    #[test]
    fn test_screen_source_with_config() {
        let config = ScreenCaptureConfig {
            source_id: "test-screen".to_string(),
            width: 1280,
            height: 720,
            fps: 60.0,
        };
        let source = ScreenCaptureSource::with_config(config);

        let meta = source.frame_metadata().unwrap();
        assert_eq!(meta.width, 1280);
        assert_eq!(meta.height, 720);
        assert_eq!(meta.fps, 60.0);
    }
}
