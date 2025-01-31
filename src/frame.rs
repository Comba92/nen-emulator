use std::sync::LazyLock;

#[derive(Debug)]
pub struct RGBColor(pub u8, pub u8, pub u8);

pub static SYS_COLORS: LazyLock<[RGBColor; 64]> = LazyLock::new(|| {
  let bytes = include_bytes!("../palettes/Composite_wiki.pal");

  let colors: Vec<RGBColor> = bytes
    .chunks(3)
    // we take only the first palette set of 64 colors, more might be in a .pal file
    .take(64)
    .map(|rgb| RGBColor(rgb[0], rgb[1], rgb[2]))
    .collect();

  colors.try_into().unwrap()
});


pub const GREYSCALE_PALETTE: [u8; 4] = [0x3F, 0x00, 0x10, 0x20];

const PIXEL_BYTES: usize = 4;
pub struct FrameBuffer {
  pub buffer: Box<[u8]>,
  pub width: usize,
  pub height: usize,
}

impl Default for FrameBuffer {
  fn default() -> Self {
    FrameBuffer::nes_screen()
  }
}

impl FrameBuffer {
  pub fn new(width: usize, height: usize) -> Self {
    let buffer = vec![0; width * height * PIXEL_BYTES].into_boxed_slice();
    Self { buffer, width, height }
  }

  pub fn nes_screen() -> Self {
    FrameBuffer::new(SCREEN_WIDTH*8, SCREEN_HEIGHT*8)
  }

  pub fn pitch(&self) -> usize {
    self.width * PIXEL_BYTES
  }

  pub fn set_pixel(&mut self, x: usize, y: usize, color_id: u8) {
    let color = &SYS_COLORS[color_id as usize];
    let idx = (y*self.width + x) * PIXEL_BYTES;
    self.buffer[idx + 0] = color.0;
    self.buffer[idx + 1] = color.1;
    self.buffer[idx + 2] = color.2;
    self.buffer[idx + 3] = 255;
  }
}

pub const SCREEN_WIDTH: usize = 32;
pub const SCREEN_HEIGHT: usize = 30;
