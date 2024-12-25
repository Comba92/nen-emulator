#![allow(unused)]

use crate::dma::{Dma, DmcDma};
use super::{Channel, Timer};

const DMC_SEQUENCE: [u16; 16] = [
  428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106,  84,  72,  54
];

pub struct Dmc {
  pub irq_enabled: bool,
  pub irq_flag: Option<()>,
  pub loop_enabled: bool,
  timer: Timer,

  buffer_empty: bool,
  buffer: u8,
  level: u8,
  bits_remaining: u8,
  address: u16,
  length: u16,
  shift_reg: u8,
  silence: bool,

  pub reader: DmcDma,
}

impl Default for Dmc {
  fn default() -> Self {
    Self { irq_enabled: Default::default(), irq_flag: Default::default(), loop_enabled: Default::default(), timer: Default::default(), level: Default::default(), buffer_empty: true, buffer: Default::default(), bits_remaining: Default::default(), address: Default::default(), length: Default::default(), shift_reg: Default::default(), silence: true, reader: Default::default() }
  }
}


impl Dmc {
  pub fn write_ctrl(&mut self, val: u8) {
    self.irq_enabled = val & 0b1000_0000 != 0;
    self.loop_enabled = val & 0b0100_0000 != 0;
    self.timer.period = DMC_SEQUENCE[val as usize & 0b1111];

    if !self.irq_enabled {
      self.irq_flag = None;
    }
  }

  pub fn write_level(&mut self, val: u8) {
    let previous_level = self.level;
    let new_level = val & 0b0111_1111;

    // Reduce dmc popping
    if new_level.abs_diff(previous_level) <= 50 {
      self.level = new_level;
    } else {
      self.level = 50;
    }
  }

  pub fn write_addr(&mut self, val: u8) {
    self.address = 0xC000 | ((val as u16) << 6);
  }

  pub fn write_length(&mut self, val: u8) {
    self.length = ((val as u16) << 4) + 1;
  }

  pub fn load_sample(&mut self, sample: u8) {
    self.buffer = sample;
    self.buffer_empty = false;
    self.bits_remaining = 8;

    if !self.reader.is_transfering() {
      if self.loop_enabled {
        self.restart_dma();
      } else if self.irq_enabled {
        self.irq_flag = Some(());
      }
    }

    // if(self.length == 1 && !self.loop_enabled) {
    //   //When DMA ends around the time the bit counter resets, a CPU glitch sometimes causes another DMA to be requested immediately.
    //   if(self.bits_remaining == 8 && self.timer.count == self.timer.period) {
    //     self.shift_reg = self.buffer;
    //     self.silence = false;
    //     self.buffer_empty = true;
    //     self.restart_dma();
    //   } else if(self.bits_remaining == 1 && self.timer.count < 2) {
    //     self.shift_reg = self.buffer;
    //     self.buffer_empty = false;
    //     self.restart_dma();
    //   }
    // }
  }

  pub fn restart_dma(&mut self) {
    self.reader.init(self.address, self.length);
  }

  pub fn is_empty(&self) -> bool {
    self.buffer_empty
  }
}

impl Channel for Dmc {
  fn step_timer(&mut self) {
    self.timer.step(|_| {
      if !self.silence {
        if self.shift_reg & 1 != 0 {
          if self.level <= 125 { self.level += 2; }
        } else if self.level >= 2 {
          self.level -= 2;
        }
        self.shift_reg >>= 1;
      }

      self.bits_remaining -= 1;
      if self.bits_remaining == 0 {
        self.bits_remaining = 8;

        if self.buffer_empty {
          self.silence = true;
        } else {
          self.silence = false;
          self.shift_reg = self.buffer;
          self.buffer_empty = true;
        }
      }
    });
  }

  fn step_half(&mut self) {}
  fn step_quarter(&mut self) {}

  fn is_enabled(&self) -> bool {
    // D will read as 1 if the DMC bytes remaining is more than 0.
    self.reader.remaining > 0
  }

  fn set_enabled(&mut self, enabled: bool) {
    // If the DMC bit is clear, the DMC bytes remaining will be set to 0 and the DMC will silence when it empties.
    // If the DMC bit is set, the DMC sample will be restarted only if its bytes remaining is 0. If there are bits remaining in the 1-byte sample buffer, these will finish playing before the next sample is fetched
    if enabled {
      if self.reader.remaining == 0 { self.restart_dma(); }
    } else {
      self.reader.remaining = 0;
    }
  }

  // TODO: dmc causes noise
  fn get_sample(&self) -> u8 {
    self.level
  }
}