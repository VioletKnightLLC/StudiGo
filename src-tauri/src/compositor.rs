//! Compositor - composes multiple SourceBus frames into scenes
//!
//! This module provides the Compositor struct that takes frames from multiple
//! SourceBus implementations and composites them into a single scene output.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::frame::Frame;
use crate::scene::{Layer, Scene, Transition, TransitionType};
use crate::source_bus::SourceBus;

/// Compositor error types
#[derive(Debug, thiserror::Error)]
pub enum CompositorError {
    #[error("Source not found: {0}")]
    SourceNotFound(String),

    #[error("Frame dimension mismatch for source {0}: expected {1}x{2}, got {3}x{4}")]
    DimensionMismatch(String, u32, u32, u32, u32),

    #[error("No active scene")]
    NoActiveScene,

    #[error("Scene rendering failed: {0}")]
    RenderFailed(String),
}

/// Compositor - composites multiple source frames into scenes
pub struct Compositor {
    /// Active scene configuration
    scene: Option<Scene>,
    /// Source buffers: source_id -> latest frame
    source_buffers: HashMap<String, Arc<RwLock<Option<Frame>>>>,
    /// Output frame pool
    output_buffer: Vec<u8>,
    /// Output dimensions
    output_width: u32,
    output_height: u32,
}

impl Compositor {
    /// Create a new compositor
    pub fn new(output_width: u32, output_height: u32) -> Self {
        let buffer_size = (output_width * output_height * 3) as usize;
        Compositor {
            scene: None,
            source_buffers: HashMap::new(),
            output_buffer: vec![0u8; buffer_size],
            output_width,
            output_height,
        }
    }

    /// Create a new compositor with a scene
    pub fn with_scene(scene: Scene) -> Self {
        let width = scene.resolution.width;
        let height = scene.resolution.height;
        let mut compositor = Compositor::new(width, height);
        compositor.scene = Some(scene);
        compositor
    }

    /// Set the active scene
    pub fn set_scene(&mut self, scene: Scene) {
        self.output_width = scene.resolution.width;
        self.output_height = scene.resolution.height;
        let buffer_size = (self.output_width * self.output_height * 3) as usize;
        self.output_buffer.resize(buffer_size, 0);
        self.scene = Some(scene);
    }

    /// Get the current scene
    pub fn scene(&self) -> Option<&Scene> {
        self.scene.as_ref()
    }

    /// Register a source for compositing
    pub fn register_source(&mut self, source_id: String) {
        self.source_buffers
            .insert(source_id, Arc::new(RwLock::new(None)));
    }

    /// Unregister a source
    pub fn unregister_source(&mut self, source_id: &str) {
        self.source_buffers.remove(source_id);
    }

    /// Push a frame from a source
    pub fn push_frame(&self, source_id: &str, frame: Frame) -> Result<(), CompositorError> {
        let buffer = self
            .source_buffers
            .get(source_id)
            .ok_or_else(|| CompositorError::SourceNotFound(source_id.to_string()))?;

        let mut guard = buffer.write().map_err(|_| {
            CompositorError::RenderFailed("Failed to acquire source buffer lock".to_string())
        })?;
        *guard = Some(frame);
        Ok(())
    }

    /// Get latest frame from a source
    pub fn get_frame(&self, source_id: &str) -> Option<Frame> {
        self.source_buffers.get(source_id).and_then(|buffer| {
            buffer.read().ok().and_then(|guard| guard.clone())
        })
    }

    /// Composite all source frames into a single output frame
    pub fn composite(&mut self) -> Result<Frame, CompositorError> {
        let scene_opt = self.scene.as_ref().ok_or(CompositorError::NoActiveScene)?;

        // Clear output buffer (black background)
        self.output_buffer.fill(0);

        // Collect layers with their source frames
        let layers_with_frames: Vec<_> = scene_opt
            .layers
            .iter()
            .filter_map(|layer| {
                if !layer.visible {
                    None
                } else {
                    self.get_frame(&layer.source_id)
                        .map(|source_frame| (layer.clone(), source_frame))
                }
            })
            .collect();

        // Composite each layer (back to front)
        for (layer, source_frame) in layers_with_frames {
            self.composite_layer(&source_frame, &layer)?;
        }

        let timestamp_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_micros() as u64)
            .unwrap_or(0);

        Ok(Frame {
            data: self.output_buffer.clone(),
            width: self.output_width,
            height: self.output_height,
            timestamp_us,
            source_id: "compositor".to_string(),
        })
    }

    /// Composite a single layer onto the output
    fn composite_layer(
        &mut self,
        source_frame: &Frame,
        layer: &Layer,
    ) -> Result<(), CompositorError> {
        let src_width = source_frame.width;
        let src_height = source_frame.height;
        let dst_width = layer.size.width;
        let dst_height = layer.size.height;

        let x_offset = layer.position.x.max(0) as u32;
        let y_offset = layer.position.y.max(0) as u32;

        // Calculate actual compositing bounds
        let max_dst_x = (x_offset + dst_width).min(self.output_width);
        let max_dst_y = (y_offset + dst_height).min(self.output_height);
        let actual_width = max_dst_x - x_offset;
        let actual_height = max_dst_y - y_offset;

        if actual_width == 0 || actual_height == 0 {
            return Ok(());
        }

        // Simple nearest-neighbor scaling
        let x_ratio = src_width as f32 / dst_width as f32;
        let y_ratio = src_height as f32 / dst_height as f32;

        for dy in 0..actual_height {
            for dx in 0..actual_width {
                // Source coordinates
                let src_x = (dx as f32 * x_ratio) as u32;
                let src_y = (dy as f32 * y_ratio) as u32;

                // Clamp to source bounds
                let src_x = src_x.min(src_width - 1);
                let src_y = src_y.min(src_height - 1);

                // Source and destination pixel indices
                let src_idx = ((src_y * src_width + src_x) * 3) as usize;
                let dst_idx = (((y_offset + dy) * self.output_width + (x_offset + dx)) * 3) as usize;

                if src_idx + 2 < source_frame.data.len() && dst_idx + 2 < self.output_buffer.len()
                {
                    // Apply opacity
                    let opacity = layer.opacity;
                    for c in 0..3 {
                        let src_val = source_frame.data[src_idx + c] as f32;
                        let dst_val = self.output_buffer[dst_idx + c] as f32;
                        let composited = dst_val + (src_val - dst_val) * opacity;
                        self.output_buffer[dst_idx + c] = composited as u8;
                    }
                }
            }
        }

        Ok(())
    }

    /// Apply a transition between scenes
    pub fn apply_transition(
        &mut self,
        from_scene: &Scene,
        to_scene: &Scene,
        progress: f32,
    ) -> Result<Frame, CompositorError> {
        let transition = to_scene
            .transition
            .as_ref()
            .ok_or(CompositorError::NoActiveScene)?;

        match transition.transition_type {
            TransitionType::Cut => {
                self.set_scene(to_scene.clone());
                self.composite()
            }
            TransitionType::Fade => {
                // For fade, we render both scenes and blend
                let from_compositor = &mut Compositor::with_scene(from_scene.clone());
                // Copy buffers
                for (id, buffer) in &self.source_buffers {
                    if let Some(frame) = self.get_frame(id) {
                        from_compositor.push_frame(id, frame).ok();
                    }
                }
                if let Ok(from_frame) = from_compositor.composite() {
                    self.set_scene(to_scene.clone());
                    let to_frame = self.composite()?;

                    // Cross-fade
                    let alpha = progress;
                    for i in 0..self.output_buffer.len().min(from_frame.data.len()) {
                        let from_val = from_frame.data[i] as f32;
                        let to_val = to_frame.data[i] as f32;
                        self.output_buffer[i] = (from_val * (1.0 - alpha) + to_val * alpha) as u8;
                    }
                    return self.composite();
                } else {
                    self.set_scene(to_scene.clone());
                    return self.composite();
                }
            }
            _ => {
                // For other transitions, just cut for now
                self.set_scene(to_scene.clone());
                self.composite()
            }
        }
    }
}

/// Builder for creating Compositor instances with configuration
pub struct CompositorBuilder {
    output_width: u32,
    output_height: u32,
    scene: Option<Scene>,
    sources: Vec<String>,
}

impl CompositorBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        CompositorBuilder {
            output_width: 1920,
            output_height: 1080,
            scene: None,
            sources: Vec::new(),
        }
    }

    /// Set output resolution
    pub fn resolution(mut self, width: u32, height: u32) -> Self {
        self.output_width = width;
        self.output_height = height;
        self
    }

    /// Set the scene
    pub fn scene(mut self, scene: Scene) -> Self {
        self.scene = Some(scene);
        self
    }

    /// Add a source to register
    pub fn add_source(mut self, source_id: String) -> Self {
        self.sources.push(source_id);
        self
    }

    /// Build the compositor
    pub fn build(self) -> Compositor {
        let mut compositor = match self.scene {
            Some(scene) => Compositor::with_scene(scene),
            None => Compositor::new(self.output_width, self.output_height),
        };

        for source_id in self.sources {
            compositor.register_source(source_id);
        }

        compositor
    }
}

impl Default for CompositorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::layouts;

    #[test]
    fn test_compositor_creation() {
        let compositor = Compositor::new(1920, 1080);
        assert_eq!(compositor.output_width, 1920);
        assert_eq!(compositor.output_height, 1080);
    }

    #[test]
    fn test_compositor_with_scene() {
        let scene = layouts::single("camera", 1920, 1080);
        let compositor = Compositor::with_scene(scene);
        assert!(compositor.scene().is_some());
    }

    #[test]
    fn test_register_source() {
        let mut compositor = Compositor::new(1920, 1080);
        compositor.register_source("camera".to_string());
        compositor.register_source("screen".to_string());
        assert_eq!(compositor.source_buffers.len(), 2);
    }

    #[test]
    fn test_push_and_get_frame() {
        let mut compositor = Compositor::new(640, 480);
        compositor.register_source("camera".to_string());

        let frame = Frame::new(640, 480, "camera");
        compositor.push_frame("camera", frame).unwrap();

        let retrieved = compositor.get_frame("camera");
        assert!(retrieved.is_some());
    }

    #[test]
    fn test_composite_single_source() {
        let scene = layouts::single("camera", 640, 480);
        let mut compositor = Compositor::with_scene(scene);
        compositor.register_source("camera".to_string());

        let frame = Frame::from_data(vec![255u8; 640 * 480 * 3], 640, 480, "camera");
        compositor.push_frame("camera", frame).unwrap();

        let result = compositor.composite();
        assert!(result.is_ok());
    }

    #[test]
    fn test_compositor_builder() {
        let compositor = CompositorBuilder::new()
            .resolution(1920, 1080)
            .add_source("camera".to_string())
            .add_source("screen".to_string())
            .build();

        assert_eq!(compositor.output_width, 1920);
        assert_eq!(compositor.source_buffers.len(), 2);
    }
}