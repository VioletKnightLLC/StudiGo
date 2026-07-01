//! Virtual Camera Output via DirectShow
//!
//! This module implements a virtual camera filter that exposes composited frames
//! as a webcam using the Windows DirectShow API. The virtual camera appears as
//! "GoPro Webcam Studio" in applications like OBS, Zoom, and Teams.
//!
//! Note: This is a Rust wrapper for the virtual camera functionality. The actual
//! DirectShow filter DLL would need to be built separately and registered with Windows.

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

/// Virtual camera filter CLSID
/// {B7A0C123-8D4E-4F3A-9B5E-6C8D7E2F1A0B}
pub const CLSID_GOPRO_CAM_FILTER: &str = "b7a0c123-8d4e-4f3a-9b5e-6c8d7e2f1a0b";

/// Virtual camera name as it appears in applications
pub const VIRTUAL_CAM_NAME: &str = "GoPro Webcam Studio";

/// Default frame dimensions
pub const DEFAULT_WIDTH: u32 = 1920;
pub const DEFAULT_HEIGHT: u32 = 1080;
pub const DEFAULT_FPS: u32 = 30;

/// Virtual camera error types
#[derive(Debug, thiserror::Error)]
pub enum VirtualCamError {
    #[error("Filter not initialized")]
    NotInitialized,
    
    #[error("Filter already running")]
    AlreadyRunning,
    
    #[error("Filter not running")]
    NotRunning,
    
    #[error("Failed to acquire lock: {0}")]
    LockError(String),
    
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
    
    #[error("COM registration error: {0}")]
    RegistrationError(String),
    
    #[error("Frame buffer error: {0}")]
    BufferError(String),
}

/// Video format configuration
#[derive(Debug, Clone)]
pub struct VideoFormat {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub media_type: MediaType,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MediaType {
    RGB24,
    YUV422,
    MJPEG,
}

impl Default for VideoFormat {
    fn default() -> Self {
        VideoFormat {
            width: DEFAULT_WIDTH,
            height: DEFAULT_HEIGHT,
            fps: DEFAULT_FPS,
            media_type: MediaType::RGB24,
        }
    }
}

/// Frame data for the virtual camera
#[derive(Debug, Clone)]
pub struct VirtualCamFrame {
    /// Raw pixel data
    pub data: Vec<u8>,
    /// Frame width
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Timestamp in microseconds
    pub timestamp_us: u64,
}

impl VirtualCamFrame {
    /// Create a new frame with the given dimensions
    pub fn new(width: u32, height: u32) -> Self {
        let data = vec![0u8; (width * height * 3) as usize];
        let timestamp_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        
        VirtualCamFrame {
            data,
            width,
            height,
            timestamp_us,
        }
    }
    
    /// Create a frame from existing data
    pub fn from_data(data: Vec<u8>, width: u32, height: u32) -> Self {
        let timestamp_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);
        
        VirtualCamFrame {
            data,
            width,
            height,
            timestamp_us,
        }
    }
}

/// IAMStreamConfig implementation for video format configuration
/// 
/// This trait defines the interface for getting and setting video stream format
pub trait IAMStreamConfig {
    /// Get the current media format
    fn get_format(&self) -> Result<VideoFormat, VirtualCamError>;
    
    /// Set the media format
    fn set_format(&self, format: VideoFormat) -> Result<(), VirtualCamError>;
    
    /// Get the number of supported format capabilities
    fn get_capabilities_count(&self) -> Result<usize, VirtualCamError>;
    
    /// Get a specific capability by index
    fn get_capability(&self, index: usize) -> Result<VideoFormat, VirtualCamError>;
}

/// IAMVideoProcAmp implementation for video processing
/// 
/// This trait defines the interface for video processing like brightness, contrast, etc.
pub trait IAMVideoProcAmp {
    /// Get the range for a given property
    fn get_range(&self, property: VideoProcAmpProperty) -> Result<(i32, i32, i32, i32), VirtualCamError>;
    
    /// Get the current value of a property
    fn get(&self, property: VideoProcAmpProperty) -> Result<i32, VirtualCamError>;
    
    /// Set a property value
    fn set(&self, property: VideoProcAmpProperty, value: i32, flags: VideoProcAmpFlags) -> Result<(), VirtualCamError>;
}

/// Video processing amplifier properties
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VideoProcAmpProperty {
    Brightness,
    Contrast,
    Hue,
    Saturation,
    Sharpness,
    Gamma,
    ColorEnable,
    WhiteBalance,
    BacklightCompensation,
}

impl Default for VideoProcAmpProperty {
    fn default() -> Self {
        VideoProcAmpProperty::Brightness
    }
}

/// Video processing flags
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VideoProcAmpFlags {
    Auto,
    Manual,
}

impl Default for VideoProcAmpFlags {
    fn default() -> Self {
        VideoProcAmpFlags::Manual
    }
}

/// IKsPropertySet implementation for property handling
/// 
/// This trait defines the interface for DirectShow property sets
pub trait IKsPropertySet {
    /// Set a property
    fn set(
        &self,
        guid: &str,
        prop_id: u32,
        data: &[u8],
    ) -> Result<(), VirtualCamError>;
    
    /// Get a property
    fn get(
        &self,
        guid: &str,
        prop_id: u32,
        data: &mut [u8],
    ) -> Result<usize, VirtualCamError>;
    
    /// Query if a property is supported
    fn query_supported(
        &self,
        guid: &str,
        prop_id: u32,
    ) -> Result<bool, VirtualCamError>;
}

/// Filter state
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterState {
    Stopped,
    Paused,
    Running,
}

impl Default for FilterState {
    fn default() -> Self {
        FilterState::Stopped
    }
}

/// Virtual camera filter implementation
/// 
/// This struct implements the DirectShow filter interface including:
/// - IBaseFilter: Base filter functionality
/// - IAMStreamConfig: Stream format configuration
/// - IAMVideoProcAmp: Video processing
/// - IKsPropertySet: Property handling
pub struct VirtualCamFilter {
    /// Filter state
    state: Mutex<FilterState>,
    /// Current video format
    format: Mutex<VideoFormat>,
    /// Frame buffer for delivering frames
    frame_buffer: Mutex<Option<VirtualCamFrame>>,
    /// Video processing settings
    video_proc_amp: Mutex<HashMap<VideoProcAmpProperty, i32>>,
    /// Reference count
    ref_count: Mutex<i32>,
    /// Filter name
    name: String,
}

impl VirtualCamFilter {
    /// Create a new virtual camera filter
    pub fn new() -> Self {
        let mut video_proc_amp = HashMap::new();
        video_proc_amp.insert(VideoProcAmpProperty::Brightness, 128);
        video_proc_amp.insert(VideoProcAmpProperty::Contrast, 128);
        video_proc_amp.insert(VideoProcAmpProperty::Saturation, 128);
        video_proc_amp.insert(VideoProcAmpProperty::Hue, 0);
        video_proc_amp.insert(VideoProcAmpProperty::Sharpness, 0);
        video_proc_amp.insert(VideoProcAmpProperty::Gamma, 100);
        
        VirtualCamFilter {
            state: Mutex::new(FilterState::Stopped),
            format: Mutex::new(VideoFormat::default()),
            frame_buffer: Mutex::new(None),
            video_proc_amp: Mutex::new(video_proc_amp),
            ref_count: Mutex::new(1),
            name: VIRTUAL_CAM_NAME.to_string(),
        }
    }
    
    /// Create a new filter with specific format
    pub fn with_format(format: VideoFormat) -> Self {
        let mut filter = Self::new();
        *filter.format.lock().unwrap() = format;
        filter
    }
    
    /// Get the filter name
    pub fn name(&self) -> &str {
        &self.name
    }
    
    /// Get current filter state
    pub fn get_state(&self) -> FilterState {
        *self.state.lock().unwrap()
    }
    
    /// Start the filter
    pub fn start(&self) -> Result<(), VirtualCamError> {
        let mut state = self.state.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        
        if *state == FilterState::Running {
            return Err(VirtualCamError::AlreadyRunning);
        }
        
        *state = FilterState::Running;
        Ok(())
    }
    
    /// Stop the filter
    pub fn stop(&self) -> Result<(), VirtualCamError> {
        let mut state = self.state.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        
        if *state == FilterState::Stopped {
            return Err(VirtualCamError::NotRunning);
        }
        
        *state = FilterState::Stopped;
        Ok(())
    }
    
    /// Pause the filter
    pub fn pause(&self) -> Result<(), VirtualCamError> {
        let mut state = self.state.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        
        if *state == FilterState::Stopped {
            *state = FilterState::Paused;
        }
        
        Ok(())
    }
    
    /// Push a frame to the filter
        pub fn push_frame(&self, frame: VirtualCamFrame) -> Result<(), VirtualCamError> {
            let mut buffer = self
                .frame_buffer
                .lock()
                .map_err(|e| VirtualCamError::LockError(e.to_string()))?;
            let format = self
                .format
                .lock()
                .map_err(|e| VirtualCamError::LockError(e.to_string()))?;

            // Verify frame dimensions match
            if frame.width != format.width || frame.height != format.height {
                return Err(VirtualCamError::InvalidFormat(format!(
                    "Frame dimensions {}x{} do not match filter format {}x{}",
                    frame.width, frame.height, format.width, format.height
                )));
            }

            *buffer = Some(frame);
                    Ok(())
                }

                /// Pop the current frame (for delivery to the DirectShow pin)
                pub fn pop_frame(&self) -> Option<VirtualCamFrame> {
                    self.frame_buffer.lock().ok()?.take()
                }

                /// Set the video format directly
        pub fn set_format(&self, format: VideoFormat) -> Result<(), VirtualCamError> {
            if format.width == 0 || format.height == 0 {
                return Err(VirtualCamError::InvalidFormat("Invalid dimensions".to_string()));
            }
            let mut current = self
                .format
                .lock()
                .map_err(|e| VirtualCamError::LockError(e.to_string()))?;
            *current = format;
            Ok(())
        }
    
    /// Check if a frame is available
    pub fn has_frame(&self) -> bool {
        self.frame_buffer.lock().map(|b| b.is_some()).unwrap_or(false)
    }
    
    // IAMStreamConfig implementation
    pub fn iam_stream_config(&self) -> VirtualCamIAMStreamConfig {
        VirtualCamIAMStreamConfig {
            filter: self,
        }
    }
    
    // IAMVideoProcAmp implementation
    pub fn iam_video_proc_amp(&self) -> VirtualCamIAMVideoProcAmp {
        VirtualCamIAMVideoProcAmp {
            filter: self,
        }
    }
    
    // IKsPropertySet implementation
    pub fn iks_property_set(&self) -> VirtualCamIKsPropertySet {
        VirtualCamIKsPropertySet {
            filter: self,
        }
    }
}

impl Default for VirtualCamFilter {
    fn default() -> Self {
        Self::new()
    }
}

/// IAMStreamConfig wrapper for VirtualCamFilter
pub struct VirtualCamIAMStreamConfig<'a> {
    filter: &'a VirtualCamFilter,
}

impl<'a> IAMStreamConfig for VirtualCamIAMStreamConfig<'a> {
    fn get_format(&self) -> Result<VideoFormat, VirtualCamError> {
        let format = self.filter.format.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        Ok(format.clone())
    }
    
    fn set_format(&self, format: VideoFormat) -> Result<(), VirtualCamError> {
        let mut current = self.filter.format.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        
        // Validate format
        if format.width == 0 || format.height == 0 {
            return Err(VirtualCamError::InvalidFormat("Invalid dimensions".to_string()));
        }
        
        *current = format;
        Ok(())
    }
    
    fn get_capabilities_count(&self) -> Result<usize, VirtualCamError> {
        // Support multiple resolutions
        Ok(4) // 640x480, 1280x720, 1920x1080, 2560x1440
    }
    
    fn get_capability(&self, index: usize) -> Result<VideoFormat, VirtualCamError> {
        let capabilities = [
            VideoFormat { width: 640, height: 480, fps: 30, media_type: MediaType::RGB24 },
            VideoFormat { width: 1280, height: 720, fps: 30, media_type: MediaType::RGB24 },
            VideoFormat { width: 1920, height: 1080, fps: 30, media_type: MediaType::RGB24 },
            VideoFormat { width: 2560, height: 1440, fps: 30, media_type: MediaType::RGB24 },
        ];
        
        capabilities
            .get(index)
            .cloned()
            .ok_or_else(|| VirtualCamError::InvalidFormat("Invalid capability index".to_string()))
    }
}

/// IAMVideoProcAmp wrapper for VirtualCamFilter
pub struct VirtualCamIAMVideoProcAmp<'a> {
    filter: &'a VirtualCamFilter,
}

impl<'a> IAMVideoProcAmp for VirtualCamIAMVideoProcAmp<'a> {
    fn get_range(&self, property: VideoProcAmpProperty) -> Result<(i32, i32, i32, i32), VirtualCamError> {
        // Returns (min, max, default, step)
        match property {
            VideoProcAmpProperty::Brightness |
            VideoProcAmpProperty::Contrast |
            VideoProcAmpProperty::Saturation => {
                Ok((0, 255, 128, 1))
            }
            VideoProcAmpProperty::Hue => {
                Ok((-180, 180, 0, 1))
            }
            VideoProcAmpProperty::Sharpness |
            VideoProcAmpProperty::Gamma => {
                Ok((0, 100, 50, 1))
            }
            _ => Ok((0, 100, 50, 1)),
        }
    }
    
    fn get(&self, property: VideoProcAmpProperty) -> Result<i32, VirtualCamError> {
        let amp = self.filter.video_proc_amp.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        
        amp.get(&property)
            .copied()
            .ok_or_else(|| VirtualCamError::InvalidFormat("Property not found".to_string()))
    }
    
    fn set(&self, property: VideoProcAmpProperty, value: i32, _flags: VideoProcAmpFlags) -> Result<(), VirtualCamError> {
        let (min, max, _, _) = self.get_range(property)?;
        
        if value < min || value > max {
            return Err(VirtualCamError::InvalidFormat(format!(
                "Value {} out of range [{}, {}]",
                value, min, max
            )));
        }
        
        let mut amp = self.filter.video_proc_amp.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        amp.insert(property, value);
        
        Ok(())
    }
}

/// IKsPropertySet wrapper for VirtualCamFilter
pub struct VirtualCamIKsPropertySet<'a> {
    filter: &'a VirtualCamFilter,
}

impl<'a> IKsPropertySet for VirtualCamIKsPropertySet<'a> {
    fn set(&self, _guid: &str, _prop_id: u32, _data: &[u8]) -> Result<(), VirtualCamError> {
        // Property set implementation
        Ok(())
    }
    
    fn get(&self, _guid: &str, _prop_id: u32, _data: &mut [u8]) -> Result<usize, VirtualCamError> {
        // Property get implementation
        Ok(0)
    }
    
    fn query_supported(&self, _guid: &str, _prop_id: u32) -> Result<bool, VirtualCamError> {
        Ok(false)
    }
}

/// Virtual camera manager for controlling multiple virtual cameras
pub struct VirtualCamManager {
    /// Active filters
    filters: Mutex<HashMap<String, Arc<VirtualCamFilter>>>,
    /// Registration state
    registered: Mutex<bool>,
}

impl VirtualCamManager {
    /// Create a new virtual camera manager
    pub fn new() -> Self {
        VirtualCamManager {
            filters: Mutex::new(HashMap::new()),
            registered: Mutex::new(false),
        }
    }
    
    /// Register the virtual camera with Windows
    #[cfg(target_os = "windows")]
    pub fn register(&self) -> Result<(), VirtualCamError> {
        let mut registered = self.registered.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        
        if *registered {
            return Ok(()); // Already registered
        }
        
        // In a real implementation, this would:
        // 1. Load the DirectShow filter DLL
        // 2. Call RegisterFilter from the DLL
        // 3. Add to the global filter category
        
        *registered = true;
        Ok(())
    }
    
    /// Unregister the virtual camera from Windows
    #[cfg(target_os = "windows")]
    pub fn unregister(&self) -> Result<(), VirtualCamError> {
        let mut registered = self.registered.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        
        if !*registered {
            return Ok(()); // Not registered
        }
        
        // In a real implementation, this would:
        // 1. Call UnregisterFilter from the DLL
        // 2. Remove from the global filter category
        
        *registered = false;
        Ok(())
    }
    
    /// Check if the virtual camera is registered
    pub fn is_registered(&self) -> bool {
        self.registered.lock().map(|r| *r).unwrap_or(false)
    }
    
    /// Create or get a named filter
    pub fn get_or_create_filter(&self, name: &str) -> Arc<VirtualCamFilter> {
        let mut filters = self.filters.lock().unwrap();
        
        if let Some(filter) = filters.get(name) {
            return Arc::clone(filter);
        }
        
        let filter = Arc::new(VirtualCamFilter::new());
        filters.insert(name.to_string(), Arc::clone(&filter));
        filter
    }
    
    /// Get the default GoPro Webcam Studio filter
    pub fn get_default_filter(&self) -> Arc<VirtualCamFilter> {
        self.get_or_create_filter(VIRTUAL_CAM_NAME)
    }
    
    /// Remove a filter by name
    pub fn remove_filter(&self, name: &str) -> Result<(), VirtualCamError> {
        let mut filters = self.filters.lock().map_err(|e| VirtualCamError::LockError(e.to_string()))?;
        filters.remove(name);
        Ok(())
    }
}

impl Default for VirtualCamManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_virtual_cam_filter_creation() {
        let filter = VirtualCamFilter::new();
        assert_eq!(filter.name(), VIRTUAL_CAM_NAME);
        assert_eq!(filter.get_state(), FilterState::Stopped);
    }
    
    #[test]
    fn test_virtual_cam_filter_with_format() {
        let format = VideoFormat {
            width: 1280,
            height: 720,
            fps: 60,
            media_type: MediaType::RGB24,
        };
        let filter = VirtualCamFilter::with_format(format);
        
        let iam = filter.iam_stream_config();
        let current = iam.get_format().unwrap();
        assert_eq!(current.width, 1280);
        assert_eq!(current.height, 720);
    }
    
    #[test]
    fn test_virtual_cam_frame_push_pop() {
        let filter = VirtualCamFilter::new();
        
        let frame = VirtualCamFrame::new(1920, 1080);
        filter.push_frame(frame).unwrap();
        
        assert!(filter.has_frame());
        
        let frame = filter.pop_frame();
        assert!(frame.is_some());
        assert!(!filter.has_frame());
    }
    
    #[test]
    fn test_iam_stream_config() {
        let filter = VirtualCamFilter::new();
        let iam = filter.iam_stream_config();
        
        // Test get format
        let format = iam.get_format().unwrap();
        assert_eq!(format.width, DEFAULT_WIDTH);
        assert_eq!(format.height, DEFAULT_HEIGHT);
        
        // Test set format
        let new_format = VideoFormat {
            width: 1280,
            height: 720,
            fps: 30,
            media_type: MediaType::RGB24,
        };
        iam.set_format(new_format.clone()).unwrap();
        
        let current = iam.get_format().unwrap();
        assert_eq!(current.width, 1280);
        assert_eq!(current.height, 720);
        
        // Test capabilities
        let count = iam.get_capabilities_count().unwrap();
        assert!(count > 0);
        
        let cap = iam.get_capability(0).unwrap();
        assert_eq!(cap.width, 640);
    }
    
    #[test]
    fn test_iam_video_proc_amp() {
        let filter = VirtualCamFilter::new();
        let iam = filter.iam_video_proc_amp();
        
        // Test get range
        let (min, max, default, step) = iam.get_range(VideoProcAmpProperty::Brightness).unwrap();
        assert_eq!(min, 0);
        assert_eq!(max, 255);
        assert_eq!(default, 128);
        
        // Test get and set
        let brightness = iam.get(VideoProcAmpProperty::Brightness).unwrap();
        assert_eq!(brightness, 128);
        
        iam.set(VideoProcAmpProperty::Brightness, 100, VideoProcAmpFlags::Manual).unwrap();
        
        let brightness = iam.get(VideoProcAmpProperty::Brightness).unwrap();
        assert_eq!(brightness, 100);
    }
    
    #[test]
    fn test_virtual_cam_manager() {
        let manager = VirtualCamManager::new();
        
        // Get default filter
        let filter = manager.get_default_filter();
        assert_eq!(filter.name(), VIRTUAL_CAM_NAME);
        
        // Get same filter again
        let filter2 = manager.get_default_filter();
        assert!(Arc::ptr_eq(&filter, &filter2));
    }
    
    #[test]
    fn test_frame_dimensions_mismatch() {
        let format = VideoFormat {
            width: 1280,
            height: 720,
            fps: 30,
            media_type: MediaType::RGB24,
        };
        let filter = VirtualCamFilter::with_format(format);
        
        // Try to push a frame with wrong dimensions
        let frame = VirtualCamFrame::new(1920, 1080);
        let result = filter.push_frame(frame);
        
        assert!(result.is_err());
    }
}