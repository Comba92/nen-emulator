#[derive(Default, serde::Serialize, serde::Deserialize)]
pub enum IrqMode { 
  #[default] Mode0, // Scanline 
  Mode1             // Cycle
}

// https://www.nesdev.org/wiki/VRC_IRQ
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct KonamiIrq {
  pub prescaler: isize,
  pub count: u16,
  pub latch: u16,
  pub enabled_after_ack: bool,
  pub enabled: bool,
  pub mode: IrqMode,
  pub requested: Option<()>,
}

impl KonamiIrq {
  pub fn write_ctrl(&mut self, val: u8) {
    self.enabled_after_ack = val & 0b001 != 0;
    self.enabled = val & 0b010 != 0;
    self.mode = match val & 0b100 != 0 {
      false => IrqMode::Mode0,
      true  => IrqMode::Mode1,
    };

    self.requested = None;
    if self.enabled {
      self.count = self.latch;
      self.prescaler = 341;
    }
  }

  pub fn write_ack(&mut self) {
    self.requested = None;
    self.enabled = self.enabled_after_ack;    
  }
}