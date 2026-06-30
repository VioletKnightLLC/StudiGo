//! Fake UVC source - generates color-bar test pattern

use anyhow::Result;
use image::{ImageBuffer, Rgb};
use crate::capture::{FrameSource, WIDTH, HEIGHT};

/// Fake UVC source that generates a color-bar pattern at 30fps
pub struct FakeUvcSource {
    frame_counter: u32,
}

impl FakeUvcSource {
    pub fn new() -> Self {
        FakeUvcSource { frame_counter: 0 }
    }
}

impl FrameSource for FakeUvcSource {
    fn next_frame(&mut self) -> Result<Vec<u8>> {
        self.frame_counter += 1;

        // Generate color-bar pattern (SMPTE-style bars)
        // 8 vertical bars of equal width
        let bar_width = WIDTH / 8;
        
        let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(WIDTH, HEIGHT);

        // Standard SMPTE color bar colors
        let colors: [(u8, u8, u8); 8] = [
            (255, 255, 255), // White
            (255, 255, 0),   // Yellow
            (0, 255, 255),   // Cyan
            (0, 255, 0),     // Green
            (255, 0, 255),   // Magenta
            (255, 0, 0),     // Red
            (0, 0, 255),     // Blue
            (0, 0, 0),       // Black
        ];

        for (bar_idx, &(r, g, b)) in colors.iter().enumerate() {
            let start_x = (bar_idx as u32) * bar_width;
            for x in start_x..(start_x + bar_width).min(WIDTH) {
                for y in 0..HEIGHT {
                    img.put_pixel(x, y, Rgb([r, g, b]));
                }
            }
        }

        // Add half-gray bar in middle bottom (75% white)
        let half_gray = Rgb([192, 192, 192]);
        let mid_y_start = HEIGHT * 7 / 10;
        for x in 0..WIDTH {
            for y in mid_y_start..HEIGHT {
                img.put_pixel(x, y, half_gray);
            }
        }

        // Convert to RGB bytes (flatten the buffer)
        let rgb_data = img.into_raw();
        Ok(rgb_data)
    }
}