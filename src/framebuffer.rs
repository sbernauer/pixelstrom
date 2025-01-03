use colorgrad::Gradient;

use crate::proto::ScreenSync;

#[derive(Debug)]
pub struct FrameBuffer {
    width: u32,
    height: u32,
    pixels: Vec<u32>,
}

impl FrameBuffer {
    pub fn new(width: u32, height: u32) -> Self {
        let pixels = vec![0; width as usize * height as usize];

        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn num_pixels(&self) -> u32 {
        self.width * self.height
    }

    // pub fn fill_with_random_color(&mut self) {
    //     let color = rand::random::<u32>();
    //     self.pixels.fill(color);
    //     // self.pixels = (0..self.num_pixels()).map(|index| index).collect();
    //     // self.pixels.fill(color);
    // }

    pub fn fill_with_rainbow(&mut self) {
        let gradient = colorgrad::preset::sinebow();
        let start = rand::random::<f32>();
        let end = rand::random::<f32>();

        let step_size = (end - start) / self.num_pixels() as f32;

        self.pixels = (0..self.num_pixels())
            .map(|pixel_idx| {
                let [r, g, b, _] = gradient.at(start + pixel_idx as f32 * step_size).to_rgba8();
                r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
                // todo!()
            })
            .collect();
    }
}

impl From<&FrameBuffer> for ScreenSync {
    fn from(value: &FrameBuffer) -> Self {
        // TODO: Find more efficient way that works across all endianness
        let pixels = value
            .pixels
            .iter()
            .flat_map(|pixel| pixel.to_le_bytes())
            .collect();

        Self {
            width: value.width,
            height: value.height,
            pixels,
        }
    }
}
