use crate::{bus::{Bus, Banking, CpuHandler, IrqFlags}, emu::Mirroring, mapper::Mapper, utils::byte_set_lo};


// https://www.nesdev.org/wiki/VRC1
#[derive(Default)]
pub struct VRC1 {
  chr_bank0: u16,
  chr_bank1: u16,
}
impl Mapper for VRC1 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_page_to_last_bank(3);
    mem.banks.chr = Banking::new_chr(&mem.header, 2);

    Box::new(Self::default()) 
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr & 0xf000 {
      0x8000 => mem.banks.prg.set_page(0, val),
      0xa000 => mem.banks.prg.set_page(1, val),
      0xc000 => mem.banks.prg.set_page(2, val),
      0x9000 => {
        let mirroring = match val & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal,
        };
        mem.banks.vram.mirror(&mirroring);

        self.chr_bank0 = (self.chr_bank0 & 0xf) | ((val & 0x2) << 3);
        self.chr_bank1 = (self.chr_bank1 & 0xf) | ((val & 0x4) << 2);
        mem.banks.chr.set_page(0, self.chr_bank0);
        mem.banks.chr.set_page(1, self.chr_bank1);
      }
      0xe000 => {
        self.chr_bank0 = (self.chr_bank0 & 0x10) | (val & 0xf);
        mem.banks.chr.set_page(0, self.chr_bank0);
      }
      0xf000 => {
        self.chr_bank1 = (self.chr_bank1 & 0x10) | (val & 0xf);
        mem.banks.chr.set_page(1, self.chr_bank1);
      }
      _ => {}
    }
  }
}

// https://www.nesdev.org/wiki/VRC3
#[derive(Default)]
pub struct VRC3 {
  irq_count: u16,
  irq_latch: u16,
  irq_enabled: bool,
  irq_enable_on_ack: bool,
  irq_8bit_mode: bool,
}
impl Mapper for VRC3 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr >> 12 {
      0x8 => self.irq_latch = (self.irq_latch & !0x000f) | (((val & 0xf) as u16) << 0),
      0x9 => self.irq_latch = (self.irq_latch & !0x00f0) | (((val & 0xf) as u16) << 4),
      0xa => self.irq_latch = (self.irq_latch & !0x0f00) | (((val & 0xf) as u16) << 8),
      0xb => self.irq_latch = (self.irq_latch & !0xf000) | (((val & 0xf) as u16) << 12),
      0xc => {
        self.irq_enable_on_ack = val & 0x1 > 0;
        self.irq_enabled = val & 0x2 > 0;
        if self.irq_enabled {
          self.irq_count = self.irq_latch;
        }

        self.irq_8bit_mode = val & 0x4 > 0;
        mem.irq.remove(IrqFlags::MAPPER);
      }
      0xd => {
        self.irq_enabled = self.irq_enable_on_ack;
        mem.irq.remove(IrqFlags::MAPPER);
      }
      
      0xf => mem.banks.prg.set_page(0, val),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.irq_enabled {      
      if self.irq_8bit_mode {
        let next = (self.irq_count & 0xff) + 1;
        if next > 0xff {
          self.irq_count = byte_set_lo(self.irq_count, self.irq_latch as u8);
          mem.irq.insert(IrqFlags::MAPPER);
        }
        self.irq_count = byte_set_lo(self.irq_count, next as u8);
      } else {
        self.irq_count = self.irq_count.wrapping_add(1);
        if self.irq_count == 0 {
          self.irq_count = self.irq_latch;
          mem.irq.insert(IrqFlags::MAPPER);
        }
      }
    }
  }
}

mod vrc {
  use crate::bus::{self, IrqFlags};

  #[derive(Default)]
  // https://www.nesdev.org/wiki/VRC_IRQ
  // this fag still jitters in some games
  pub struct Irq {
    prescaler: i16,
    pub count: u8,
    pub latch: u8,
    pub enable_after_ack: bool,
    pub enabled: bool,
    pub mode_scanline: bool,
  }

  impl Irq {
    pub fn write_ctrl(&mut self, val: u8, mem: &mut bus::Bus) {
      self.enable_after_ack = val & 0x1 > 0;
      self.enabled = val & 0x2 > 0;
      self.mode_scanline = val & 0x4 == 0;

      if self.enabled {
        self.count = self.latch;
        self.prescaler = 341;
      }
      
      // Any write to this register will acknowledge the pending IRQ and reset the prescaler.
      mem.irq.remove(IrqFlags::MAPPER)
    }

    pub fn write_ack(&mut self, mem: &mut bus::Bus) {
      self.enabled = self.enable_after_ack;
      mem.irq.remove(IrqFlags::MAPPER);
    }

    pub fn step(&mut self, mem: &mut bus::Bus) {
      if !self.enabled { return; } 

      self.prescaler -= 3;
      if !self.mode_scanline || (self.mode_scanline && self.prescaler <= 0) {
        if self.count == 0xff {
          self.count = self.latch;
          mem.irq.insert(IrqFlags::MAPPER);
        } else {
          self.count += 1;
        }
        self.prescaler += 341;
      }
    }

    //   let mut clock = || {
    //     if self.count >= 0xff {
    //       self.count = self.latch;
    //       mem.irq.insert(IrqFlags::MAPPER);
    //     } else {
    //       self.count += 1;
    //     }
    //   };

    //   if self.mode_scanline {
    //     self.prescaler -= 3;
    //     if self.prescaler <= 0 {
    //       self.prescaler += 341;
    //       clock();
    //     }
    //   } else {
    //     clock();
    //   }
    // }
  }
}


// https://www.nesdev.org/wiki/VRC2_and_VRC4
#[derive(Default)]
pub struct VRC2_4 {
  irq: vrc::Irq,
  mapper: u16,
  submapper: u8,
  is_vrc2: bool,

  prg_swapped: u8,
  prg_bank: u16,
  chr_regs: [u16; 8],

  latch: u8,
}
impl VRC2_4 {
  fn translate_address(&self, addr: u16) -> u16 {
    // The primary difference between them was having the mapper address lines connected in different ways. In particular, two lines chosen from A0-A7 will be used to select registers. 

    let take_bits = |a0: u8, a1: u8| {
      ((addr >> a0) & 1, (addr >> a1) & 1)
    };

    let (a0, a1) = match (self.mapper, self.submapper) {
      (23, 1 | 3) => take_bits(0, 1),
      (22, 0) | (25, 1 | 3) => take_bits(1, 0),
      (21, 1) => take_bits(1, 2),
      (21, 2) => take_bits(6, 7),
      (25, 2) => take_bits(3, 2),
      (23, 2) => take_bits(2, 3),

      _ => unreachable!()
    };

    addr & 0xff00 | (a1 << 1) | a0
  }

  fn update_chr_banks(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    let reg_pair = (addr >> 12) - 0xb;
    // we can tell if it is low or high nibble by second bit
    let low_or_high = (addr >> 1) & 1;
    // multiply reg pair by two, add low or high
    let page = ((reg_pair) << 1) | low_or_high;

    let reg = &mut self.chr_regs[page as usize]; 
    
    if addr & 1 == 0 {
      // low
      *reg = (*reg & 0x1f0) | (val & 0xf);
    } else {
      // high
      let val = if self.is_vrc2 {
        // VRC2 only has 4 high bits of CHR select. $B003 bit 4 is ignored. 
        val & 0xf
      } else { val & 0x1f };

      *reg = (*reg & 0xf) | (val << 4);
    }

    if self.mapper == 22 {
      // On VRC2a (mapper 22), the low bit is ignored (right shift value by 1). 
      mem.banks.chr.set_page(page as u8, *reg >> 1);
    } else {
      mem.banks.chr.set_page(page as u8, *reg);
    }
  }
}
impl Mapper for VRC2_4 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    let last_bank = mem.banks.prg.banks_count-1;
    mem.banks.prg.set_page(2, last_bank-1);
    mem.banks.prg.set_page(3, last_bank);

    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    let is_vrc2 = matches!(
      (mem.header.mapper, mem.header.submapper),
      (22, 0) | (23, 3) | (25, 3)
    );

    if is_vrc2 && mem.wram.is_empty() {
      mem.set_wram_handlers(CpuHandler::Mapper);
    }

    // TODO: might have 2kb wram mirrored, we cant do that with 8kb handlers..

    Box::new(Self {
      mapper: mem.header.mapper,
      submapper: mem.header.submapper,
      is_vrc2,
      ..Default::default()
    })
  }

  fn cart_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    if self.is_vrc2 && matches!(addr, 0x6000..=0x6fff) {
      self.latch
    } else { mem.cpu_data_bus }
  }

  fn cart_write(&mut self, _: &mut Bus, addr: u16, val: u16) {
    if self.is_vrc2 && matches!(addr, 0x6000..=0x6fff) {
      self.latch = val as u8 & 1;
    }
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    let addr = self.translate_address(addr);
    match (addr & 0xf00f, self.is_vrc2) {
      (0x9002, false) => {
        mem.wram_enable(val & 0x1 > 0);

        let swap_mode = val & 0x2 > 0;
        let second_last_bank = mem.banks.prg.banks_count-2;
        if swap_mode {
          // the 8 KiB page at $8000 is fixed to the second-to-last 8 KiB in the ROM
          // the 8 KiB page at $C000 is controlled by the $800x register
          mem.banks.prg.set_page(0, second_last_bank);
          self.prg_swapped = 2;
        } else {
          // the 8 KiB page at $8000 is controlled by the $800x register
          // the 8 KiB page at $C000 is fixed to the second-to-last 8 KiB in the ROM
          self.prg_swapped = 0;
          mem.banks.prg.set_page(2, second_last_bank);
        }
        mem.banks.prg.set_page(self.prg_swapped, self.prg_bank);
      }

      (0x8000..=0x8003, _) => {
        self.prg_bank = val;
        mem.banks.prg.set_page(self.prg_swapped, val)
      }
      (0xa000..=0xa003, _) => mem.banks.prg.set_page(1, val),
      (0x9000..=0x9003, true) | (0x9000, false) => {
        let val = if self.is_vrc2 { val & 0b01 } else { val & 0b11};

        let mirroring = match val {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::LowTable,
          _ => Mirroring::HighTable,
        };
        mem.banks.vram.mirror(&mirroring);
      }

      (0xb000..=0xe003, _) => self.update_chr_banks(mem, addr, val),

      (0xf000, false) => self.irq.latch = (self.irq.latch & 0xf0) | (val as u8 & 0xf),
      (0xf001, false) => self.irq.latch = (self.irq.latch & 0x0f) | ((val as u8 & 0xf) << 4),
      (0xf002, false) => self.irq.write_ctrl(val as u8, mem),
      (0xf003, false) => self.irq.write_ack(mem),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if !self.is_vrc2 {
      self.irq.step(mem);
    }
  }
}

mod vrc6 {
  use crate::{apu, utils::{byte_set_hi, byte_set_lo}};

  #[derive(Default)]
  pub struct Pulse {
    enabled: bool,
    div: apu::DividerCounter,
    volume: u8,
    duty: u8,
    step: u8,
    ignore_duty: bool,
  }

  impl Pulse {
    pub fn write_ctrl(&mut self, val: u8) {
      self.volume = val & 0xf;
      self.duty = (val >> 4) & 0x7;
      self.ignore_duty = val & 0x80 > 0;
    }

    pub fn write_freq_lo(&mut self, val: u8) {
      self.div.period = byte_set_lo(self.div.period, val);
    }

    pub fn write_freq_hi(&mut self, val: u8, shift: u8) {
      self.div.period = byte_set_hi(self.div.period, val & 0xf);
      self.div.period >>= shift;

      self.enabled = val & 0x80 > 0;

      if !self.enabled {
        self.step = 0;
      }
    }

    pub fn step(&mut self) {
      self.div.step(|| {
        self.step = (self.step + 1) % 16;
      });
    }

    pub fn sample(&self) -> u8 {
      if self.enabled && (self.ignore_duty || self.step <= self.duty) {
        self.volume
      } else { 0 }
    }
  }

  #[derive(Default)]
  pub struct Saw {
    enabled: bool,
    rate: u8,
    acc: u8,
    count: u8,
    div: apu::DividerCounter,
  }
  impl Saw {
    pub fn write_ctrl(&mut self, val: u8) {
      self.rate = val & 0x3f;
    }

    pub fn write_freq_lo(&mut self, val: u8) {
      self.div.period = byte_set_lo(self.div.period, val);
    }

    pub fn write_freq_hi(&mut self, val: u8, shift: u8) {
      self.div.period = byte_set_hi(self.div.period, val & 0xf);
      self.div.period >>= shift;

      self.enabled = val & 0x80 > 0;
      if !self.enabled {
        self.acc = 0;
        self.count = 0;
      }
    }

    pub fn step(&mut self) {
      self.div.step(|| {
        self.count = (self.count + 1) % 14;
        
        if self.count == 0 {
          self.acc = 0;
        } else if self.count % 2 == 0 {
          // If A is more than 42 the accumulator will wrap, resulting in distorted sound. 
          self.acc = (self.acc + self.rate) % 42;
        }
      });
    }

    pub fn sample(&self) -> u8 {
      if self.enabled { self.acc >> 3 } else { 0 }
    }
  }
}

// https://www.nesdev.org/wiki/VRC6
#[derive(Default)]
pub struct VRC6 {
  mapper: u16,
  regs: [u16; 8],
  mode: u8,
  mirroring: u8,
  uses_chr_rom: bool,

  irq: vrc::Irq,

  audio_halt: bool,
  audio_freq_shift: u8,

  p0: vrc6::Pulse,
  p1: vrc6::Pulse,
  saw: vrc6::Saw,
}
impl VRC6 {
  fn update_chr_banks(&mut self, mem: &mut Bus) {
    let chr = &mut mem.banks.chr;

    // When bit 5 of $B003 is set, 2 KiB pattern table banks pass PPU A10 through (ignoring the LSB of the register).
    // So, mode 1, 2, 3 2kb banks should be contiguos.

    // When bit 5 of $B003 is clear, CHR/CIRAM A10 will be controlled directly by the register LSB, causing 2 KiB banks to have duplicate 1 KiB halves.
    // Existing Konami games did not use this configuration. 
    // This means 2kb map to the same bank. We are not emulating it.

    match self.mode {
      0 => for i in 0..8 {
        chr.set_page(i as u8, self.regs[i]);
      }

      // each register sets two pages
      1 => for i in 0..4 {
        chr.set_pages_aligned2(2 * i as u8, self.regs[i]);
      }

      _ => {
        for i in 0..4 {
          chr.set_page(i as u8, self.regs[i]);
        }
        // only r4 and r5 set two pages each
        chr.set_pages_aligned2(4, self.regs[4]);
        chr.set_pages_aligned2(6, self.regs[5]);
      }
    }
  }

  fn update_vram_banks(&mut self, mem: &mut Bus) {
    let vram = &mut mem.banks.vram;

    // When bit 5 of $B003 is set, 2 KiB pattern table banks pass PPU A10 through (ignoring the LSB of the register).
    // Nametables apply different rules at the same time: see below. 

    // Only mode 0 was used by Konami's commercial games.
    match self.mode {
      // This mode was not intended for use with ROM nametables ($B003:4 set), because it overrides the LSB of the nametable registers with the signal intended for CIRAM A10. 
      // Because R6 and R7 are already in use to control the pattern banks, this is not very suitable if combined with ROM nametables (Mode 3 is designed for that instead). 
      0 => {
        match self.mirroring {
          // Vertical
          0 => {
            vram.set_page(0, self.regs[6] & !1);
            vram.set_page(1, self.regs[6] | 1);
            vram.set_page(2, self.regs[7] & !1);
            vram.set_page(3, self.regs[7] | 1);
          }
          // Horizontal
          1 => {
            vram.set_page(0, self.regs[6] & !1);
            vram.set_page(1, self.regs[7] & !1);
            vram.set_page(2, self.regs[6] | 1);
            vram.set_page(3, self.regs[7] | 1);
          }
          // SingleScreenA
          2 => {
            vram.set_page(0, self.regs[6] & !1);
            vram.set_page(1, self.regs[6] & !1);
            vram.set_page(2, self.regs[7] & !1);
            vram.set_page(3, self.regs[7] & !1);
          }
          // SingleScreenB
          _ => {
            vram.set_page(0, self.regs[6] | 1);
            vram.set_page(1, self.regs[7] | 1);
            vram.set_page(2, self.regs[6] | 1);
            vram.set_page(3, self.regs[7] | 1);
          }
        }
      }
      _ => todo!("VRC6 modes 1, 2, 3")
    }
  }

  fn update_all_banks(&mut self, mem: &mut Bus) {
    self.update_chr_banks(mem);
    self.update_vram_banks(mem);
  }
}
impl Mapper for VRC6 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_pages_aligned2(0, 0);
    mem.banks.prg.set_page_to_last_bank(3);

    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    let mut res = Box::new(Self {
      mapper: mem.header.mapper,
      ..Default::default()
    });

    res.update_all_banks(mem);
    res
  }

  fn prg_write(&mut self, mem: &mut Bus, mut addr: u16, val: u16) {
    if self.mapper == 26 {
      addr = (addr & 0xfffc) | ((addr & 0x01) << 1) | ((addr & 0x02) >> 1);
    }

    let val = val as u8;
    match addr & 0xf003 {
      // be careful here: value passed here is missing lsb bit, so we have to shift it right
      0x8000..=0x8003 => mem.banks.prg.set_pages_aligned2(0, (val as u16) << 1),
      0xc000..=0xc003 => mem.banks.prg.set_page(2, val as u16),
      
      0xb003 => {
        self.mode = val & 0x3;
        self.mirroring = (val >> 2) & 0x3;

        // The VRC6 supports the use of a larger RAM to provide more nametables.
        // However, the three commercial VRC6 games neither provided extra nametable RAM, nor used ROM nametables.
        self.uses_chr_rom = val & 0x10 > 0;
        
        // The commercial games always left bit 5 set.

        self.update_all_banks(mem);

        mem.wram_enable(val & 0x80 > 0);
      }
      0xd000..=0xd003 => {
        self.regs[addr as usize - 0xd000] = val as u16;
        self.update_all_banks(mem);
      }
      0xe000..=0xe003 => {
        self.regs[addr as usize - 0xe000 + 4] = val as u16;
        self.update_all_banks(mem);
      }

      0xf000 => self.irq.latch = val,
      0xf001 => self.irq.write_ctrl(val, mem),
      0xf002 => self.irq.write_ack(mem),

      0x9003 => {
        self.audio_halt = val & 0x1 > 0;
        let audio_16x = val & 0x2 > 0;
        let audio_256x = val & 0x4 > 0;

        if !self.audio_halt {
          self.audio_freq_shift = 0;
        } else if audio_256x {
          self.audio_freq_shift = 8;
        } else if audio_16x {
          self.audio_freq_shift = 4;
        }
      }

      0x9000 => self.p0.write_ctrl(val),
      0x9001 => self.p0.write_freq_lo(val),
      0x9002 => self.p0.write_freq_hi(val, self.audio_freq_shift),

      0xa000 => self.p1.write_ctrl(val),
      0xa001 => self.p1.write_freq_lo(val),
      0xa002 => self.p1.write_freq_hi(val, self.audio_freq_shift),

      0xb000 => self.saw.write_ctrl(val),
      0xb001 => self.saw.write_freq_lo(val),
      0xb002 => self.saw.write_freq_hi(val, self.audio_freq_shift),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    self.irq.step(mem);

    if !self.audio_halt {
      self.p0.step();
      self.p1.step();
      self.saw.step();
    }
  }

  fn sample(&self) -> f32 {
    (self.p0.sample() + self.p1.sample() + self.saw.sample()) as f32
  }
}

// https://www.nesdev.org/wiki/VRC7
// TODO: incomplete
#[derive(Default)]
pub struct VRC7 {
  irq: vrc::Irq,
}
impl Mapper for VRC7 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_page_to_last_bank(3);
    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    let addr = if addr & 0x10 > 0 {
      // if we have 0x10, clear it and insert 0x8
      addr & !0x10 | 0x8
    } else { addr };

    match addr & 0xf00f {
      // LOL
      0x8000 => mem.banks.prg.set_page(0, val),
      0x8008 => mem.banks.prg.set_page(1, val),
      0x9000 => mem.banks.prg.set_page(2, val),
      0xa000 => mem.banks.chr.set_page(0, val),
      0xa008 => mem.banks.chr.set_page(1, val),
      0xb000 => mem.banks.chr.set_page(2, val),
      0xb008 => mem.banks.chr.set_page(3, val),
      0xc000 => mem.banks.chr.set_page(4, val),
      0xc008 => mem.banks.chr.set_page(5, val),
      0xd000 => mem.banks.chr.set_page(6, val),
      0xd008 => mem.banks.chr.set_page(7, val),

      0xe000 => {
        let mirroring = match val & 0x03 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::LowTable,
          _ => Mirroring::HighTable,
        };
        mem.banks.vram.mirror(&mirroring);

        // TODO: audio

        mem.wram_enable(val & 0x80 > 0);
      }
      0xe008 => self.irq.latch = val as u8,
      0xf000 => self.irq.write_ctrl(val as u8, mem),
      0xf008 => self.irq.write_ack(mem),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    self.irq.step(mem);
  }
}