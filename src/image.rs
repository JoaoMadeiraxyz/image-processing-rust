use image::{ImageBuffer, Rgba, open, ImageFormat};
use std::path::Path;

pub struct Image {
    data: ImageBuffer<Rgba<u8>, Vec<u8>>,
    filename: String,
}

impl Image {
    pub fn new(filename: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data = open(filename)?;
        let rgba_image = data.to_rgba8();
        Ok(Image {
            data: rgba_image,
            filename: filename.to_string(),
        })
    }
    
    pub fn blank(width: u32, height: u32, channels: u32) -> Self {
        let data = ImageBuffer::new(width, height);
        Image {
            data,
            filename: String::new(),
        }
    }
    
    pub fn read(&mut self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let img = open(filename)?;
        let rgba_image = img.to_rgba8();
        self.data = rgba_image;
        self.filename = filename.to_string();
        Ok(())
    }
    
    pub fn write(&self, filename: &str) -> Result<(), Box<dyn std::error::Error>> {
        let format = Self::detect_format(filename);
        
        // JPEG doesn't support RGBA, convert to RGB
        if format == ImageFormat::Jpeg {
            let rgb_image = image::DynamicImage::ImageRgba8(self.data.clone())
                .into_rgb8();
            rgb_image.save_with_format(filename, format)?;
        } else {
            self.data.save_with_format(filename, format)?;
        }
        Ok(())
    }
    
    pub fn detect_format(filename: &str) -> ImageFormat {
        match Path::new(filename).extension().and_then(|s| s.to_str()) {
            Some("png") => ImageFormat::Png,
            Some("jpg") | Some("jpeg") => ImageFormat::Jpeg,
            Some("bmp") => ImageFormat::Bmp,
            Some("tga") => ImageFormat::Tga,
            _ => ImageFormat::Png,
        }
    }
    
    pub fn grayscale_avg_process(&mut self) {
        for pixel in self.data.pixels_mut() {
            let gray = ((pixel[0] as u16 + pixel[1] as u16 + pixel[2] as u16) / 3) as u8;
            pixel[0] = gray;
            pixel[1] = gray;
            pixel[2] = gray;
        }
    }
    
    pub fn grayscale_avg(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.grayscale_avg_process();
        if !self.filename.is_empty() {
            self.save_with_suffix("-grayscale-avg")
        } else {
            Ok(())
        }
    }
    
    pub fn grayscale_lum_process(&mut self) {
        for pixel in self.data.pixels_mut() {
            let gray = (0.2126 * pixel[0] as f32 + 0.7152 * pixel[1] as f32 + 0.0722 * pixel[2] as f32) as u8;
            pixel[0] = gray;
            pixel[1] = gray;
            pixel[2] = gray;
        }
    }
    
    pub fn grayscale_lum(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.grayscale_lum_process();
        if !self.filename.is_empty() {
            self.save_with_suffix("-grayscale-lum")
        } else {
            Ok(())
        }
    }
    
    pub fn color_mask_process(&mut self, r: f32, g: f32, b: f32) {
        for pixel in self.data.pixels_mut() {
            pixel[0] = (pixel[0] as f32 * r) as u8;
            pixel[1] = (pixel[1] as f32 * g) as u8;
            pixel[2] = (pixel[2] as f32 * b) as u8;
        }
    }
    
    pub fn color_mask(&mut self, r: f32, g: f32, b: f32) -> Result<(), Box<dyn std::error::Error>> {
        self.color_mask_process(r, g, b);
        if !self.filename.is_empty() {
            self.save_with_suffix("-color-mask")
        } else {
            Ok(())
        }
    }
    
    fn save_with_suffix(&self, suffix: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = Path::new(&self.filename);
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let stem = path.file_stem().unwrap().to_string_lossy();
        let ext = path.extension().unwrap().to_string_lossy();
        let output = parent.join(format!("{}{}.{}", stem, suffix, ext));
        self.write(output.to_str().unwrap())
    }
    
    pub fn width(&self) -> u32 {
        self.data.width()
    }
    
    pub fn height(&self) -> u32 {
        self.data.height()
    }
    
    pub fn pixel_at(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x < self.data.width() && y < self.data.height() {
            let pixel = self.data.get_pixel(x, y);
            Some([pixel[0], pixel[1], pixel[2], pixel[3]])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blank_image_creation() {
        let img = Image::blank(100, 100, 4);
        assert_eq!(img.width(), 100);
        assert_eq!(img.height(), 100);
        
        let pixel = img.pixel_at(0, 0);
        assert!(pixel.is_some());
    }

    #[test]
    fn test_blank_image_custom_size() {
        let img = Image::blank(50, 75, 4);
        assert_eq!(img.width(), 50);
        assert_eq!(img.height(), 75);
    }

    #[test]
    fn test_image_load_from_file() {
        let result = Image::new("src/test.png");
        assert!(result.is_ok(), "Failed to load test.png");
        
        let img = result.unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_image_load_jpg_from_file() {
        let result = Image::new("src/test2.jpg");
        assert!(result.is_ok(), "Failed to load test2.jpg");
        
        let img = result.unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_image_copy_clone() {
        let original = Image::blank(50, 50, 4);
        let _copy = Image::blank(50, 50, 4);
        
        assert_eq!(original.width(), 50);
        assert_eq!(original.height(), 50);
    }

    #[test]
    fn test_grayscale_avg_process() {
        let mut img = Image::new("src/test2.jpg").unwrap();
        let original_width = img.width();
        let original_height = img.height();
        
        img.grayscale_avg_process();
        
        assert_eq!(img.width(), original_width);
        assert_eq!(img.height(), original_height);
    }

    #[test]
    fn test_grayscale_lum_process() {
        let mut img = Image::new("src/test2.jpg").unwrap();
        let original_width = img.width();
        let original_height = img.height();
        
        img.grayscale_lum_process();
        
        assert_eq!(img.width(), original_width);
        assert_eq!(img.height(), original_height);
    }

    #[test]
    fn test_color_mask_process() {
        let mut img = Image::new("src/test2.jpg").unwrap();
        let original_width = img.width();
        let original_height = img.height();
        
        img.color_mask_process(1.0, 1.60, 1.70);
        
        assert_eq!(img.width(), original_width);
        assert_eq!(img.height(), original_height);
    }


    #[test]
    fn test_color_mask_value_changes() {
        let mut img = Image::blank(10, 10, 4);
        
        img.color_mask_process(0.5, 0.5, 0.5);
        
        assert_eq!(img.width(), 10);
        assert_eq!(img.height(), 10);
    }

    #[test]
    fn test_image_write() {
        let img = Image::blank(50, 50, 4);
        let result = img.write("test_output.png");
        
        assert!(result.is_ok(), "Failed to write image");
        
        assert!(Path::new("test_output.png").exists());
        
        std::fs::remove_file("test_output.png").ok();
    }

    #[test]
    fn test_detect_format_png() {
        let format = Image::detect_format("image.png");
        assert_eq!(format, ImageFormat::Png);
    }

    #[test]
    fn test_detect_format_jpg() {
        let format = Image::detect_format("image.jpg");
        assert_eq!(format, ImageFormat::Jpeg);
        
        let format2 = Image::detect_format("image.jpeg");
        assert_eq!(format2, ImageFormat::Jpeg);
    }

    #[test]
    fn test_detect_format_bmp() {
        let format = Image::detect_format("image.bmp");
        assert_eq!(format, ImageFormat::Bmp);
    }

    #[test]
    fn test_detect_format_default() {
        // Unknown format should default to PNG
        let format = Image::detect_format("image.unknown");
        assert_eq!(format, ImageFormat::Png);
    }
}
