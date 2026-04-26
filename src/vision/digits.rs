use anyhow::Result;
use image::{DynamicImage, GenericImage, ImageBuffer, Rgba};

/// Digit patterns for drawing numbers (5x3 bitmap patterns)
/// Each digit is represented as a 5-row, 3-column pattern
const DIGIT_PATTERNS: [[bool; 15]; 10] = [
    // 0
    [
        true, true, true, true, false, true, true, false, true, true, false, true, true, true, true,
    ],
    // 1
    [
        false, true, false, true, true, false, false, true, false, false, true, false, true, true,
        true,
    ],
    // 2
    [
        true, true, true, false, false, true, true, true, true, true, false, false, true, true,
        true,
    ],
    // 3
    [
        true, true, true, false, false, true, true, true, true, false, false, true, true, true,
        true,
    ],
    // 4
    [
        true, false, true, true, false, true, true, true, true, false, false, true, false, false,
        true,
    ],
    // 5
    [
        true, true, true, true, false, false, true, true, true, false, false, true, true, true,
        true,
    ],
    // 6
    [
        true, true, true, true, false, false, true, true, true, true, false, true, true, true, true,
    ],
    // 7
    [
        true, true, true, false, false, true, false, true, false, true, false, false, true, false,
        false,
    ],
    // 8
    [
        true, true, true, true, false, true, true, true, true, true, false, true, true, true, true,
    ],
    // 9
    [
        true, true, true, true, false, true, true, true, true, false, false, true, true, true, true,
    ],
];

/// Digits drawing utility for annotating screenshots
pub struct Digits;

impl Digits {
    /// Draw a single digit on an RGBA image buffer
    pub fn draw_digit_on_buffer(
        img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: i32,
        y: i32,
        digit: u32,
        color: Rgba<u8>,
        scale: u32,
    ) -> Result<()> {
        if digit > 9 {
            anyhow::bail!("Single digit must be 0-9");
        }

        let pattern = &DIGIT_PATTERNS[digit as usize];
        let (width, height) = img.dimensions();

        // Draw the digit pattern scaled
        for row in 0..5u32 {
            for col in 0..3u32 {
                if pattern[(row * 3 + col) as usize] {
                    // Draw scaled pixel
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = x + (col * scale + sx) as i32;
                            let py = y + (row * scale + sy) as i32;

                            // Check bounds
                            if px >= 0 && py >= 0 {
                                let px_u = px as u32;
                                let py_u = py as u32;
                                if px_u < width && py_u < height {
                                    img.put_pixel(px_u, py_u, color);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Draw a number on an RGBA image buffer
    pub fn draw_number_on_buffer(
        img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: i32,
        y: i32,
        number: u32,
        color: Rgba<u8>,
        scale: u32,
    ) -> Result<()> {
        let digits_str = number.to_string();
        let mut spacing = 0i32;

        for digit_char in digits_str.chars() {
            let digit = digit_char
                .to_digit(10)
                .ok_or_else(|| anyhow::anyhow!("Invalid digit character"))?;

            Self::draw_digit_on_buffer(img, x + spacing, y, digit, color, scale)?;

            // Spacing between digits: digit width (3*scale) + gap (scale)
            spacing += (4 * scale) as i32;
        }

        Ok(())
    }

    /// Get the width needed to draw a number
    pub fn get_number_width(number: u32, scale: u32) -> u32 {
        let digit_count = if number == 0 {
            1
        } else {
            number.to_string().len() as u32
        };
        // Each digit: 3*scale width, plus scale spacing between digits
        // Total: digit_count * 3*scale + (digit_count - 1) * scale
        // For single digit: 3*scale + scale (extra padding) = 4*scale
        digit_count * 4 * scale
    }

    /// Get the height needed to draw a digit
    pub fn get_digit_height(scale: u32) -> u32 {
        5 * scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{DynamicImage, ImageBuffer};

    #[test]
    fn test_digit_patterns_exist() {
        // All 10 digits should have patterns
        for i in 0..10 {
            let pattern = &DIGIT_PATTERNS[i];
            assert_eq!(pattern.len(), 15);
        }
    }

    #[test]
    fn test_get_number_width() {
        assert_eq!(Digits::get_number_width(0, 1), 4);
        assert_eq!(Digits::get_number_width(1, 1), 4);
        assert_eq!(Digits::get_number_width(12, 1), 8);
        assert_eq!(Digits::get_number_width(123, 1), 12);
        assert_eq!(Digits::get_number_width(1, 2), 8);
        assert_eq!(Digits::get_number_width(12, 2), 16);
    }

    #[test]
    fn test_get_digit_height() {
        assert_eq!(Digits::get_digit_height(1), 5);
        assert_eq!(Digits::get_digit_height(2), 10);
        assert_eq!(Digits::get_digit_height(3), 15);
    }

    #[test]
    fn test_draw_single_digit() {
        let mut img = ImageBuffer::new(100, 100);
        let color = Rgba([255, 255, 255, 255]);

        let result = Digits::draw_digit_on_buffer(&mut img, 10, 10, 5, color, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_draw_number() {
        let mut img = ImageBuffer::new(200, 200);
        let color = Rgba([0, 0, 0, 255]);

        let result = Digits::draw_number_on_buffer(&mut img, 20, 20, 123, color, 3);
        assert!(result.is_ok());

        // Test zero
        let result = Digits::draw_number_on_buffer(&mut img, 20, 50, 0, color, 2);
        assert!(result.is_ok());
    }

    #[test]
    fn test_draw_digit_bounds_check() {
        // Should not fail for out-of-bounds coordinates
        let mut img = ImageBuffer::new(10, 10);
        let color = Rgba([255, 0, 0, 255]);

        // Drawing at negative coordinates should silently skip
        let result = Digits::draw_digit_on_buffer(&mut img, -5, -5, 1, color, 1);
        assert!(result.is_ok());

        // Drawing beyond image bounds should silently skip
        let result = Digits::draw_digit_on_buffer(&mut img, 100, 100, 1, color, 1);
        assert!(result.is_ok());
    }
}
