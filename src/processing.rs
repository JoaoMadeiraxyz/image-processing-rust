use image::{ImageBuffer, Rgba, open, ImageFormat};
use pixo::ColorType;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Image {
    data: ImageBuffer<Rgba<u8>, Vec<u8>>,
    filename: String,
}

pub trait Filter {
    fn apply(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32);
}

pub struct SaturationFilter(pub f32);
pub struct ContrastFilter(pub f32);
pub struct ColorMaskFilter(pub f32, pub f32, pub f32);
pub struct BrightnessFilter(pub f32);
pub struct InvertFilter;

impl Filter for SaturationFilter {
    fn apply(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        let luma = 0.299 * r + 0.587 * g + 0.114 * b;
        (
            luma + (r - luma) * self.0,
            luma + (g - luma) * self.0,
            luma + (b - luma) * self.0,
        )
    }
}

impl Filter for ContrastFilter {
    fn apply(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        let cf = (259.0 * (self.0 * 255.0 + 255.0)) / (255.0 * (259.0 - self.0 * 255.0));
        let apply = |v: f32| (cf * (v * 255.0 - 128.0) + 128.0) / 255.0;
        (apply(r), apply(g), apply(b))
    }
}

impl Filter for ColorMaskFilter {
    fn apply(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        (r * self.0, g * self.1, b * self.2)
    }
}

impl Filter for BrightnessFilter {
    fn apply(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        (r + self.0, g + self.0, b + self.0)
    }
}

impl Filter for InvertFilter {
    fn apply(&self, r: f32, g: f32, b: f32) -> (f32, f32, f32) {
        (1.0 - r, 1.0 - g, 1.0 - b)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub struct FilterParams {
    pub brightness_on: bool,
    pub brightness: f32,
    pub contrast_on: bool,
    pub contrast: f32,
    pub saturation_on: bool,
    pub saturation: f32,
    pub color_mask_on: bool,
    pub mask: [f32; 3],
    pub invert_on: bool,
    pub scale_on: bool,
    pub scale_width: i16,
    pub scale_height: i16,
    pub compress_on: bool,
    pub compress_effort: u8,
}

impl Default for FilterParams {
    fn default() -> Self {
        FilterParams {
            brightness_on: false,
            brightness: 0.0,
            contrast_on: false,
            contrast: 0.0,
            saturation_on: false,
            saturation: 1.0,
            color_mask_on: false,
            mask: [1.0, 1.0, 1.0],
            invert_on: false,
            scale_on: false,
            scale_width: 0,
            scale_height: 0,
            compress_on: false,
            compress_effort: 1,
        }
    }
}

impl FilterParams {
    pub fn build(&self) -> Vec<Box<dyn Filter>> {
        let mut filters: Vec<Box<dyn Filter>> = Vec::new();
        if self.brightness_on {
            filters.push(Box::new(BrightnessFilter(self.brightness)));
        }
        if self.contrast_on {
            filters.push(Box::new(ContrastFilter(self.contrast)));
        }
        if self.saturation_on {
            filters.push(Box::new(SaturationFilter(self.saturation)));
        }
        if self.color_mask_on {
            filters.push(Box::new(ColorMaskFilter(self.mask[0], self.mask[1], self.mask[2])));
        }
        if self.invert_on {
            filters.push(Box::new(InvertFilter));
        }
        filters
    }

    pub fn any_enabled(&self) -> bool {
        self.brightness_on
            || self.contrast_on
            || self.saturation_on
            || self.color_mask_on
            || self.invert_on
    }
}

impl Image {
    pub fn new(filename: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let filename = filename.as_ref();
        let data = open(filename)?;
        let rgba_image = data.to_rgba8();
        Ok(Image {
            data: rgba_image,
            filename: filename.to_string_lossy().into_owned(),
        })
    }

    pub fn write(&self, filename: impl AsRef<Path>) -> Result<(), Box<dyn std::error::Error>> {
        let filename = filename.as_ref();
        let format = Self::detect_format(filename);

        if format == ImageFormat::Jpeg {
            let rgb_image = image::DynamicImage::ImageRgba8(self.data.clone())
                .into_rgb8();
            rgb_image.save_with_format(filename, format)?;
        } else {
            self.data.save_with_format(filename, format)?;
        }
        Ok(())
    }

    pub fn detect_format(filename: impl AsRef<Path>) -> ImageFormat {
        let ext = filename
            .as_ref()
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase());

        match ext.as_deref() {
            Some("png") => ImageFormat::Png,
            Some("jpg") | Some("jpeg") => ImageFormat::Jpeg,
            Some("bmp") => ImageFormat::Bmp,
            Some("tga") => ImageFormat::Tga,
            _ => ImageFormat::Png,
        }
    }

    pub fn process_filters(&mut self, filters: &[&dyn Filter]) {
        for pixel in self.data.pixels_mut() {
            let (mut r, mut g, mut b) = (
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
            );

            for filter in filters {
                (r, g, b) = filter.apply(r, g, b);
            }

            pixel[0] = (r.clamp(0.0, 1.0) * 255.0) as u8;
            pixel[1] = (g.clamp(0.0, 1.0) * 255.0) as u8;
            pixel[2] = (b.clamp(0.0, 1.0) * 255.0) as u8;
        }
    }

    pub fn process_params(&mut self, params: &FilterParams) {
        let owned = params.build();
        let refs: Vec<&dyn Filter> = owned.iter().map(|f| f.as_ref()).collect();
        self.process_filters(&refs);
    }

    pub fn width(&self) -> u32 {
        self.data.width()
    }

    pub fn height(&self) -> u32 {
        self.data.height()
    }

    pub fn filename(&self) -> &str {
        &self.filename
    }

    pub fn as_rgba_bytes(&self) -> &[u8] {
        self.data.as_raw()
    }

    pub fn thumbnail(&self, max_edge: u32) -> Image {
        let (w, h) = (self.data.width(), self.data.height());
        let longest = w.max(h);

        if longest <= max_edge || longest == 0 {
            return self.clone();
        }

        let scale = max_edge as f32 / longest as f32;
        let tw = ((w as f32 * scale).round() as u32).max(1);
        let th = ((h as f32 * scale).round() as u32).max(1);

        Image {
            data: image::imageops::thumbnail(&self.data, tw, th),
            filename: self.filename.clone(),
        }
    }

    pub fn write_compressed(
        &self,
        filename: impl AsRef<Path>,
        effort: u8,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        const BASE_QUALITY: u8 = 85;

        let path = filename.as_ref().with_extension("jpg");
        let preset = effort.min(2);

        let rgb = image::DynamicImage::ImageRgba8(self.data.clone()).into_rgb8();
        let (width, height) = rgb.dimensions();
        let pixels = rgb.as_raw();

        let encode_at = |q: u8| -> pixo::Result<Vec<u8>> {
            let options = pixo::jpeg::JpegOptions::builder(width, height)
                .color_type(ColorType::Rgb)
                .quality(q)
                .preset(preset)
                .progressive(false)
                .build();
            pixo::jpeg::encode(pixels, &options)
        };

        let mut bytes = encode_at(BASE_QUALITY)?;

        if let Ok(original_size) = std::fs::metadata(&self.filename).map(|m| m.len()) {
            if bytes.len() as u64 >= original_size {
                let (mut low, mut high) = (1i32, BASE_QUALITY as i32 - 1);
                let mut best: Option<Vec<u8>> = None;
                while low <= high {
                    let mid = low + (high - low) / 2;
                    let candidate = encode_at(mid as u8)?;
                    if (candidate.len() as u64) < original_size {
                        best = Some(candidate);
                        low = mid + 1;
                    } else {
                        high = mid - 1;
                    }
                }
                bytes = match best {
                    Some(b) => b,
                    None => encode_at(1)?,
                };
            }
        }

        std::fs::write(&path, &bytes)?;
        Ok(path)
    }
}

#[cfg(test)]
impl Image {
    pub fn blank(width: u32, height: u32, _channels: u32) -> Self {
        Image {
            data: ImageBuffer::new(width, height),
            filename: String::new(),
        }
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
        let format = Image::detect_format("image.unknown");
        assert_eq!(format, ImageFormat::Png);
    }

    #[test]
    fn test_thumbnail_caps_longest_edge_and_keeps_aspect() {
        let img = Image::new("src/test2.jpg").unwrap();
        assert_eq!((img.width(), img.height()), (4001, 5000));

        let thumb = img.thumbnail(800);

        assert_eq!(thumb.height(), 800, "longest edge should be capped");
        assert_eq!(thumb.width(), 640, "4001/5000 * 800 rounds to 640");
        assert_eq!((img.width(), img.height()), (4001, 5000));
    }

    #[test]
    fn test_thumbnail_does_not_upscale() {
        let img = Image::blank(50, 20, 4);
        let thumb = img.thumbnail(800);
        assert_eq!((thumb.width(), thumb.height()), (50, 20));
    }

    #[test]
    fn test_as_rgba_bytes_length_matches_egui_expectation() {
        let img = Image::blank(7, 5, 4);
        assert_eq!(img.as_rgba_bytes().len(), 7 * 5 * 4);
    }

    #[test]
    fn test_brightness_filter_is_additive() {
        let (r, g, b) = BrightnessFilter(0.25).apply(0.5, 0.5, 0.5);
        assert!((r - 0.75).abs() < 1e-6);
        assert!((g - 0.75).abs() < 1e-6);
        assert!((b - 0.75).abs() < 1e-6);
    }

    #[test]
    fn test_invert_filter() {
        let (r, g, b) = InvertFilter.apply(0.0, 0.25, 1.0);
        assert!((r - 1.0).abs() < 1e-6);
        assert!((g - 0.75).abs() < 1e-6);
        assert!((b - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_process_filters_clamps_out_of_range_results() {
        let mut img = Image::blank(4, 4, 4);
        img.process_filters(&[&BrightnessFilter(5.0)]);
        assert_eq!(img.pixel_at(0, 0).unwrap()[0], 255);

        let mut img = Image::blank(4, 4, 4);
        img.process_filters(&[&BrightnessFilter(-5.0)]);
        assert_eq!(img.pixel_at(0, 0).unwrap()[0], 0);
    }

    #[test]
    fn test_filter_params_default_is_a_no_op() {
        let params = FilterParams::default();
        assert!(!params.any_enabled());
        assert!(params.build().is_empty());

        let mut img = Image::new("src/test.png").unwrap();
        let before = img.pixel_at(10, 10).unwrap();
        img.process_params(&params);
        assert_eq!(img.pixel_at(10, 10).unwrap(), before);
    }

    #[test]
    fn test_filter_params_builds_only_enabled_filters() {
        let mut params = FilterParams::default();
        params.saturation_on = true;
        params.invert_on = true;

        assert!(params.any_enabled());
        assert_eq!(params.build().len(), 2);
    }

    #[test]
    fn test_process_params_matches_manual_filter_chain() {
        let mut params = FilterParams::default();
        params.saturation_on = true;
        params.saturation = 1.4;
        params.contrast_on = true;
        params.contrast = 0.15;

        let source = Image::new("src/test.png").unwrap();

        let mut via_params = source.clone();
        via_params.process_params(&params);

        let mut via_chain = source.clone();
        via_chain.process_filters(&[&ContrastFilter(0.15), &SaturationFilter(1.4)]);

        assert_eq!(via_params.as_rgba_bytes(), via_chain.as_rgba_bytes());
    }

    #[test]
    fn test_full_resolution_save_round_trip() {
        let source = Image::new("src/test.png").unwrap();
        let (w, h) = (source.width(), source.height());

        let mut params = FilterParams::default();
        params.invert_on = true;

        let mut out = source.clone();
        out.process_params(&params);

        let target = std::env::temp_dir().join("ipr-full-res-save-test.png");
        out.write(&target).expect("write should succeed");

        let reloaded = Image::new(&target).unwrap();
        assert_eq!(
            (reloaded.width(), reloaded.height()),
            (w, h),
            "saved file must be full resolution, not a preview"
        );
        assert_ne!(reloaded.pixel_at(0, 0), source.pixel_at(0, 0));

        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn test_write_to_jpeg_drops_alpha_without_error() {
        let img = Image::blank(16, 16, 4);
        let target = std::env::temp_dir().join("ipr-jpeg-write-test.jpg");
        assert!(img.write(&target).is_ok());
        assert_eq!(Image::new(&target).unwrap().width(), 16);
        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn test_write_compressed_forces_jpg_extension() {
        let img = Image::new("src/test.png").unwrap();
        let requested = std::env::temp_dir().join("ipr-compress-ext-test.png");
        std::fs::remove_file(&requested).ok();

        let written = img.write_compressed(&requested, 1).expect("compress should succeed");

        assert_eq!(written.extension().unwrap(), "jpg");
        assert!(written.exists(), "the .jpg file should exist");
        assert!(!requested.exists(), "the originally-requested .png path must not be created");

        std::fs::remove_file(&written).ok();
    }

    #[test]
    fn test_write_compressed_output_is_decodable_at_full_resolution() {
        let img = Image::new("src/test.png").unwrap();
        let (w, h) = (img.width(), img.height());
        let target = std::env::temp_dir().join("ipr-compress-decode-test.jpg");

        let written = img.write_compressed(&target, 1).unwrap();
        let reloaded = Image::new(&written).expect("compressed output should be decodable");

        assert_eq!((reloaded.width(), reloaded.height()), (w, h));
        std::fs::remove_file(&written).ok();
    }

    #[test]
    fn test_write_compressed_max_effort_is_never_larger_than_fast() {
        let img = Image {
            data: Image::new("src/test2.jpg").unwrap().data,
            filename: String::new(),
        };
        let fast = std::env::temp_dir().join("ipr-compress-effort-fast.jpg");
        let max = std::env::temp_dir().join("ipr-compress-effort-max.jpg");

        img.write_compressed(&fast, 0).unwrap();
        img.write_compressed(&max, 2).unwrap();

        let fast_size = std::fs::metadata(&fast).unwrap().len();
        let max_size = std::fs::metadata(&max).unwrap().len();

        assert!(
            max_size <= fast_size,
            "effort 2 ({max_size} bytes) should be no larger than effort 0 ({fast_size} bytes)"
        );

        std::fs::remove_file(&fast).ok();
        std::fs::remove_file(&max).ok();
    }

    #[test]
    fn test_write_compressed_never_exceeds_original_file_size() {
        let img = Image::new("src/test2.jpg").unwrap();
        let original_size = std::fs::metadata("src/test2.jpg").unwrap().len();
        let target = std::env::temp_dir().join("ipr-compress-never-bigger-test.jpg");

        let written = img.write_compressed(&target, 2).unwrap();
        let compressed_size = std::fs::metadata(&written).unwrap().len();

        assert!(
            compressed_size < original_size,
            "compressed size ({compressed_size} bytes) must be smaller than the original ({original_size} bytes)"
        );

        std::fs::remove_file(&written).ok();
    }

    #[test]
    fn test_write_compressed_skips_size_guarantee_without_a_source_file() {
        let img = Image::blank(64, 64, 4);
        let target = std::env::temp_dir().join("ipr-compress-no-source-test.jpg");

        assert!(img.write_compressed(&target, 2).is_ok());
        std::fs::remove_file(&target).ok();
    }

    #[test]
    fn test_write_compressed_max_effort_output_matches_source_pixels() {
        let img = Image::new("src/test2.jpg").unwrap();
        let target = std::env::temp_dir().join("ipr-compress-max-effort-pixel-test.jpg");

        let written = img.write_compressed(&target, 2).unwrap();
        let reloaded = Image::new(&written).expect("compressed output should be decodable");

        assert_eq!((reloaded.width(), reloaded.height()), (img.width(), img.height()));

        let (w, h) = (img.width(), img.height());
        let samples_x = 10.min(w);
        let samples_y = 10.min(h);
        let mut max_channel_diff = 0i32;

        for sx in 0..samples_x {
            for sy in 0..samples_y {
                let x = sx * (w - 1) / samples_x.max(1);
                let y = sy * (h - 1) / samples_y.max(1);
                let original = img.pixel_at(x, y).unwrap();
                let compressed = reloaded.pixel_at(x, y).unwrap();
                for channel in 0..3 {
                    let diff = (original[channel] as i32 - compressed[channel] as i32).abs();
                    max_channel_diff = max_channel_diff.max(diff);
                }
            }
        }

        assert!(
            max_channel_diff < 100,
            "max effort compression should not desync the JPEG bitstream; \
             largest sampled per-channel difference was {max_channel_diff}, \
             far beyond what lossy quantization alone would produce"
        );

        std::fs::remove_file(&written).ok();
    }

    #[test]
    fn test_write_compressed_clamps_out_of_range_effort() {
        let img = Image::blank(8, 8, 4);
        let target = std::env::temp_dir().join("ipr-compress-clamp-test.jpg");

        assert!(img.write_compressed(&target, 200).is_ok(), "effort above 2 should clamp, not error");

        std::fs::remove_file(&target).ok();
    }
}
