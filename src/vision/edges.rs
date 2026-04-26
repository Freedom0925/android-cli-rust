use image::{DynamicImage, GenericImageView, ImageBuffer, Luma};

/// Sobel edge detection with automatic or manual threshold
///
/// Implements the Sobel-Feldman operator for edge detection with:
/// - Sobel convolution kernels (3x3)
/// - Gradient magnitude calculation
/// - Otsu's method for automatic threshold detection
/// - Binary output image
pub struct SobelEdges;

impl SobelEdges {
    /// Apply Sobel edge detection with optional threshold
    ///
    /// If threshold is 0 or negative, uses Otsu's method to compute optimal threshold
    ///
    /// # Arguments
    /// * `img` - Input image
    /// * `threshold` - Edge threshold (0 = auto-detect using Otsu)
    ///
    /// # Returns
    /// Binary edge image (white = edge, black = no edge)
    pub fn sobel_edges_with_threshold(
        img: &DynamicImage,
        threshold: i32,
    ) -> ImageBuffer<Luma<u8>, Vec<u8>> {
        let (width, height) = img.dimensions();

        // Convert to grayscale
        let grayscale = img.to_luma8();

        // Create edge magnitude image
        let mut edge_image = ImageBuffer::new(width, height);
        let mut histogram = [0u32; 256];

        // Apply Sobel kernel (skip border pixels)
        for y in 1..height - 1 {
            for x in 1..width - 1 {
                // Sobel X kernel: [[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]]
                let gx = -(grayscale.get_pixel(x - 1, y - 1)[0] as f64)
                    + grayscale.get_pixel(x + 1, y - 1)[0] as f64
                    - 2.0 * grayscale.get_pixel(x - 1, y)[0] as f64
                    + 2.0 * grayscale.get_pixel(x + 1, y)[0] as f64
                    - (grayscale.get_pixel(x - 1, y + 1)[0] as f64)
                    + grayscale.get_pixel(x + 1, y + 1)[0] as f64;

                // Sobel Y kernel: [[-1, -2, -1], [0, 0, 0], [1, 2, 1]]
                let gy = -(grayscale.get_pixel(x - 1, y - 1)[0] as f64)
                    - 2.0 * grayscale.get_pixel(x, y - 1)[0] as f64
                    - (grayscale.get_pixel(x + 1, y - 1)[0] as f64)
                    + grayscale.get_pixel(x - 1, y + 1)[0] as f64
                    + 2.0 * grayscale.get_pixel(x, y + 1)[0] as f64
                    + grayscale.get_pixel(x + 1, y + 1)[0] as f64;

                // Compute magnitude
                let magnitude = (gx * gx + gy * gy).sqrt();
                let edge_value = magnitude.min(255.0) as u8;

                edge_image.put_pixel(x, y, Luma([edge_value]));
                histogram[edge_value as usize] += 1;
            }
        }

        // Determine threshold
        let final_threshold = if threshold > 0 {
            threshold as u8
        } else {
            find_otsu_threshold(&histogram, (width * height) as u64)
        };

        // Create binary output
        let mut binary_image = ImageBuffer::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let pixel = edge_image.get_pixel(x, y);
                let value = if pixel[0] > final_threshold { 255 } else { 0 };
                binary_image.put_pixel(x, y, Luma([value]));
            }
        }

        binary_image
    }

    /// Apply Sobel edge detection returning edge magnitude image (not binary)
    pub fn sobel_edges_raw(img: &DynamicImage) -> ImageBuffer<Luma<u8>, Vec<u8>> {
        let (width, height) = img.dimensions();
        let grayscale = img.to_luma8();
        let mut edge_image = ImageBuffer::new(width, height);

        for y in 1..height - 1 {
            for x in 1..width - 1 {
                let gx = -(grayscale.get_pixel(x - 1, y - 1)[0] as f64)
                    + grayscale.get_pixel(x + 1, y - 1)[0] as f64
                    - 2.0 * grayscale.get_pixel(x - 1, y)[0] as f64
                    + 2.0 * grayscale.get_pixel(x + 1, y)[0] as f64
                    - (grayscale.get_pixel(x - 1, y + 1)[0] as f64)
                    + grayscale.get_pixel(x + 1, y + 1)[0] as f64;

                let gy = -(grayscale.get_pixel(x - 1, y - 1)[0] as f64)
                    - 2.0 * grayscale.get_pixel(x, y - 1)[0] as f64
                    - (grayscale.get_pixel(x + 1, y - 1)[0] as f64)
                    + grayscale.get_pixel(x - 1, y + 1)[0] as f64
                    + 2.0 * grayscale.get_pixel(x, y + 1)[0] as f64
                    + grayscale.get_pixel(x + 1, y + 1)[0] as f64;

                let magnitude = (gx * gx + gy * gy).sqrt();
                let edge_value = magnitude.min(255.0) as u8;
                edge_image.put_pixel(x, y, Luma([edge_value]));
            }
        }

        edge_image
    }
}

/// Find optimal threshold using Otsu's method
///
/// Otsu's method maximizes the between-class variance to find
/// the optimal threshold for separating foreground (edges) from background
fn find_otsu_threshold(histogram: &[u32; 256], _total_pixels: u64) -> u8 {
    let mut sum: f64 = 0.0;
    let mut total: u64 = 0;

    for i in 0..256 {
        sum += i as f64 * histogram[i] as f64;
        total += histogram[i] as u64;
    }

    // Handle edge case of empty histogram
    if total == 0 {
        return 128; // Default threshold
    }

    let mut sum_b: f64 = 0.0;
    let mut w_b: u64 = 0;
    let mut var_max: f64 = 0.0;
    let mut threshold: u8 = 0;

    for i in 0..256 {
        w_b += histogram[i] as u64;

        if w_b == 0 {
            continue;
        }

        let w_f = total - w_b;

        if w_f == 0 {
            break;
        }

        sum_b += i as f64 * histogram[i] as f64;

        let m_b = sum_b / w_b as f64;
        let m_f = (sum - sum_b) / w_f as f64;

        // Between-class variance
        let var_between = w_b as f64 * w_f as f64 * (m_b - m_f) * (m_b - m_f);

        if var_between > var_max {
            var_max = var_between;
            threshold = i as u8;
        }
    }

    threshold
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::DynamicImage;

    #[test]
    fn test_sobel_edges_basic() {
        let img = DynamicImage::new_rgba8(100, 100);
        let edges = SobelEdges::sobel_edges_with_threshold(&img, 0);

        assert_eq!(edges.dimensions(), (100, 100));
    }

    #[test]
    fn test_sobel_edges_manual_threshold() {
        let img = DynamicImage::new_rgba8(50, 50);
        let edges = SobelEdges::sobel_edges_with_threshold(&img, 50);

        assert_eq!(edges.dimensions(), (50, 50));
    }

    #[test]
    fn test_sobel_edges_raw() {
        let img = DynamicImage::new_rgba8(30, 30);
        let edges = SobelEdges::sobel_edges_raw(&img);

        assert_eq!(edges.dimensions(), (30, 30));
    }

    #[test]
    fn test_otsu_threshold_empty_histogram() {
        let histogram = [0u32; 256];
        let threshold = find_otsu_threshold(&histogram, 0);

        // Default threshold for empty histogram
        assert_eq!(threshold, 128);
    }

    #[test]
    fn test_otsu_threshold_uniform() {
        let mut histogram = [0u32; 256];
        // Uniform distribution
        for i in 0..256 {
            histogram[i] = 100;
        }
        let threshold = find_otsu_threshold(&histogram, 25600);

        // Should be near middle for uniform distribution
        assert!(threshold > 100 && threshold < 200);
    }

    #[test]
    fn test_otsu_threshold_bimodal() {
        let mut histogram = [0u32; 256];
        // Two peaks: low values and high values
        for i in 0..50 {
            histogram[i] = 500;
        }
        for i in 200..256 {
            histogram[i] = 500;
        }
        let threshold = find_otsu_threshold(&histogram, 28000);

        // Threshold should be somewhere between the two peaks or at boundary
        // The optimal threshold may be at one of the peak boundaries (49 or 200)
        assert!(threshold >= 49 && threshold <= 200);
    }

    #[test]
    fn test_edge_detection_with_gradient() {
        // Create image with clear edge (left half black, right half white)
        let mut img = ImageBuffer::new(100, 100);
        for y in 0..100 {
            for x in 0..100 {
                let value = if x < 50 { 0 } else { 255 };
                img.put_pixel(x, y, Luma([value]));
            }
        }
        let dynamic_img = DynamicImage::ImageLuma8(img);

        let edges = SobelEdges::sobel_edges_with_threshold(&dynamic_img, 0);

        // Edge should be detected near x=50
        let edge_detected = edges.get_pixel(50, 50)[0] > 0;
        assert!(edge_detected);
    }

    #[test]
    fn test_sobel_edges_single_pixel() {
        // Create image with single bright pixel in center
        let mut img = ImageBuffer::new(10, 10);
        for y in 0..10 {
            for x in 0..10 {
                img.put_pixel(x, y, Luma([0]));
            }
        }
        img.put_pixel(5, 5, Luma([255]));

        let dynamic_img = DynamicImage::ImageLuma8(img);
        let edges = SobelEdges::sobel_edges_with_threshold(&dynamic_img, 10);

        // Edges should be detected around the bright pixel
        assert_eq!(edges.dimensions(), (10, 10));
    }

    #[test]
    fn test_sobel_kernel_values() {
        // Verify Sobel kernel produces expected results on known pattern
        let mut img = ImageBuffer::new(5, 5);
        // Create diagonal gradient
        for y in 0..5u32 {
            for x in 0..5u32 {
                img.put_pixel(x, y, Luma([(x + y) as u8 * 25])); // Use 25 to avoid overflow
            }
        }

        let dynamic_img = DynamicImage::ImageLuma8(img);
        let edges = SobelEdges::sobel_edges_raw(&dynamic_img);

        // Center pixel should have some edge value
        let center_edge = edges.get_pixel(2, 2)[0];
        assert!(center_edge > 0);
    }
}
