use crate::error::AppError;

pub const DISPLAY_WIDTH: usize = 240;
pub const DISPLAY_HEIGHT: usize = 160;
pub const SCALE: usize = 3;
pub const SCALED_WIDTH: usize = DISPLAY_WIDTH * SCALE;
pub const SCALED_HEIGHT: usize = DISPLAY_HEIGHT * SCALE;

/// Convert the emulator's u32 RGB24 frame buffer (0x00RRGGBB per pixel)
/// to a packed RGB byte buffer at native resolution.
pub fn to_rgb(src: &[u32]) -> Vec<u8> {
    assert_eq!(src.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT);
    let mut out = vec![0u8; DISPLAY_WIDTH * DISPLAY_HEIGHT * 3];
    for (i, &pixel) in src.iter().enumerate() {
        out[i * 3] = ((pixel >> 16) & 0xFF) as u8;
        out[i * 3 + 1] = ((pixel >> 8) & 0xFF) as u8;
        out[i * 3 + 2] = (pixel & 0xFF) as u8;
    }
    out
}

/// JPEG-encode an RGB byte buffer. Returns the raw JPEG bytes.
pub fn encode_jpeg(rgb: &[u8], width: usize, height: usize, quality: u8) -> Result<Vec<u8>, AppError> {
    let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    compress.set_size(width, height);
    compress.set_quality(quality as f32);

    let mut out = Vec::new();
    let mut started = compress
        .start_compress(&mut out)
        .map_err(|e| AppError::Jpeg(e.to_string()))?;

    let row_stride = width * 3;
    for row in 0..height {
        let row_data = &rgb[row * row_stride..(row + 1) * row_stride];
        started
            .write_scanlines(row_data)
            .map_err(|e| AppError::Jpeg(e.to_string()))?;
    }

    started
        .finish()
        .map_err(|e| AppError::Jpeg(e.to_string()))?;

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_rgb_output_dimensions() {
        let src = vec![0u32; DISPLAY_WIDTH * DISPLAY_HEIGHT];
        let rgb = to_rgb(&src);
        assert_eq!(rgb.len(), DISPLAY_WIDTH * DISPLAY_HEIGHT * 3);
        assert_eq!(rgb.len(), 240 * 160 * 3);
    }

    #[test]
    fn test_to_rgb_pixel_conversion() {
        let mut src = vec![0u32; DISPLAY_WIDTH * DISPLAY_HEIGHT];
        src[0] = 0x00FF0000;
        let rgb = to_rgb(&src);
        assert_eq!(rgb[0], 255, "red");
        assert_eq!(rgb[1], 0, "green");
        assert_eq!(rgb[2], 0, "blue");
    }

    #[test]
    fn test_to_rgb_pixel_at_arbitrary_position() {
        let mut src = vec![0u32; DISPLAY_WIDTH * DISPLAY_HEIGHT];
        src[5 * DISPLAY_WIDTH + 10] = 0x000000FF;
        let rgb = to_rgb(&src);
        let idx = (5 * DISPLAY_WIDTH + 10) * 3;
        assert_eq!(rgb[idx], 0);
        assert_eq!(rgb[idx + 1], 0);
        assert_eq!(rgb[idx + 2], 255);
    }

    #[test]
    fn test_encode_jpeg_magic_bytes() {
        let rgb = vec![128u8; DISPLAY_WIDTH * DISPLAY_HEIGHT * 3];
        let jpeg = encode_jpeg(&rgb, DISPLAY_WIDTH, DISPLAY_HEIGHT, 85)
            .expect("jpeg encode should succeed");
        assert!(!jpeg.is_empty());
        assert_eq!(&jpeg[0..2], &[0xFF, 0xD8], "should start with JPEG SOI marker");
    }

    #[test]
    fn test_encode_jpeg_produces_valid_output_size() {
        let rgb = vec![64u8; DISPLAY_WIDTH * DISPLAY_HEIGHT * 3];
        let jpeg = encode_jpeg(&rgb, DISPLAY_WIDTH, DISPLAY_HEIGHT, 85)
            .expect("jpeg encode should succeed");
        assert!(jpeg.len() < 100_000, "jpeg too large: {} bytes", jpeg.len());
        assert!(jpeg.len() > 100);
    }
}
