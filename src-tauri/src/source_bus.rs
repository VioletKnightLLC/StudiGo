//! SourceBus trait definition
//!
//! This module defines the core trait for all video/audio sources in the application.
//! All source implementations (WiFi, Bluetooth, UVC, Screen) must implement this trait.

use anyhow::Result;

/// Frame metadata
#[derive(Debug, Clone)]
pub struct FrameMetadata {
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Frame timestamp (microseconds since epoch)
    pub timestamp_us: u64,
    /// Frame rate (frames per second)
    pub fps: f32,
}

impl FrameMetadata {
    pub fn new(width: u32, height: u32, fps: f32) -> Self {
        FrameMetadata {
            width,
            height,
            timestamp_us: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_micros() as u64)
                .unwrap_or(0),
            fps,
        }
    }
}

/// SourceBus trait - the core interface for all video/audio sources
pub trait SourceBus {
    /// Connect to the source and begin streaming
    fn connect(&mut self) -> Result<()>;

    /// Disconnect from the source
    fn disconnect(&mut self) -> Result<()>;

    /// Check if currently connected
    fn is_connected(&self) -> bool;

    /// Get the next frame
    /// Returns the frame data as RGB bytes
    fn next_frame(&mut self) -> Result<Vec<u8>>;

    /// Get frame metadata for the current frame
    fn frame_metadata(&self) -> Option<FrameMetadata>;
}

/// Source error types
#[derive(Debug, thiserror::Error)]
pub enum SourceError {
    #[error("Source not connected")]
    NotConnected,

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Frame capture failed: {0}")]
    CaptureFailed(String),

    #[error("Source disconnected unexpectedly")]
    Disconnected,
}

pub mod sealed {
    use super::SourceBus;

    /// Helper to allow dynamic dispatch on source implementations
    pub trait SourceBusDyn: SourceBus {}
    impl<T: SourceBus> SourceBusDyn for T {}
}
