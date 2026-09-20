//! Scene configuration schema
//!
//! This module defines the scene configuration schema for composing multiple
//! video sources into a single output with layout, sources, and transitions.

use serde::{Deserialize, Serialize};

/// Scene configuration - defines how sources are composited
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// Unique scene identifier
    pub id: String,
    /// Scene name for display
    pub name: String,
    /// Output resolution
    pub resolution: Resolution,
    /// List of source layers in z-order (back to front)
    pub layers: Vec<Layer>,
    /// Active transition configuration
    pub transition: Option<Transition>,
}

impl Scene {
    /// Create a new scene with default settings
    pub fn new(id: &str, name: &str, width: u32, height: u32) -> Self {
        Scene {
            id: id.to_string(),
            name: name.to_string(),
            resolution: Resolution { width, height },
            layers: Vec::new(),
            transition: None,
        }
    }

    /// Add a layer to the scene
    pub fn add_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    /// Set a transition for scene changes
    pub fn set_transition(&mut self, transition: Transition) {
        self.transition = Some(transition);
    }
}

/// Output resolution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub fn new(width: u32, height: u32) -> Self {
        Resolution { width, height }
    }
}

/// A single layer in the scene (source + position + size)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    /// Source identifier (matches SourceBus source_id)
    pub source_id: String,
    /// Position on canvas
    pub position: Position,
    /// Size on canvas
    pub size: Size,
    /// Opacity (0.0 = invisible, 1.0 = fully visible)
    pub opacity: f32,
    /// Whether this layer is visible
    pub visible: bool,
    /// Audio volume for this source (0.0 = muted, 1.0 = full)
    pub volume: f32,
}

impl Layer {
    /// Create a new layer
    pub fn new(source_id: &str, x: i32, y: i32, width: u32, height: u32) -> Self {
        Layer {
            source_id: source_id.to_string(),
            position: Position::new(x, y),
            size: Size::new(width, height),
            opacity: 1.0,
            visible: true,
            volume: 1.0,
        }
    }
}

/// 2D position
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

impl Position {
    pub fn new(x: i32, y: i32) -> Self {
        Position { x, y }
    }
}

/// 2D size
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Size {
    pub width: u32,
    pub height: u32,
}

impl Size {
    pub fn new(width: u32, height: u32) -> Self {
        Size { width, height }
    }
}

/// Transition types for scene changes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum TransitionType {
    /// Instant cut (no transition)
    #[default]
    Cut,
    /// Cross-fade between scenes
    Fade,
    /// Slide in from direction
    Slide(SlideDirection),
    /// Push transition
    Push(PushDirection),
    /// Zoom transition
    Zoom,
}

/// Slide direction for slide transitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SlideDirection {
    Left,
    Right,
    Top,
    Bottom,
}

/// Push direction for push transitions
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PushDirection {
    Left,
    Right,
    Top,
    Bottom,
}

/// Transition configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transition {
    /// Transition type
    pub transition_type: TransitionType,
    /// Transition duration in milliseconds
    pub duration_ms: u32,
}

impl Transition {
    /// Create a new transition
    pub fn new(transition_type: TransitionType, duration_ms: u32) -> Self {
        Transition {
            transition_type,
            duration_ms,
        }
    }

    /// Create a default fade transition (500ms)
    pub fn fade() -> Self {
        Transition {
            transition_type: TransitionType::Fade,
            duration_ms: 500,
        }
    }

    /// Create a default cut (instant)
    pub fn cut() -> Self {
        Transition {
            transition_type: TransitionType::Cut,
            duration_ms: 0,
        }
    }
}

/// Predefined scene layouts
pub mod layouts {
    use super::*;

    /// Single source full screen
    pub fn single(source_id: &str, output_width: u32, output_height: u32) -> Scene {
        let mut scene = Scene::new("single", "Single Source", output_width, output_height);
        scene.add_layer(Layer::new(source_id, 0, 0, output_width, output_height));
        scene
    }

    /// Picture-in-picture (small inset)
    pub fn pip(
        main_source: &str,
        pip_source: &str,
        output_width: u32,
        output_height: u32,
        pip_width: u32,
        pip_height: u32,
        pip_position: Position,
    ) -> Scene {
        let mut scene = Scene::new("pip", "Picture in Picture", output_width, output_height);

        // Main source (full screen)
        scene.add_layer(Layer::new(main_source, 0, 0, output_width, output_height));

        // PiP source (inset)
        let mut pip_layer = Layer::new(
            pip_source,
            pip_position.x,
            pip_position.y,
            pip_width,
            pip_height,
        );
        pip_layer.opacity = 1.0;
        scene.add_layer(pip_layer);

        scene
    }

    /// Side by side (horizontal split)
    pub fn side_by_side(
        left_source: &str,
        right_source: &str,
        output_width: u32,
        output_height: u32,
    ) -> Scene {
        let mut scene = Scene::new("side_by_side", "Side by Side", output_width, output_height);
        let half_width = output_width / 2;

        scene.add_layer(Layer::new(left_source, 0, 0, half_width, output_height));
        scene.add_layer(Layer::new(
            right_source,
            half_width as i32,
            0,
            half_width,
            output_height,
        ));

        scene
    }

    /// Grid layout (2x2)
    pub fn grid_2x2(sources: &[&str; 4], output_width: u32, output_height: u32) -> Scene {
        let mut scene = Scene::new("grid_2x2", "2x2 Grid", output_width, output_height);
        let half_width = output_width / 2;
        let half_height = output_height / 2;

        // Top-left
        scene.add_layer(Layer::new(sources[0], 0, 0, half_width, half_height));
        // Top-right
        scene.add_layer(Layer::new(
            sources[1],
            half_width as i32,
            0,
            half_width,
            half_height,
        ));
        // Bottom-left
        scene.add_layer(Layer::new(
            sources[2],
            0,
            half_height as i32,
            half_width,
            half_height,
        ));
        // Bottom-right
        scene.add_layer(Layer::new(
            sources[3],
            half_width as i32,
            half_height as i32,
            half_width,
            half_height,
        ));

        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scene_creation() {
        let scene = Scene::new("test", "Test Scene", 1920, 1080);
        assert_eq!(scene.id, "test");
        assert_eq!(scene.resolution.width, 1920);
        assert_eq!(scene.layers.len(), 0);
    }

    #[test]
    fn test_single_layout() {
        let scene = layouts::single("camera", 1920, 1080);
        assert_eq!(scene.layers.len(), 1);
        assert_eq!(scene.layers[0].source_id, "camera");
    }

    #[test]
    fn test_pip_layout() {
        let scene = layouts::pip(
            "camera",
            "screen",
            1920,
            1080,
            320,
            180,
            Position::new(1600, 900),
        );
        assert_eq!(scene.layers.len(), 2);
    }

    #[test]
    fn test_transition() {
        let fade = Transition::fade();
        assert_eq!(fade.transition_type, TransitionType::Fade);
        assert_eq!(fade.duration_ms, 500);
    }
}
