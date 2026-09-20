//! GoPro WiFi Streaming Source
//!
//! Connects to GoPro's built-in RTSP server over WiFi.
//! GoPro cameras expose an RTSP stream at rtsp://<camera-ip>:8554/live

use anyhow::Result;
use log::{info, warn};
use std::net::TcpStream;
use std::time::Duration;

use crate::source_bus::{FrameMetadata, SourceBus};

/// WiFi connection configuration
#[derive(Debug, Clone)]
pub struct WifiConfig {
    /// GoPro camera IP address (default: 10.5.5.1 for Hero 8+ when connected to camera WiFi)
    pub camera_ip: String,
    /// RTSP port (default: 8554)
    pub rtsp_port: u16,
    /// Stream path (default: /live)
    pub stream_path: String,
}

impl Default for WifiConfig {
    fn default() -> Self {
        WifiConfig {
            camera_ip: "10.5.5.1".to_string(),
            rtsp_port: 8554,
            stream_path: "/live".to_string(),
        }
    }
}

/// GoPro WiFi Streaming Source
pub struct WifiSource {
    config: WifiConfig,
    connected: bool,
}

impl WifiSource {
    /// Create a new WiFi source with default configuration
    pub fn new() -> Self {
        WifiSource {
            config: WifiConfig::default(),
            connected: false,
        }
    }

    /// Create a new WiFi source with custom configuration
    pub fn with_config(config: WifiConfig) -> Self {
        WifiSource {
            config,
            connected: false,
        }
    }

    /// Verify connection by attempting TCP connection to RTSP port
    fn verify_connection(&self) -> bool {
        let addr = format!("{}:{}", self.config.camera_ip, self.config.rtsp_port);

        match TcpStream::connect_timeout(
            &addr
                .parse()
                .unwrap_or_else(|_| panic!("Invalid address: {}", addr)),
            Duration::from_secs(5),
        ) {
            Ok(_stream) => {
                info!(
                    "TCP connection to {} successful - GoPro RTSP server reachable",
                    addr
                );
                true
            }
            Err(e) => {
                warn!("TCP connection to {} failed: {}", addr, e);
                false
            }
        }
    }
}

impl Default for WifiSource {
    fn default() -> Self {
        Self::new()
    }
}

impl SourceBus for WifiSource {
    fn connect(&mut self) -> Result<()> {
        let url = format!(
            "rtsp://{}:{}{}",
            self.config.camera_ip, self.config.rtsp_port, self.config.stream_path
        );

        info!("Attempting to connect to GoPro WiFi stream at {}", url);

        // TODO: Implement full RTSP handshake with the GoPro camera
        // For now, we verify the connection attempt by attempting a TCP connection
        // to the RTSP port

        let result = self.verify_connection();

        if result {
            self.connected = true;
            info!("WiFi source connected successfully");
            Ok(())
        } else {
            self.connected = false;
            anyhow::bail!(
                "Failed to connect to GoPro RTSP server at {}:{}",
                self.config.camera_ip,
                self.config.rtsp_port
            )
        }
    }

    fn disconnect(&mut self) -> Result<()> {
        info!("Disconnecting WiFi source");
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }

    fn next_frame(&mut self) -> Result<Vec<u8>> {
        if !self.connected {
            anyhow::bail!("Not connected to WiFi source. Call connect() first.");
        }

        // TODO: Implement actual frame reception from RTSP stream
        // For now, return empty frame data - this stub verifies connection only
        warn!("next_frame() called but RTSP frame parsing not yet implemented");
        Ok(vec![])
    }

    fn frame_metadata(&self) -> Option<FrameMetadata> {
        if self.connected {
            Some(FrameMetadata::new(1920, 1080, 30.0))
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wifi_source_creation() {
        let source = WifiSource::new();
        assert!(!source.is_connected());
    }

    #[test]
    fn test_wifi_source_with_config() {
        let config = WifiConfig {
            camera_ip: "192.168.1.100".to_string(),
            rtsp_port: 8554,
            stream_path: "/live".to_string(),
        };
        let source = WifiSource::with_config(config);
        assert!(!source.is_connected());
    }

    #[test]
    fn test_wifi_source_disconnect() {
        let mut source = WifiSource::new();
        // Disconnect should work even when not connected
        let result = source.disconnect();
        assert!(result.is_ok());
    }

    #[test]
    fn test_wifi_source_next_frame_not_connected() {
        let mut source = WifiSource::new();
        let result = source.next_frame();
        assert!(result.is_err());
    }
}
