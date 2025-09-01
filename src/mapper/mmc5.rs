use crate::{apu, bus::{Bus, Banking, CpuHandler, IrqFlags, PpuHandler}, mapper::Mapper};

#[derive(Default)]
pub struct MMC5 {
  ppu_substituion: bool,
  ppu_big_sprites: bool,

  prg_mode: u8,
  prg_regs: [u16; 5],

  chr_mode: u8,
  chr_regs: [u16; 12],
  chr_hi: u8,
  last_chr_wrote: u16,

  fill_tile: u8,
  fill_color: u8,

  nametbl_fetch_count: usize,
  last_nametbl_addr: u16,

  wram_protect: u8,
  exram_mode: u8,

  irq_enabled: bool,
  irq_pending: bool,
  irq_cmp: u16,
  irq_count: u16,
  ppu_in_frame: bool,

  ppu_same_addr_count: usize,
  last_ppu_addr: Option<u16>,
  ppu_idle_countdown: usize,

  multiplicand: u8,
  multiplier: u8,
  product: u16,

  p0: apu::Pulse,
  p1: apu::Pulse,
  audio_cycles: usize,
}
impl MMC5 {
  fn update_prg_banks(&mut self, mem: &mut Bus) {
    let wram = &mut mem.banks.wram;
    let prg = &mut mem.banks.prg;
  
    // always on wram page 0
    wram.set_page(0, self.prg_regs[0] & 0x7f);
    // always on rom, so forcefully set high bit to 1
    self.prg_regs[4] |= 0x80;

    let mut set_bank = |page, bank| {
      if bank & 0x80 > 0 {
        // rom
        prg.set_page(page - 1, bank & 0x7f);
        mem.cpu_handlers_8kb[3 + page as usize] = CpuHandler::PrgMMC5;
      } else {
        // ram
        wram.set_page(page, bank & 0x7f);

        let handler = if mem.wram.is_empty() {
          CpuHandler::Mapper
        } else if self.wram_protect == 0x6 {
          CpuHandler::Wram
        } else {
          CpuHandler::WramReadOnly
        };

        mem.cpu_handlers_8kb[3 + page as usize] = handler;
      }
    };

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
    let chr = &mut mem.banks.chr;

    match self.chr_mode {
      // 8kb
      0 => chr.set_pages_unaligned(0, self.chr_regs[7], 8),
      // 4kb
      1 => {
        chr.set_pages_unaligned(0, self.chr_regs[3], 4);
        chr.set_pages_unaligned(4, self.chr_regs[7], 4);
      }
      // 2kb
      2 =>  for i in 0..4 {
        // only odds chr_regs
        chr.set_pages_unaligned(i, self.chr_regs[i as usize * 2 + 1], 2);
      }
      // 1kb
      _ => for i in 0..8 {
        chr.set_page(i, self.chr_regs[i as usize]);
      }
    }
  }

  fn update_chr_high_regs(&mut self, mem: &mut Bus) {
    // Caution: Unlike the MMC1 and unlike PRG banking on the MMC5, the banks are always indexed by the currently selected size.
    // When using 2kb, 4kb or 8kb bank sizes, the registers hold bank index of that larger size, and lower bits are *not* ignored. 
    // shifting is needed
    let chr = &mut mem.banks.chr;

    match self.chr_mode {
      // 8kb
      0 => chr.set_pages_unaligned(0, self.chr_regs[11], 8),
      // 4kb
      1 => {
        chr.set_pages_unaligned(0, self.chr_regs[11], 4);
        chr.set_pages_unaligned(4, self.chr_regs[11], 4);
      }
      // 2kb
      2 =>  {
        chr.set_pages_unaligned(0, self.chr_regs[9], 2);
        chr.set_pages_unaligned(2, self.chr_regs[11], 2);
        chr.set_pages_unaligned(4, self.chr_regs[9], 2);
        chr.set_pages_unaligned(6, self.chr_regs[11], 2);
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
    self.last_ppu_addr = None;
    self.irq_count = 0;
    mem.irq.remove(IrqFlags::MAPPER);
  }
}
impl Mapper for MMC5 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    // wram can be mapped in range 0x6000..=0xdfff (32kb)
    mem.banks.wram = Banking::new(mem.header.wram_size, 32 * 1024, 4);
    mem.set_prg_handlers(CpuHandler::PrgMMC5);
    mem.cpu_handlers_8kb[1] = CpuHandler::PpuMMC5;

    // we simulate exram by extending vram to 4 screens
    // exram is mapped to third, fill screen is mapped to fourth
    mem.set_4screen_mirroring();
    // needed to substitute attribute tables reads
    mem.set_vram_handlers(PpuHandler::VramMMC5);
    mem.set_chr_handlers(PpuHandler::ChrMMC5);

    let mut res = Self::default();
    // The Koei games never write to this register, apparently relying on the MMC5 defaulting to mode 3 at power on. 
    res.prg_mode = 3;
    // All known games have their reset vector in the last bank of PRG ROM, and the vector points to an address greater than or equal to $E000.
    // This tells us that $5117 must have a reliable power-on value of $FF. 
    res.prg_regs[4] = 0xff;

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

  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    let val = val as u8;
    match addr {
      0x5000 => self.p0.write_ctrl(val as u8),
      0x5002 => self.p0.write_timer_lo(val as u8),
      0x5003 => self.p0.write_timer_hi(val as u8),

      0x5004 => self.p1.write_ctrl(val as u8),
      0x5006 => self.p1.write_timer_lo(val as u8),
      0x5007 => self.p1.write_timer_hi(val as u8),

      0x5015 => {
        self.p0.len.enable(val & 0x1 > 0);
        self.p1.len.enable(val & 0x2 > 0);
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
        // self.wram_protect == 0x6
        // TODO: wram rw
      }
      0x5103 => {
        self.wram_protect = (self.wram_protect & 0x3) | ((val & 0x3) << 2);
        // self.wram_protect == 0x6
        // TODO: wram rw
      }

      // TODO: only update when necessary
      0x5104 => self.exram_mode = val & 0x3,

      0x5105 => {for i in 0..4 {
        let nametbl = (val >> (i * 2)) & 0x3;
        // exram is mapped to the third nametable, fill mode to fourth
        mem.banks.vram.set_page(i, nametbl as u16);
      }
    }

      0x5106 => self.fill_tile = val,
      0x5107 => self.fill_color = val & 0x3,

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
        let exram_addr = 0x800 + (addr as usize - 0x5c00);

        // in mode 0 and 1, can only write during rendering
        // in mode 2, can always write
        if matches!((self.exram_mode, self.ppu_in_frame), (0 | 1, true) | (2, _)) {
          mem.vram[exram_addr] = val;
        }
      }
      _ => {}
    }
  }

  fn special_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    // extended attributes only work on background tiles
    if self.exram_mode == 1 && self.ppu_in_frame && (self.nametbl_fetch_count < 32 || self.nametbl_fetch_count >= 48) {
        //  The extended attributes are 1-screen mirrored; in other words, they apply the same for all nametables. 
      let exram_offset = self.last_nametbl_addr as usize & 0x3ff;
      let exattr = mem.vram[0x800 + exram_offset];

      if addr < 0x2000 {
        // pattern fetch

        // In other words, this works as if the nametable was extended from 8-bit to 14-bit tile offsets, 
        // with the ExRAM holding the upper 6-bits and the 2-bit palette value, while the nametable selected through $5105 contains the lower 8 bits. 
        let chr_bank = ((self.chr_hi as u16) << 6) | (exattr as u16 & 0x3f);
        let chr_addr = (chr_bank << 12) + (addr & 0xfff);
        mem.chr[chr_addr as usize]
      } else if addr & 0x3ff < 0x3c0 {
        // normal nametbl fetch
        self.last_nametbl_addr = addr;

        let vram_addr = addr - 0x2000;
        let table = (vram_addr) / 0x400;
  
        if mem.banks.vram.bankings[table as usize] == 0xc00 {
          self.fill_tile
        } else {
          mem.vram[mem.banks.vram.translate(vram_addr)]
        }
      } else {
        // attribute fetch
        let palette = (exattr >> 6) as u8;
        (palette << 6) | (palette << 4) | (palette << 2) | palette
      }
    } else {
      // not in extended attribute mode

      if addr < 0x2000 {
        // normal chr read
        return mem.chr[mem.banks.chr.translate(addr)]
      }

      let vram_addr = addr - 0x2000;
      let table = (vram_addr) / 0x400;

      if vram_addr & 0x3ff < 0x3c0 {
        // nametables, normal fetch
        if mem.banks.vram.bankings[table as usize] == 0xc00 {
          self.fill_tile
        } else {
          match self.exram_mode {
            2 | 3 => 0,
            _ => mem.vram[mem.banks.vram.translate(vram_addr)]
          }
        }
      } else {
        // attributes, special fetch
        if mem.banks.vram.bankings[table as usize] == 0xc00 {
          // if table is mapped to fill mode (fourth table)
          // Each byte of the attribute table normally contains four 2-bit palette indexes. The two bits in this register are copied for all four indexes. 
          (self.fill_color << 6) | (self.fill_color << 4) | (self.fill_color << 2) | self.fill_color
        } else {
          // table is mapped to normal vram, normal attribute fetch
          // if exram mode is 2 or 3, any table mapped to exram should read 0
          match self.exram_mode {
            2 | 3 => 0,
            _ => mem.vram[mem.banks.vram.translate(vram_addr)]
          }
        }
      }
    }
  } 

  // https://www.nesdev.org/wiki/MMC5#Scanline_Detection_and_Scanline_IRQ
  fn notify_ppu_addr(&mut self, mem: &mut Bus, _cycles: usize) {
    // nametable tile fetches, we also count attribute fetches
    if mem.ppu_addr_bus & 0x2000 > 0 && mem.ppu_addr_bus & 0x3ff < 0x3c0 {
      self.nametbl_fetch_count += 1;
      
      // there are 16 dummy nametables fetches during sprites rendering
      if self.ppu_in_frame {
        self.update_chr_banks(mem);
      }
    }

    // The MMC5 detects scanlines by first looking for three consecutive PPU reads from the same nametable address in the range $2xxx. 
    // the scanline gets detected when the PPU does the attribute table byte read, which is at PPU cycle 4.
    if mem.ppu_addr_bus & 0x2000 > 0 && self.last_ppu_addr.is_some_and(|x| x == mem.ppu_addr_bus) {
      self.ppu_same_addr_count += 1;

      if self.ppu_same_addr_count >= 2 {
        // scanline just started
        self.nametbl_fetch_count = 0;

        if !self.ppu_in_frame {
          self.ppu_in_frame = true;
          self.irq_count = 0;
          self.update_chr_banks(mem);
        } else {
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
    } else {
      self.ppu_same_addr_count = 0;
    }

    self.last_ppu_addr = Some(mem.ppu_addr_bus);
    self.ppu_idle_countdown = 3;
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.ppu_idle_countdown > 0 {
      self.ppu_idle_countdown -= 1;
      if self.ppu_idle_countdown == 0 {
        self.ppu_in_frame = false;
        self.last_ppu_addr = None;
        self.update_chr_banks(mem);
      }
    }

    if self.audio_cycles % 2 == 1 {
      self.p0.step_divider();
      self.p1.step_divider();
    }
    
    // envelope and length counter are fixed to a 240hz update rate.
    if self.audio_cycles > (1789773 / 240) {
      self.audio_cycles -= 1789773 / 240;
      self.p0.len.step();
      self.p0.env.step();
      self.p1.len.step();
      self.p1.env.step();
    }
    self.audio_cycles += 1;
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
          self.last_ppu_addr = None;
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

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u16) {}

  // The sound output of the square channels are equivalent in volume to the corresponding APU channels, but the polarity of all MMC5 channels is reversed compared to the APU. 
  fn sample(&self) -> f32 {
    // ((self.p0.sample() + self.p1.sample()) as f32).neg()
    0.0
  }
}