use crate::cart::{CartHeader, Mirroring, VRamTarget};

use super::{konami_irq::{IrqMode, KonamiIrq}, mirror_nametbl, Banking, ChrBanking, Mapper, PrgBanking, VRamBanking};

#[derive(serde::Serialize, serde::Deserialize)]
enum ChrMode { Bank1kb, Bank2kb, BankMixed }
#[derive(serde::Serialize, serde::Deserialize)]
enum NametblSrc { CiRam, ChrRom }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct VRC6 {
  mapper: u16,
  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,

  vram_chr_rom_banks: Banking<VRamBanking>,
  vram_ciram_banks: Banking<VRamBanking>,

  chr_selects: [usize; 8],

  irq: KonamiIrq,

  chr_mode: ChrMode,
  mirroring: Mirroring,
  nametbl_src: NametblSrc,
  nametbl_mode: u8,
  chr_latch: bool,
  sram_enabled: bool,
}

impl VRC6 {
  fn update_chr_banks(&mut self) {
    let bank_half = self.chr_latch as usize;

    match &self.chr_mode {
      ChrMode::Bank1kb => {
        for (reg, &bank) in self.chr_selects.iter().enumerate() {
          self.chr_banks.set(reg, bank as usize);
        }
      }
      ChrMode::Bank2kb => 
        for reg in (0..self.chr_selects.len()).step_by(2) {
          self.chr_banks.set(reg, self.chr_selects[reg/2] as usize);
          self.chr_banks.set(reg, self.chr_selects[reg/2] as usize+bank_half);
        }
      ChrMode::BankMixed => {
        for reg in 0..self.chr_selects.len()/2 {
          self.chr_banks.set(reg, self.chr_selects[reg] as usize);
        }
        self.chr_banks.set(4, self.chr_selects[4] as usize);
        self.chr_banks.set(5, self.chr_selects[4] as usize+bank_half);
        self.chr_banks.set(6, self.chr_selects[5] as usize);
        self.chr_banks.set(7, self.chr_selects[5] as usize+bank_half);
      }
    }
  }

  fn update_mirroring(&mut self) {
    // https://github.com/SourMesen/Mesen2/blob/master/Core/NES/Mappers/Konami/VRC6.h
    // no clue how this shit works

    match self.nametbl_src {
      NametblSrc::ChrRom => match self.nametbl_mode {
        0x20 | 0x27 => {
          self.vram_chr_rom_banks.set(0, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(1, self.chr_selects[6] | 1);
          self.vram_chr_rom_banks.set(2, self.chr_selects[7]);
          self.vram_chr_rom_banks.set(3, self.chr_selects[7] | 1);
        }
        0x23 | 0x24 => {
          self.vram_chr_rom_banks.set(0, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(1, self.chr_selects[7]);
          self.vram_chr_rom_banks.set(2, self.chr_selects[6] | 1);
          self.vram_chr_rom_banks.set(3, self.chr_selects[7] | 1);
        }
        0x28 | 0x2F => {
          self.vram_chr_rom_banks.set(0, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(1, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(2, self.chr_selects[7]);
          self.vram_chr_rom_banks.set(3, self.chr_selects[7]);
        }
        0x2B | 0x2C => {
          self.vram_chr_rom_banks.set(0, self.chr_selects[6] | 1);
          self.vram_chr_rom_banks.set(1, self.chr_selects[7] | 1);
          self.vram_chr_rom_banks.set(2, self.chr_selects[6] | 1);
          self.vram_chr_rom_banks.set(3, self.chr_selects[7] | 1);
        }

        0 | 6 | 7 => {
          self.vram_chr_rom_banks.set(0, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(1, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(2, self.chr_selects[7]);
          self.vram_chr_rom_banks.set(3, self.chr_selects[7]);
        }
        1 | 5 => {
          self.vram_chr_rom_banks.set(0, self.chr_selects[4]);
          self.vram_chr_rom_banks.set(1, self.chr_selects[5]);
          self.vram_chr_rom_banks.set(2, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(3, self.chr_selects[7]);
        }
        2 | 3 | 4 => {
          self.vram_chr_rom_banks.set(0, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(1, self.chr_selects[7]);
          self.vram_chr_rom_banks.set(2, self.chr_selects[6]);
          self.vram_chr_rom_banks.set(3, self.chr_selects[7]);
        }
        _ => {}
      }
      NametblSrc::CiRam => match self.nametbl_mode {
        0x20 | 0x27 => self.mirroring = Mirroring::Vertical,
        0x23 | 0x24 => self.mirroring = Mirroring::Horizontal,
        0x28 | 0x2F => self.mirroring = Mirroring::SingleScreenA,
        0x2B | 0x2C => self.mirroring = Mirroring::SingleScreenB,

        0 | 6 | 7 => {
          self.mirroring = Mirroring::FourScreen;
          self.vram_ciram_banks.set(0, self.chr_selects[6]);
          self.vram_ciram_banks.set(1, self.chr_selects[6]);
          self.vram_ciram_banks.set(2, self.chr_selects[7]);
          self.vram_ciram_banks.set(3, self.chr_selects[7]);
        }
        1 | 5 => {
          self.mirroring = Mirroring::FourScreen;
          self.vram_ciram_banks.set(0, self.chr_selects[4]);
          self.vram_ciram_banks.set(1, self.chr_selects[5]);
          self.vram_ciram_banks.set(2, self.chr_selects[6]);
          self.vram_ciram_banks.set(3, self.chr_selects[7]);
        }
        2 | 3 | 4 => {
          self.mirroring = Mirroring::FourScreen;
          self.vram_ciram_banks.set(0, self.chr_selects[6]);
          self.vram_ciram_banks.set(1, self.chr_selects[7]);
          self.vram_ciram_banks.set(2, self.chr_selects[6]);
          self.vram_ciram_banks.set(3, self.chr_selects[7]);
        }
        _ => {}
      }
    }
  }
}

#[typetag::serde]
impl Mapper for VRC6 {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 4);
    let chr_banks = Banking::new_chr(header, 8);
    let vram_chr_rom_banks = Banking::new_vram(header.chr_real_size());
    let vram_ciram_banks = Banking::new_vram(4 * 1024);
    
    // prg_banks.set(1, 1);
    prg_banks.set_page_to_last_bank(3);
    
    let mut mapper = Self {
      mapper: header.mapper,
      prg_banks,
      chr_banks,
      vram_chr_rom_banks,
      vram_ciram_banks,
      chr_selects: [0; 8],
      irq: Default::default(),
      chr_latch: false,
      chr_mode: ChrMode::Bank1kb,
      mirroring: Mirroring::Vertical,
      nametbl_src: NametblSrc::CiRam,
      nametbl_mode: 0,
      sram_enabled: false,
    };
    mapper.update_chr_banks();
    mapper.update_mirroring();

    Box::new(mapper)
  }

  fn write(&mut self, mut addr: usize, val: u8) {
		if self.mapper == 26 {
			addr = (addr & 0xFFFC) | ((addr & 0x01) << 1) | ((addr & 0x02) >> 1);
		}

    match addr & 0xF003 {
      0x8000..=0x8003 => {
        let bank = (val as usize & 0b1111) << 1;
        self.prg_banks.set(0, bank);
        self.prg_banks.set(1, bank+1);
      }
      0xC000..=0xC003 => {
        self.prg_banks.set(2, val as usize & 0b1_1111);
      }
      
      0xB003 => {
        self.chr_mode = match val & 0b11 {
          0 => ChrMode::Bank1kb,
          1 => ChrMode::Bank2kb,
          _ => ChrMode::BankMixed,
        };
        self.update_chr_banks();

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
        self.update_chr_banks();
      }
      0xE000..=0xE003 => {
        let reg = addr - 0xE000;
        self.chr_selects[reg + 4] = val as usize;
        self.update_chr_banks();
      }

      0xF000 => self.irq.latch = val as u16,
      0xF001 => self.irq.write_ctrl(val),
      0xF002 => self.irq.write_ack(),
      _ => {}
    }
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn vram_addr(&mut self, addr: usize) -> (VRamTarget, usize) {
    match self.nametbl_src {
      NametblSrc::CiRam => {
        let ciram_addr = if self.mirroring != Mirroring::FourScreen {
          mirror_nametbl(self.mirroring, addr)
        } else {
          self.vram_ciram_banks.addr(addr)
        };

        (VRamTarget::CiRam, ciram_addr)
      }
      NametblSrc::ChrRom => {
        let chr_rom_addr = self.vram_chr_rom_banks.addr(addr);
        (VRamTarget::Chr, chr_rom_addr)
      }
    }
  }

  fn notify_cpu_cycle(&mut self) {
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

  fn poll_irq(&mut self) -> bool {
    self.irq.requested.is_some()
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}