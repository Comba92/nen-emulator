use std::ops::Neg;
use crate::{apu, bus::{Banking, Bus, CpuHandler, IrqFlags, PpuHandler}, mapper::Mapper};

#[derive(Default, PartialEq, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
enum WramKind { #[default] SingleChip, DoubleChip16kb, DoubleChip64kb }

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MMC5 {
  ppu_substituion: bool,
  ppu_big_sprites: bool,

  prg_mode: u8,
  prg_regs: [u16; 5],

  chr_mode: u8,
  chr_regs: [u16; 12],
  chr_hi: u8,
  last_chr_wrote: u16,

  wram_kind: WramKind,
  wram_protect: u8,
  wram_writable: bool,

  exram_mode: u8,
  exattr_curr_val: u8,
  nametbl_fetch_count: usize,

  irq_enabled: bool,
  irq_pending: bool,
  irq_cmp: u16,
  irq_count: u16,
  ppu_in_frame: bool,

  // needed for correct irq
  irq_delay: u8,

  ppu_same_addr_count: usize,
  ppu_last_addr: Option<u16>,
  ppu_idle_countdown: usize,

  multiplicand: u8,
  multiplier: u8,
  product: u16,

  p0: apu::Pulse,
  p1: apu::Pulse,
}
impl MMC5 {
  fn update_prg_banks(&mut self, mem: &mut Bus) {
    let wram = &mut mem.banks.wram;
    let prg = &mut mem.banks.prg;

    let mut set_bank = |page, bank| {
      if bank & 0x80 > 0 {
        // rom
        prg.set_page(page - 1, bank & 0x7f);
        mem.cpu_handlers_8kb[3 + page as usize] = CpuHandler::PrgSpecial;
      } else {
        // ram

        // mutliple ram chips logic
        let block = (bank & 0x4 > 0) as u16;
        let bank = match self.wram_kind {
          WramKind::SingleChip => bank,
          WramKind::DoubleChip16kb => block,
          WramKind::DoubleChip64kb => (block * (wram.banks_count/2)) + (bank & 0x3),
        };
        wram.set_page(page, bank);

        let handler = if mem.wram.is_empty() || (self.wram_kind == WramKind::SingleChip && block > 0) {
          CpuHandler::Mapper
        } else if self.wram_writable {
          CpuHandler::Wram
        } else {
          CpuHandler::WramReadOnly
        };
        mem.cpu_handlers_8kb[3 + page as usize] = handler;
      }
    };

    // always on wram page 0
    // cut bit 7 as this has to be in wram
    set_bank(0, self.prg_regs[0] & 0x7f);

    // 5114 only in mode 3
    if self.prg_mode == 3 {
      set_bank(1, self.prg_regs[1]);
    }

    // 5115 in modes 1, 2, 3
    let reg5115 = self.prg_regs[2];
    if self.prg_mode == 3 {
      set_bank(2, reg5115);
    } else if matches!(self.prg_mode, 1 | 2) {
      set_bank(1, reg5115 & !1);
      set_bank(2, reg5115 | 1);
    }

    // 5116 in modes 2, 3
    if matches!(self.prg_mode, 2 | 3) {
      set_bank(3, self.prg_regs[3]);
    }

    // 5117 in all modes
    // always on rom, so forcefully set high bit to 1
    self.prg_regs[4] |= 0x80;
    let reg5117 = self.prg_regs[4];
    if matches!(self.prg_mode, 2 | 3) {
      set_bank(4, reg5117);
    } else if self.prg_mode == 1 {
      set_bank(3, reg5117 & !1);
      set_bank(4, reg5117 | 1);
    } else if self.prg_mode == 0 {
      let reg5117 = reg5117 & !0x3;

      set_bank(1, reg5117 | 0);
      set_bank(2, reg5117 | 1);
      set_bank(3, reg5117 | 2);
      set_bank(4, reg5117 | 3);
    }
  }

  // TODO: update chr banks only when necessary
  fn update_chr_banks(&mut self, mem: &mut Bus) {
    // in 8x8 sprites mode, in 16x8 sprites mode and rendering sprites, in vblank use last written low registers
    if !self.ppu_big_sprites {
      self.last_chr_wrote = 0;
    }
    
    let use_low_regs = !(self.ppu_big_sprites && self.ppu_substituion)
      || (self.nametbl_fetch_count >= 32 && self.nametbl_fetch_count < 48)
      || (!self.ppu_in_frame && self.last_chr_wrote <= 0x5127);

    // In ExAttributes mode, the values of the CHR banking registers $5120-$512B are ignored.
    if use_low_regs {
      self.update_chr_low_regs(mem);
    } else {
      self.update_chr_high_regs(mem);
    }
  }

  fn update_chr_low_regs(&mut self, mem: &mut Bus) {
    // Caution: Unlike the MMC1 and unlike PRG banking on the MMC5, the banks are always indexed by the currently selected size.
    // When using 2kb, 4kb or 8kb bank sizes, the registers hold bank index of that larger size, and lower bits are *not* ignored. 
    // shifting is needed
    let chr = &mut mem.banks.chr;
    match self.chr_mode {
      // 8kb
      0 => chr.set_pages_aligned8(0, self.chr_regs[7] << 3),
      // 4kb
      1 => {
        chr.set_pages_aligned4(0, self.chr_regs[3] << 2);
        chr.set_pages_aligned4(4, self.chr_regs[7] << 2);
      }
      // 2kb
      2 =>  for i in 0..4 {
        // only odds chr_regs
        chr.set_pages_aligned2(i, self.chr_regs[i as usize * 2 + 1] << 1);
      }
      // 1kb
      _ => for i in 0..8 {
        chr.set_page(i, self.chr_regs[i as usize]);
      }
    }
  }

  fn update_chr_high_regs(&mut self, mem: &mut Bus) {
    let chr = &mut mem.banks.chr;

    match self.chr_mode {
      // 8kb
      0 => chr.set_pages_aligned8(0, self.chr_regs[11] << 3),
      // 4kb
      1 => {
        chr.set_pages_aligned4(0, self.chr_regs[11] << 2);
        chr.set_pages_aligned4(4, self.chr_regs[11] << 2);
      }
      // 2kb
      2 =>  {
        chr.set_pages_aligned2(0, self.chr_regs[9] << 1);
        chr.set_pages_aligned2(2, self.chr_regs[11] << 1);
        chr.set_pages_aligned2(4, self.chr_regs[9] << 1);
        chr.set_pages_aligned2(6, self.chr_regs[11] << 1);
      }
      // 1kb
      _ => for i in 0..4 {
        let bank = self.chr_regs[8 + i as usize];
        chr.set_page(i, bank);
        chr.set_page(4 + i, bank);
      }
    }
  }

  fn reset_irq(&mut self, mem: &mut Bus) {
    self.ppu_in_frame = false;
    self.ppu_last_addr = None;
    self.irq_count = 0;
    mem.irq.remove(IrqFlags::MAPPER);
  }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for MMC5 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    // wram can be mapped in range 0x6000..=0xdfff (32kb)
    mem.banks.wram = Banking::new(0x6000, mem.header.wram_size, 32 * 1024, 4);
    mem.set_prg_handlers(CpuHandler::PrgSpecial);
    mem.cpu_handlers_8kb[1] = CpuHandler::PpuMMC5;

    // we simulate exram by extending vram to 4 screens
    // exram is mapped to third, fill screen is mapped to fourth
    mem.set_4screen_mirroring();
    // needed to substitute attribute tables reads
    mem.set_chr_handlers(PpuHandler::ChrMMC5);
    mem.set_vram_handlers(PpuHandler::VramMMC5);

    let mut res = Self::default();
    // The Koei games never write to this register, apparently relying on the MMC5 defaulting to mode 3 at power on. 
    res.prg_mode = 3;
    // All known games have their reset vector in the last bank of PRG ROM, and the vector points to an address greater than or equal to $E000.
    // This tells us that $5117 must have a reliable power-on value of $FF. 
    res.prg_regs = [0x80; 5];
    res.prg_regs[4] = 0xff;

    // https://www.nesdev.org/wiki/MMC5#PRG-RAM_configurations
    res.wram_kind = if mem.wram.len() == 16 * 1024 {
      WramKind::DoubleChip16kb
    } else if mem.wram.len() == 64 * 1024 {
      WramKind::DoubleChip64kb
    } else {
      WramKind::SingleChip
    };

    res.update_prg_banks(mem);
    res.update_chr_banks(mem);

    Box::new(res)
  }

  fn cart_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    match addr {
      0x5015 => {
        let mut res = 0;
        res |= ((self.p0.len.count > 0) as u8) << 0;
        res |= ((self.p1.len.count > 0) as u8) << 1;
        res
      }

      0x5204 => {
        let mut res = 0;
        res |= (self.irq_pending as u8) << 7;
        res |= (self.ppu_in_frame as u8) << 6;

        self.irq_pending = false;
        mem.irq.remove(IrqFlags::MAPPER);
        res
      }

      0x5205 => self.product as u8,
      0x5206 => (self.product >> 8) as u8,

      0x5c00..=0x5fff => {
        let exram_addr = 0x800 + (addr as usize - 0x5c00);
        match self.exram_mode {
          0 | 1 => mem.cpu_data_bus,
          // we simulate exram by storing it as the third nametable in vram
          _ => mem.vram[exram_addr]
        }
      }
      _ => mem.cpu_data_bus,
    }
  }

  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    match addr {
      0x5000 => self.p0.write_ctrl(val ),
      0x5002 => self.p0.write_timer_lo(val),
      0x5003 => self.p0.write_timer_hi(val),

      0x5004 => self.p1.write_ctrl(val),
      0x5006 => self.p1.write_timer_lo(val),
      0x5007 => self.p1.write_timer_hi(val),

      0x5015 => {
        self.p0.enable(val & 0x1 > 0);
        self.p1.enable(val & 0x2 > 0);
      }

      0x5100 => {
        self.prg_mode = val & 0x3;
        self.update_prg_banks(mem);
      }
      0x5101 => {
        self.chr_mode = val & 0x3;
        self.update_chr_banks(mem);
      }

      0x5102 => {
        self.wram_protect = (self.wram_protect & 0xc) | (val & 0x3);
        self.wram_writable = self.wram_protect == 0x6;
      }
      0x5103 => {
        self.wram_protect = (self.wram_protect & 0x3) | ((val & 0x3) << 2);
        self.wram_writable = self.wram_protect == 0x6;
      }

      0x5104 => self.exram_mode = val & 0x3,

      0x5105 => for i in 0..4 {
        let nametbl = (val >> (i * 2)) & 0x3;
        // exram is mapped to the third nametable, fill mode to fourth
        mem.banks.vram.set_page(i, nametbl as u16);
      }

      0x5106 => mem.vram[0xc00..0xfc0].fill(val),
      0x5107 => {
        // Each byte of the attribute table normally contains four 2-bit palette indexes. The two bits in this register are copied for all four indexes. 
        let color = val & 0x3;
        let attribute = (color << 6) | (color << 4) | (color << 2) | color;
        mem.vram[0xfc0..0x1000].fill(attribute);
      }

      // TODO: only update when necessary
      0x5113..=0x5117 => {
        let reg = addr as usize - 0x5113;
        self.prg_regs[reg] = val as u16;
        self.update_prg_banks(mem);
      }

      // TODO: only update when necessary
      0x5120..=0x512b => {
        let reg = addr as usize - 0x5120;
        self.chr_regs[reg] = ((self.chr_hi as u16) << 8) | val as u16;
        self.last_chr_wrote = addr;
        self.update_chr_banks(mem);
      }

      // no official game relies on this register, and most don't even initialize it. 
      0x5130 => self.chr_hi = val & 0x3,

      0x5203 => self.irq_cmp = val as u16,
      0x5204 => {
        self.irq_enabled = val & 0x80 > 0;
      
        if self.irq_enabled && self.irq_pending {
          mem.irq.insert(IrqFlags::MAPPER);
        } else if !self.irq_enabled {
          mem.irq.remove(IrqFlags::MAPPER);
        }
      }

      0x5205 => {
        self.multiplicand = val;
        self.product = self.multiplicand as u16 * self.multiplier as u16;
      }
      0x5206 => {
        self.multiplier = val;
        self.product = self.multiplicand as u16 * self.multiplier as u16;
      }

      0x5c00..=0x5fff => {
        // in mode 0 and 1, can only write during rendering
        // in mode 2, can always write
        let exram_addr = 0x800 + (addr as usize - 0x5c00);
        if matches!((self.exram_mode, self.ppu_in_frame), (0 | 1, true) | (2, _)) {
          mem.vram[exram_addr] = val;
        } else {
          mem.vram[exram_addr] = 0;
        }
      }
      _ => {}
    }
  }

  fn ppu_special_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    // extended attributes only work on background tiles
    if self.exram_mode == 1 && self.ppu_in_frame && (self.nametbl_fetch_count < 32 || self.nametbl_fetch_count >= 48) {
      if addr < 0x2000 {
        // chr fetch
        // In other words, this works as if the nametable was extended from 8-bit to 14-bit tile offsets, 
        // with the ExRAM holding the upper 6-bits and the 2-bit palette value, while the nametable selected through $5105 contains the lower 8 bits.

        // IMPORTANT to extend the type size here. notice the shift left by 12?? 
        let chr_bank = ((self.chr_hi as usize) << 6) | (self.exattr_curr_val as usize & 0x3f);
        let chr_addr = (chr_bank << 12) + (addr as usize & 0xfff);
        return mem.chr[chr_addr]
      } else if addr & 0x3ff < 0x3c0 {
        // nametabl fetch
        // The extended attributes are 1-screen mirrored; in other words, they apply the same for all nametables.
        let exram_offset = addr as usize & 0x3ff;
        self.exattr_curr_val = mem.vram[0x800 + exram_offset];
        return mem.vram[mem.banks.vram.translate(addr)];
      } else {
        // attribute fetch
        let palette = (self.exattr_curr_val >> 6) as u8;
        return (palette << 6) | (palette << 4) | (palette << 2) | palette
      }
    }

    if addr < 0x2000 {
      // normal chr read
      mem.chr[mem.banks.chr.translate(addr)]
    } else {
      // nametables
      let vram_addr = mem.banks.vram.translate(addr);
      if matches!(vram_addr, 0x800..0xc00) && self.exram_mode > 1 {
        // exram reads as 0
        0
      } else {
        mem.vram[vram_addr]
      }
    }
  }

  // https://www.nesdev.org/wiki/MMC5#Scanline_Detection_and_Scanline_IRQ
  fn notify_ppu_addr(&mut self, mem: &mut Bus, _cycles: usize) {
    // nametable tile fetches, we also count attribute fetches
    if !matches!(mem.ppu_addr_bus, 0x2000..0x3000) {
      self.ppu_last_addr = Some(mem.ppu_addr_bus);
      self.ppu_idle_countdown = 3;
      return;
    }

    if mem.ppu_addr_bus & 0x3ff < 0x3c0 {
      self.nametbl_fetch_count += 1;
      
      // there are 16 dummy nametables fetches during sprites rendering
      if self.ppu_in_frame {
        self.update_chr_banks(mem);
      }
    }

    // The MMC5 detects scanlines by first looking for three consecutive PPU reads from the same nametable address in the range $2xxx. 
    // the scanline gets detected when the PPU does the attribute table byte read, which is at PPU cycle 4.
    if self.ppu_last_addr.is_some_and(|x| x == mem.ppu_addr_bus) {
      self.ppu_same_addr_count += 1;

      if self.ppu_same_addr_count >= 2 {
        // scanline just started
        self.nametbl_fetch_count = 0;

        if !self.ppu_in_frame {
          self.ppu_in_frame = true;
          self.irq_count = 0;
        } else {
          // currently, this happens at ppu dot 1 (for some reason)
          // we need the irq to be set at ppu dot 4, we put a little delay here.
          // seems to be working well enough for all games
          self.irq_delay = 4;
        }
      }
    } else {
      self.ppu_same_addr_count = 0;
    }

    self.ppu_last_addr = Some(mem.ppu_addr_bus);
    self.ppu_idle_countdown = 3;
  }

  fn step(&mut self, mem: &mut Bus, cycles: usize) {
    if self.ppu_idle_countdown > 0 {
      self.ppu_idle_countdown -= 1;
      if self.ppu_idle_countdown == 0 {
        self.ppu_in_frame = false;
        self.ppu_last_addr = None;
        self.update_chr_banks(mem);
      }
    }

    // delay solution which works perfectly for now
    if self.irq_delay > 0 {
      self.irq_delay -= 1; 
      if self.irq_delay == 0 {
        self.irq_count += 1;
        // Value $00 is a special case that will not produce IRQ pending conditions
        if self.irq_count == self.irq_cmp {
          self.irq_pending = true;
          // The IRQ pending flag is raised when the desired scanline is reached regardless of whether or not the scanline IRQ is enabled, i.e. even after a 0 was written to the scanline IRQ enable flag. 
          // However, an actual IRQ is only sent to the CPU if both the scanline IRQ enable flag and IRQ pending flag are set. 
          // A $5203 value of $00 is a special case where the comparison is never true.
          if self.irq_enabled {
            mem.irq.insert(IrqFlags::MAPPER);
          }
        }
      }
    }

    if cycles % 2 == 1 {
      self.p0.step_divider();
      self.p1.step_divider();
    }
    
    // envelope and length counter are fixed to a 240hz update rate.
    // 240hz is aproximately 14914 cpu cycles
    if cycles % 14914 == 0 {
      self.p0.len.step();
      self.p1.len.step();
      self.p0.env.step();
      self.p1.env.step();
    }
  }

  fn notify_cpu_addr(&mut self, mem: &mut Bus, addr: u16, val: Option<u8>) {
    match (addr, val) {
      (0xfffa | 0xfffb, None) => {
        self.reset_irq(mem);
        self.update_chr_banks(mem);
      }

      (0x2000, Some(val)) => {
        self.ppu_big_sprites = val & 0x20 > 0;
        self.update_chr_banks(mem);
      }

      (0x2001, Some(val)) => {
        let ppu_sub = val & 0x18 > 0;
        // When the MMC5 sees $00 written to $2001, and then the PPU’s rendering gets enabled via a mirror of $2001, the MMC5 still counts scanlines and can generate scanline interrupts even though it thinks $2001 is still disabled.
        // The transition from disabled to enabled resets the scanline counter.
        if !self.ppu_substituion && ppu_sub {
          self.reset_irq(mem);
        } else if !ppu_sub {
          self.ppu_in_frame = false;
          self.ppu_last_addr = None;
        }
        
        self.ppu_substituion = ppu_sub;

        // When it sees that both E bits are cleared, it disables its ability to make substitutions on the PPU data bus.
        if !self.ppu_substituion && self.exram_mode == 1 {
          self.exram_mode = 0;
        }
        self.update_chr_banks(mem);
      }

      _ => {}
    }
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}

  // The sound output of the square channels are equivalent in volume to the corresponding APU channels, but the polarity of all MMC5 channels is reversed compared to the APU. 
  fn sample(&self) -> f64 {
    let res = ((self.p0.output + self.p1.output) as f64).neg();
    res * apu::EXT_MIX
  }
}