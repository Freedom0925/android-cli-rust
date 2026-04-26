use anyhow::{Context, Result};
use image::{DynamicImage, GenericImageView, ImageBuffer, Luma, Rgb, Rgba};

use crate::vision::Rect;

/// Image utilities for vision processing
pub struct ImageUtils;

impl ImageUtils {
    /// Create a copy of an image
    pub fn copy_image(img: &DynamicImage) -> DynamicImage {
        img.clone()
    }

    /// Convert image to grayscale
    pub fn to_grayscale(img: &DynamicImage) -> ImageBuffer<Luma<u8>, Vec<u8>> {
        img.to_luma8()
    }

    /// Iterate over each pixel with callback
    ///
    /// # Arguments
    /// * `img` - Input image
    /// * `f` - Callback function (x, y, pixel)
    pub fn for_each_pixel<F>(img: &DynamicImage, mut f: F)
    where
        F: FnMut(u32, u32, Rgba<u8>),
    {
        let (width, height) = img.dimensions();
        for y in 0..height {
            for x in 0..width {
                let pixel = img.get_pixel(x, y);
                f(x, y, pixel);
            }
        }
    }

    /// Iterate over each pixel of an RGBA buffer with callback
    pub fn for_each_pixel_rgba<F>(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, mut f: F)
    where
        F: FnMut(u32, u32, &Rgba<u8>),
    {
        let (width, height) = img.dimensions();
        for y in 0..height {
            for x in 0..width {
                let pixel = img.get_pixel(x, y);
                f(x, y, pixel);
            }
        }
    }

    /// Iterate over each pixel of a grayscale buffer with callback
    pub fn for_each_pixel_luma<F>(img: &ImageBuffer<Luma<u8>, Vec<u8>>, mut f: F)
    where
        F: FnMut(u32, u32, &Luma<u8>),
    {
        let (width, height) = img.dimensions();
        for y in 0..height {
            for x in 0..width {
                let pixel = img.get_pixel(x, y);
                f(x, y, pixel);
            }
        }
    }

    /// Safely set pixel with bounds checking
    ///
    /// # Arguments
    /// * `img` - Target image buffer
    /// * `x` - X coordinate (can be negative)
    /// * `y` - Y coordinate (can be negative)
    /// * `color` - Pixel color to set
    pub fn safe_set_pixel(
        img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
        x: i32,
        y: i32,
        color: Rgba<u8>,
    ) {
        if x >= 0 && y >= 0 {
            let (width, height) = img.dimensions();
            if x < width as i32 && y < height as i32 {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }

    /// Safely set grayscale pixel with bounds checking
    pub fn safe_set_pixel_luma(
        img: &mut ImageBuffer<Luma<u8>, Vec<u8>>,
        x: i32,
        y: i32,
        color: Luma<u8>,
    ) {
        if x >= 0 && y >= 0 {
            let (width, height) = img.dimensions();
            if x < width as i32 && y < height as i32 {
                img.put_pixel(x as u32, y as u32, color);
            }
        }
    }

    /// Safely set pixel to white (1) in binary image
    pub fn safe_set_white(img: &mut ImageBuffer<Luma<u8>, Vec<u8>>, x: i32, y: i32) {
        Self::safe_set_pixel_luma(img, x, y, Luma([255]));
    }

    /// Safely set pixel to black (0) in binary image
    pub fn safe_set_black(img: &mut ImageBuffer<Luma<u8>, Vec<u8>>, x: i32, y: i32) {
        Self::safe_set_pixel_luma(img, x, y, Luma([0]));
    }

    /// Draw a rectangle on an image (returns a new RGBA image with the rect drawn)
    pub fn draw_rect(img: &DynamicImage, rect: &Rect, color: Rgba<u8>) -> DynamicImage {
        let mut rgba_img = img.to_rgba8();
        let (width, height) = rgba_img.dimensions();
        let min_x = rect.min_x.max(0) as u32;
        let max_x = rect.max_x.min(width as i32) as u32;
        let min_y = rect.min_y.max(0) as u32;
        let max_y = rect.max_y.min(height as i32) as u32;

        // Draw horizontal lines (top and bottom)
        for x in min_x..max_x {
            if min_y < height {
                rgba_img.put_pixel(x, min_y, color);
            }
            if max_y > 0 && max_y <= height {
                rgba_img.put_pixel(x, max_y - 1, color);
            }
        }

        // Draw vertical lines (left and right)
        for y in min_y..max_y {
            if min_x < width {
                rgba_img.put_pixel(min_x, y, color);
            }
            if max_x > 0 && max_x <= width {
                rgba_img.put_pixel(max_x - 1, y, color);
            }
        }

        DynamicImage::ImageRgba8(rgba_img)
    }

    /// Draw a number at specified position (returns new RGBA image)
    pub fn draw_number(
        img: &DynamicImage,
        x: i32,
        y: i32,
        number: u32,
        color: Rgba<u8>,
        scale: u32,
    ) -> Result<DynamicImage> {
        let mut rgba_img = img.to_rgba8();
        let number_str = number.to_string();
        let mut spacing = 0i32;

        for digit_char in number_str.chars() {
            let digit = digit_char.to_digit(10).context("Invalid digit character")?;
            crate::vision::digits::Digits::draw_digit_on_buffer(
                &mut rgba_img,
                x + spacing,
                y,
                digit,
                color,
                scale,
            )?;
            spacing += (4 * scale) as i32;
        }

        Ok(DynamicImage::ImageRgba8(rgba_img))
    }

    /// Get RGB components from a pixel value
    pub fn get_rgb_components(rgb: u32) -> (u8, u8, u8) {
        let r = ((rgb >> 16) & 0xFF) as u8;
        let g = ((rgb >> 8) & 0xFF) as u8;
        let b = (rgb & 0xFF) as u8;
        (r, g, b)
    }

    /// Calculate average color in a region
    pub fn average_color(img: &DynamicImage, rect: &Rect) -> Option<Rgb<u8>> {
        let (width, height) = img.dimensions();
        let min_x = rect.min_x.max(0) as u32;
        let max_x = rect.max_x.min(width as i32) as u32;
        let min_y = rect.min_y.max(0) as u32;
        let max_y = rect.max_y.min(height as i32) as u32;

        if min_x >= max_x || min_y >= max_y {
            return None;
        }

        let mut r_sum = 0u64;
        let mut g_sum = 0u64;
        let mut b_sum = 0u64;
        let mut count = 0u64;

        for y in min_y..max_y {
            for x in min_x..max_x {
                let pixel = img.get_pixel(x, y);
                r_sum += pixel[0] as u64;
                g_sum += pixel[1] as u64;
                b_sum += pixel[2] as u64;
                count += 1;
            }
        }

        if count == 0 {
            return None;
        }

        Some(Rgb([
            (r_sum / count) as u8,
            (g_sum / count) as u8,
            (b_sum / count) as u8,
        ]))
    }

    /// Detect edges using simple gradient (deprecated - use SobelEdges instead)
    pub fn detect_edges(img: &DynamicImage) -> ImageBuffer<Rgb<u8>, Vec<u8>> {
        img.to_rgb8()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    #[test]
    fn test_rgb_components() {
        let (r, g, b) = ImageUtils::get_rgb_components(0xFF0000);
        assert_eq!(r, 255);
        assert_eq!(g, 0);
        assert_eq!(b, 0);

        let (r, g, b) = ImageUtils::get_rgb_components(0x00FF00);
        assert_eq!(r, 0);
        assert_eq!(g, 255);
        assert_eq!(b, 0);

        let (r, g, b) = ImageUtils::get_rgb_components(0x0000FF);
        assert_eq!(r, 0);
        assert_eq!(g, 0);
        assert_eq!(b, 255);
    }

    #[test]
    fn test_grayscale_conversion() {
        let img = DynamicImage::new_rgba8(10, 10);
        let gray = ImageUtils::to_grayscale(&img);
        assert_eq!(gray.dimensions(), (10, 10));
    }

    #[test]
    fn test_copy_image() {
        let img = DynamicImage::new_rgba8(100, 100);
        let copy = ImageUtils::copy_image(&img);
        assert_eq!(copy.dimensions(), img.dimensions());
    }

    #[test]
    fn test_draw_rect() {
        let img = DynamicImage::new_rgba8(100, 100);
        let rect = Rect::new(10, 10, 50, 50);
        let color = Rgba([255, 0, 0, 255]);

        let result = ImageUtils::draw_rect(&img, &rect, color);
        assert_eq!(result.dimensions(), (100, 100));
    }
}
