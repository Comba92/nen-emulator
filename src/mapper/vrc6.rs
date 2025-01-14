use crate::{apu::{ApuDivider, Channel}, cart::{CartBanking, CartHeader, Mirroring, PpuTarget}};
use super::{konami_irq::{IrqMode, KonamiIrq}, Banking, Mapper, CiramBanking};

#[derive(serde::Serialize, serde::Deserialize)]
enum ChrMode { Bank1kb, Bank2kb, BankMixed }
#[derive(serde::Serialize, serde::Deserialize)]
enum NametblSrc { CiRam, ChrRom }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct VRC6 {
  mapper: u16,

  vram_chrrom_banks: Banking<CiramBanking>,
  vram_ciram_banks: Banking<CiramBanking>,
  chr_selects: [usize; 8],

  irq: KonamiIrq,

  chr_mode: ChrMode,
  nametbl_src: NametblSrc,
  nametbl_mode: u8,
  chr_latch: bool,
  sram_enabled: bool,

  apu_halted: bool,
  apu_freq16: bool,
  apu_freq256: bool,

  pulse1: PulseVRC6,
  pulse2: PulseVRC6,
  sawtooth: SawtoothVRC6,
}

impl VRC6 {
  fn update_chr_banks(&self, banks: &mut CartBanking) {
    let bank_half = self.chr_latch as usize;

    match &self.chr_mode {
      ChrMode::Bank1kb => {
        for (reg, &bank) in self.chr_selects.iter().enumerate() {
          banks.chr.set_page(reg, bank as usize);
        }
      }
      ChrMode::Bank2kb => 
        for reg in (0..self.chr_selects.len()).step_by(2) {
          banks.chr.set_page(reg, self.chr_selects[reg/2] as usize);
          banks.chr.set_page(reg, self.chr_selects[reg/2] as usize | bank_half);
        }
      ChrMode::BankMixed => {
        for reg in 0..self.chr_selects.len()/2 {
          banks.chr.set_page(reg, self.chr_selects[reg] as usize);
        }
        banks.chr.set_page(4, self.chr_selects[4] as usize);
        banks.chr.set_page(5, self.chr_selects[4] as usize | bank_half);
        banks.chr.set_page(6, self.chr_selects[5] as usize);
        banks.chr.set_page(7, self.chr_selects[5] as usize | bank_half);
      }
    }
  }

  fn update_mirroring(&mut self) {
    // https://github.com/SourMesen/Mesen2/blob/master/Core/NES/Mappers/Konami/VRC6.h
    // no clue how this shit works

    match self.nametbl_src {
      NametblSrc::ChrRom => match self.nametbl_mode {
        0x20 | 0x27 => {
          self.vram_chrrom_banks.set_page(0, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(1, self.chr_selects[6] | 1);
          self.vram_chrrom_banks.set_page(2, self.chr_selects[7]);
          self.vram_chrrom_banks.set_page(3, self.chr_selects[7] | 1);
        }
        0x23 | 0x24 => {
          self.vram_chrrom_banks.set_page(0, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(1, self.chr_selects[7]);
          self.vram_chrrom_banks.set_page(2, self.chr_selects[6] | 1);
          self.vram_chrrom_banks.set_page(3, self.chr_selects[7] | 1);
        }
        0x28 | 0x2F => {
          self.vram_chrrom_banks.set_page(0, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(1, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(2, self.chr_selects[7]);
          self.vram_chrrom_banks.set_page(3, self.chr_selects[7]);
        }
        0x2B | 0x2C => {
          self.vram_chrrom_banks.set_page(0, self.chr_selects[6] | 1);
          self.vram_chrrom_banks.set_page(1, self.chr_selects[7] | 1);
          self.vram_chrrom_banks.set_page(2, self.chr_selects[6] | 1);
          self.vram_chrrom_banks.set_page(3, self.chr_selects[7] | 1);
        }

        0 | 6 | 7 => {
          self.vram_chrrom_banks.set_page(0, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(1, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(2, self.chr_selects[7]);
          self.vram_chrrom_banks.set_page(3, self.chr_selects[7]);
        }
        1 | 5 => {
          self.vram_chrrom_banks.set_page(0, self.chr_selects[4]);
          self.vram_chrrom_banks.set_page(1, self.chr_selects[5]);
          self.vram_chrrom_banks.set_page(2, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(3, self.chr_selects[7]);
        }
        2 | 3 | 4 => {
          self.vram_chrrom_banks.set_page(0, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(1, self.chr_selects[7]);
          self.vram_chrrom_banks.set_page(2, self.chr_selects[6]);
          self.vram_chrrom_banks.set_page(3, self.chr_selects[7]);
        }
        _ => {}
      }
      NametblSrc::CiRam => match self.nametbl_mode {
        0x20 | 0x27 => self.vram_ciram_banks.update(Mirroring::Vertical),
        0x23 | 0x24 => self.vram_ciram_banks.update(Mirroring::Horizontal),
        0x28 | 0x2F => self.vram_ciram_banks.update(Mirroring::SingleScreenA),
        0x2B | 0x2C => self.vram_ciram_banks.update(Mirroring::SingleScreenB),

        0 | 6 | 7 => {
          self.vram_ciram_banks.set_page(0, self.chr_selects[6]);
          self.vram_ciram_banks.set_page(1, self.chr_selects[6]);
          self.vram_ciram_banks.set_page(2, self.chr_selects[7]);
          self.vram_ciram_banks.set_page(3, self.chr_selects[7]);
        }
        1 | 5 => {
          self.vram_ciram_banks.set_page(0, self.chr_selects[4]);
          self.vram_ciram_banks.set_page(1, self.chr_selects[5]);
          self.vram_ciram_banks.set_page(2, self.chr_selects[6]);
          self.vram_ciram_banks.set_page(3, self.chr_selects[7]);
        }
        2 | 3 | 4 => {
          self.vram_ciram_banks.set_page(0, self.chr_selects[6]);
          self.vram_ciram_banks.set_page(1, self.chr_selects[7]);
          self.vram_ciram_banks.set_page(2, self.chr_selects[6]);
          self.vram_ciram_banks.set_page(3, self.chr_selects[7]);
        }
        _ => {}
      }
    }
  }

  fn handle_irq(&mut self) {
    if !self.irq.enabled { return; }

    match self.irq.mode {
      IrqMode::Mode1 => {
        self.irq.count += 1;
      }
      IrqMode::Mode0 => {
        self.irq.prescaler -= 3;
        if self.irq.prescaler <= 0 {
          self.irq.prescaler += 341;
          self.irq.count += 1;
        }
      }
    }

    if self.irq.count > 0xFF {
      self.irq.requested = Some(());
      self.irq.count = self.irq.latch;
    }
  }

  fn handle_apu(&mut self) {
    if self.apu_halted { return; }

    self.pulse1.step_timer();
    self.pulse2.step_timer();
    self.sawtooth.step_timer();
  }
}

#[typetag::serde]
impl Mapper for VRC6 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    banks.chr = Banking::new_chr(header, 8);
    let vram_chrrom_banks = Banking::new(header.chr_real_size(), 0x2000, 1024, 4);
    let vram_ciram_banks =  Banking::new_ciram(header);
    
    banks.prg.set_page_to_last_bank(3);
    
    let mut mapper = Self {
      mapper: header.mapper,
      vram_chrrom_banks,
      vram_ciram_banks,
      chr_selects: [0; 8],
      irq: Default::default(),
      chr_latch: false,
      chr_mode: ChrMode::Bank1kb,
      nametbl_src: NametblSrc::CiRam,
      nametbl_mode: 0,
      sram_enabled: false,
      apu_halted: true,
      apu_freq16: false,
      apu_freq256: false,
      pulse1: Default::default(),
      pulse2: Default::default(),
      sawtooth: Default::default(),
    };

    mapper.update_chr_banks(banks);
    mapper.update_mirroring();

    Box::new(mapper)
  }

  fn prg_write(&mut self, banks: &mut CartBanking, mut addr: usize, val: u8) {
		if self.mapper == 26 {
			addr = (addr & 0xFFFC) | ((addr & 0x01) << 1) | ((addr & 0x02) >> 1);
		}

    match addr & 0xF003 {
      0x8000..=0x8003 => {
        let bank = (val as usize & 0b1111) << 1;
        banks.prg.set_page(0, bank);
        banks.prg.set_page(1, bank+1);
      }
      0xC000..=0xC003 => {
        banks.prg.set_page(2, val as usize & 0b1_1111);
      }
      
      0xB003 => {
        self.chr_mode = match val & 0b11 {
          0 => ChrMode::Bank1kb,
          1 => ChrMode::Bank2kb,
          _ => ChrMode::BankMixed,
        };
        self.update_chr_banks(banks);

        self.nametbl_mode = val & 0b10_1111;

        self.nametbl_src = match (val >> 4) & 1 != 0 {
          false => NametblSrc::CiRam,
          true  => NametblSrc::ChrRom,
        };

        self.chr_latch = (val >> 5) & 1 != 0;
        self.sram_enabled = (val >> 7) & 1 != 0;
        self.update_mirroring();
      },

      0xD000..=0xD003 => {
        let reg = addr - 0xD000;
        self.chr_selects[reg] = val as usize;
        self.update_chr_banks(banks);
      }
      0xE000..=0xE003 => {
        let reg = addr - 0xE000;
        self.chr_selects[reg + 4] = val as usize;
        self.update_chr_banks(banks);
      }

      0xF000 => self.irq.latch = val as u16,
      0xF001 => self.irq.write_ctrl(val),
      0xF002 => self.irq.write_ack(),
      
      0x9003 => {
        self.apu_halted = val & 1 != 0;
        self.apu_freq16 = (val >> 1) & 1 != 0;
        self.apu_freq256 = (val >> 2) & 1 != 0;

        if self.apu_freq256 {
          self.pulse1.freq_shift = 8;
          self.pulse2.freq_shift = 8;
          self.sawtooth.freq_shift = 8;
        } else if self.apu_freq16 {
          self.pulse1.freq_shift = 4;
          self.pulse2.freq_shift = 4;
          self.sawtooth.freq_shift = 4;
        }
      }

      0x9000 => self.pulse1.set_ctrl(val),
      0x9001 => self.pulse1.set_period_low(val),
      0x9002 => self.pulse1.set_period_high(val),

      0xA000 => self.pulse2.set_ctrl(val),
      0xA001 => self.pulse2.set_period_low(val),
      0xA002 => self.pulse2.set_period_high(val),

      0xB000 => self.sawtooth.set_acc(val),
      0xB001 => self.sawtooth.set_period_low(val),
      0xB002 => self.sawtooth.set_period_high(val),
      _ => {}
    }
  }

  fn map_ppu_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PpuTarget {
    match addr {
      0x0000..=0x1FFF => PpuTarget::Chr(banks.chr.translate(addr)),
      0x2000..=0x2FFF => match self.nametbl_src {
        NametblSrc::CiRam => {
          let ciram_addr = self.vram_ciram_banks.translate(addr);
          PpuTarget::CiRam(ciram_addr)
        }
        NametblSrc::ChrRom => {
          let chrrom_addr = self.vram_chrrom_banks.translate(addr);
          PpuTarget::Chr(chrrom_addr)
        }
      }
      _ => unreachable!()
    }
  }

  fn notify_cpu_cycle(&mut self) {
    self.handle_irq();
    self.handle_apu();
  }

  fn get_sample(&self) -> f32 {
    let sum = self.pulse1.get_sample() 
      + self.pulse2.get_sample()
      + self.sawtooth.get_sample();

    // magic value taken from here
    // https://github.com/zeta0134/rustico/blob/e1ee2211cc6173fe2df0df036c9c2a30e9966136/core/src/mmc/vrc6.rs
    0.00845 * sum as f32
  }

  fn poll_irq(&mut self) -> bool {
    self.irq.requested.is_some()
  }
}


#[derive(Default, serde::Serialize, serde::Deserialize)]
struct PulseVRC6 {
  timer: ApuDivider,
  pub freq_shift: u8,
  volume: u8,
  duty_idx: u8,
  duty_cycle: u8,
  ignore_duty: bool,
  enabled: bool,
}
impl PulseVRC6 {
  pub fn set_ctrl(&mut self, val: u8) {
    self.volume = val & 0b1111;
    self.duty_cycle = (val >> 4) & 1;
    self.ignore_duty = (val >> 7) == 1;
  }

  pub fn set_period_low(&mut self, val: u8) {
    self.timer.set_period_low(val);
  }

  pub fn set_period_high(&mut self, val: u8) {
    self.timer.period = self.timer.period & 0x00FF
    | ((val as u16 & 0b1111) << 8);
    self.enabled = (val >> 7) != 0;
    if !self.enabled {
      self.duty_idx = 0;
    }
  }
}
impl Channel for PulseVRC6 {
  fn step_timer(&mut self) {
    self.timer.step(|timer| {
      self.duty_idx = (self.duty_idx + 1) % 16;
      timer.count = (timer.period >> self.freq_shift) + 1
    });
  }

  fn step_quarter(&mut self) {}
  fn step_half(&mut self) {}

  fn is_enabled(&self) -> bool { self.enabled }

  fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
  }

  fn get_sample(&self) -> u8 {
    if (self.ignore_duty || self.duty_idx <= self.duty_cycle)  
      && self.enabled
    { 
      self.volume
    } else { 0 }
  }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct SawtoothVRC6 {
  timer: ApuDivider,
  freq_shift: u8,
  acc_rate: u8,
  acc: u8,
  duty: u8,
  enabled: bool,
}
impl SawtoothVRC6 {
  pub fn set_acc(&mut self, val: u8) {
    self.acc_rate = val & 0b11_1111;
  }

  pub fn set_period_low(&mut self, val: u8) {
    self.timer.set_period_low(val);
  }

  pub fn set_period_high(&mut self, val: u8) {
    self.timer.period = self.timer.period & 0x00FF
    | ((val as u16 & 0b1111) << 8);
    self.enabled = (val >> 7) != 0;
    if !self.enabled {
      self.acc = 0;
      self.duty = 0;
    }
  }
}
impl Channel for SawtoothVRC6 {
  fn step_timer(&mut self) {
    self.timer.step(|timer| {
      self.duty = (self.duty + 1) % 14;
      timer.count = (timer.period >> self.freq_shift) + 1;

      if self.duty == 0 {
        self.acc = 0;
      } else if self.duty % 2 == 0 {
        self.acc += self.acc_rate;
      }
    });
  }

  fn step_quarter(&mut self) {}
  fn step_half(&mut self) {}
  fn is_enabled(&self) -> bool {
    self.enabled
  }
  fn set_enabled(&mut self, enabled: bool) {
    self.enabled = enabled;
  }

  fn get_sample(&self) -> u8 {
    if self.enabled {
      self.acc >> 3
    } else { 0 }
  }
}