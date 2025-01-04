use colorgrad::Gradient;
use prost::bytes::BufMut;

use crate::proto::{web_socket_message::Payload, ClientPainting, ScreenSync, WebSocketMessage};

#[derive(Debug)]
pub struct FrameBuffer {
    width: u16,
    height: u16,
    pixels: Vec<u32>,
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

    // pub fn width(&self) -> u32 {
    //     self.width
    // }

    // pub fn height(&self) -> u32 {
    //     self.height
    // }

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

    // /// Sets the rgba value for the given pixel if it exists
    // ///
    // /// The function does nothing in case the pixel does not exist (because x or y is outside of screen)
    // #[inline(always)]
    // pub fn set_no_client_update(&mut self, x: u16, y: u16, rgba: u32) {
    //     if x < self.width && y < self.height {
    //         let index = self.index(x, y);
    //         self.pixels[index] = rgba;
    //     }
    // }

    /// Sets the rgba value for the given pixel and sends a websocket update
    ///
    /// See [`set_no_client_update`] for other considerations, such as out of screen handling
    #[inline(always)]
    pub fn set_client_update(
        &mut self,
        x: u16,
        y: u16,
        rgba: u32,
        client: String,
    ) -> Option<WebSocketMessage> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let index = self.index(x, y);
        self.pixels[index] = rgba;

        let mut painted = Vec::new();
        painted.put_u16(x);
        painted.put_u16(y);
        painted.put_u32(rgba);
        Some(WebSocketMessage {
            payload: Some(Payload::ClientPainting(ClientPainting { client, painted })),
        })
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
