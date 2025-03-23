use colorgrad::Gradient;
use prost::bytes::BufMut;

use crate::proto::{web_socket_message::Payload, ScreenSync, UserPainting, WebSocketMessage};

pub struct FrameBuffer {
    width: u16,
    height: u16,
    pixels: Vec<u32>,
}

pub struct PixelUpdate {
    pub x: u16,
    pub y: u16,
    pub rgba: u32,
}

impl FrameBuffer {
    pub fn new(width: u16, height: u16) -> Self {
        let pixels = vec![0; width as usize * height as usize];

        Self {
            width,
            height,
            pixels,
        }
    }

    pub fn width(&self) -> u16 {
        self.width
    }

    pub fn height(&self) -> u16 {
        self.height
    }

    #[inline(always)]
    pub fn num_pixels(&self) -> usize {
        self.width as usize * self.height as usize
    }

    #[inline(always)]
    const fn index(&self, x: u16, y: u16) -> usize {
        y as usize * self.width as usize + x as usize
    }

    /// Gets the rgba value for the given pixel if it exists
    ///
    /// The function returns [`None`] in case the pixel does not exist (because x or y is outside of screen)
    #[inline(always)]
    pub fn get(&self, x: u16, y: u16) -> Option<u32> {
        if x < self.width && y < self.height {
            self.pixels.get(self.index(x, y)).copied()
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn set_multi(
        &mut self,
        username: impl Into<String>,
        painted: &[PixelUpdate],
    ) -> WebSocketMessage {
        let mut painted_bytes = Vec::with_capacity(painted.len() * 8 /* bytes per pixel */);
        for PixelUpdate { x, y, rgba } in painted {
            let index = self.index(*x, *y);
            self.pixels[index] = *rgba;
            painted_bytes.put_u16(*x);
            painted_bytes.put_u16(*y);
            painted_bytes.put_u32(*rgba);
        }

        WebSocketMessage {
            payload: Some(Payload::UserPainting(UserPainting {
                username: username.into(),
                painted: painted_bytes,
            })),
        }
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
            width: value.width as u32,
            height: value.height as u32,
            pixels,
        }
    }
}
