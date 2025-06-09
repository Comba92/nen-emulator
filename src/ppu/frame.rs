use std::sync::LazyLock;

#[derive(Debug)]
pub struct RGBColor(pub u8, pub u8, pub u8);

pub static SYS_COLORS: LazyLock<[RGBColor; 64]> = LazyLock::new(|| {
  let bytes = include_bytes!("../../palettes/Composite_wiki.pal");

  let colors: Vec<RGBColor> = bytes
    .chunks(3)
    // we take only the first palette set of 64 colors, more might be in a .pal file
    .take(64)
    .map(|rgb| RGBColor(rgb[0], rgb[1], rgb[2]))
    .collect();

  colors.try_into().unwrap()
});

pub const GREYSCALE_PALETTE: [u8; 4] = [0x3F, 0x00, 0x10, 0x20];

pub struct FramebufIndexed;
pub struct FramebufRGBA;

// TODO: Buffer can probably be a constant size array
pub struct FrameBuffer<T> {
  pub buffer: Box<[u8]>,
  pub width: usize,
  pub height: usize,
  kind: std::marker::PhantomData<T>,
}

impl Default for FrameBuffer<FramebufRGBA> {
  fn default() -> Self {
    FrameBuffer::nes_rgba_frame()
  }
}

impl Default for FrameBuffer<FramebufIndexed> {
  fn default() -> Self {
    FrameBuffer::nes_indexed_frame()
  }
}

impl<T> FrameBuffer<T> {
  pub fn new(width: usize, height: usize, pixel_size: usize) -> Self {
    let buffer = vec![0x0f; width * height * pixel_size].into_boxed_slice();
    Self {
      buffer,
      width,
      height,
      kind: std::marker::PhantomData::<T>,
    }
  }

  pub fn nes_rgba_frame() -> Self {
    FrameBuffer::new(SCREEN_WIDTH * 8, SCREEN_HEIGHT * 8, 4)
  }

  pub fn nes_indexed_frame() -> Self {
    FrameBuffer::new(SCREEN_WIDTH * 8, SCREEN_HEIGHT * 8, 1)
  }
}

impl FrameBuffer<FramebufRGBA> {
  pub fn set_pixel(&mut self, x: usize, y: usize, color_id: u8) {
    let color = &SYS_COLORS[color_id as usize];
    let idx = (y * self.width + x) * 4;
    self.buffer[idx + 0] = color.0;
    self.buffer[idx + 1] = color.1;
    self.buffer[idx + 2] = color.2;
    self.buffer[idx + 3] = 255;
  }

  pub fn pitch(&self) -> usize {
    self.width * 4
  }
}

impl FrameBuffer<FramebufIndexed> {
  pub fn set_pixel(&mut self, x: usize, y: usize, color_id: u8) {
    let idx = y * self.width + x;
    self.buffer[idx] = color_id;
  }

  pub fn pitch(&self) -> usize {
    self.width
  }
}

pub const SCREEN_WIDTH: usize = 32;
pub const SCREEN_HEIGHT: usize = 30;
