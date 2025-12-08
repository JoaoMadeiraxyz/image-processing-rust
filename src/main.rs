mod image;

use image::Image;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Grayscale average test
    let mut test_grayscale_avg = Image::new("src/test2.jpg")?;
    test_grayscale_avg.grayscale_avg()?;
    
    // Grayscale luminance test
    let mut test_grayscale_lum = Image::new("src/test2.jpg")?;
    test_grayscale_lum.grayscale_lum()?;

    // Color mask test
    let mut test_color_mask = Image::new("src/test2.jpg")?;
    test_color_mask.color_mask(1.0, 1.60, 1.70)?;

    Ok(())
}
