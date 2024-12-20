use crate::frame::FrameBuffer;

pub trait Emulator {
  fn new() -> Self;
  fn load(&mut self, rom: &[u8]);

  fn step_one_frame(&mut self);
  fn step_one_sample(&mut self) -> i16;
  fn get_framebuf(&self) -> &FrameBuffer;
  fn get_audiobuf(&self) -> &Vec<i16>;

  fn button_pressed(&mut self, button: PlayerInput);
  fn button_released(&mut self, button: PlayerInput);

  fn pause(&mut self);
  fn reset(&mut self);

  // fn save(&self);
  // fn load(&mut self);
}

pub enum PlayerInput {
  Up, Down, Left, Right, A, B, Start, Select,
  Pause, Reset, Save, Load, Mute,
}