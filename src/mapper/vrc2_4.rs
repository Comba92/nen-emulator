use bitfield_struct::bitfield;

use crate::cart::Mirroring;

use super::{konami_irq::{IrqMode, KonamiIrq}, Banking, ChrBanking, Mapper, PrgBanking};

#[bitfield(u16, order = Lsb)]
struct ChrSelectByte {
  #[bits(4)]
  lo: u8,
  #[bits(5)]
  hi: u8,

  #[bits(7)]
  __: u8
}

impl serde::Serialize for ChrSelectByte {
	fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
	where
		S: serde::Serializer {
		serializer.serialize_u16(self.0)
	}
}
impl<'de> serde::Deserialize<'de> for ChrSelectByte {
	fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
	where
		D: serde::Deserializer<'de> {
		struct ByteVisitor;
		impl<'de> serde::de::Visitor<'de> for ByteVisitor {
			type Value = ChrSelectByte;

			fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
				formatter.write_str("u16")
			}

			fn visit_u16<E>(self, v: u16) -> Result<Self::Value, E>
				where
					E: serde::de::Error, {
				Ok(ChrSelectByte::from_bits(v))
			}
		}
		deserializer.deserialize_u16(ByteVisitor)
	}
}

// Mappers 21, 22, 23, 25
// https://www.nesdev.org/wiki/VRC2_and_VRC4
#[derive(serde::Serialize, serde::Deserialize)]
pub struct VRC2_4 {
  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,
  
  prg_select0: u8,
  prg_select1: u8,
  chr_selects: [ChrSelectByte; 8],

  mapper: u16,
  swap_mode: bool,
  sram_ctrl: bool,
  latch: bool,

  irq: KonamiIrq,

  mirroring: Mirroring,
}

impl VRC2_4 {
  // iNes compatibility
  fn translate_addr(&self, addr: usize) -> usize {
    // Taken from Mesen emulator source, this trick makes it work without discriminating submapper
    // https://github.com/SourMesen/Mesen2/blob/master/Core/NES/Mappers/Konami/VRC2_4.h
    
    let (a0, a1) = match self.mapper {
      21 => {
        // Vrc4 a/c
        let mut a0 = (addr >> 1) & 1;
        let mut a1 = (addr >> 2) & 1;

        a0 |= (addr >> 6) & 1;
        a1 |= (addr >> 7) & 1;
        (a0, a1)
      }
      22 => {
        // Vrc1a

        let a0 = (addr >> 1) & 1;
        let a1 = addr & 1;

        (a0, a1)
      }
      23 => {
        // Vrc2b, Vrc4 e/f

        let mut a0 = addr & 1;
        let mut a1 = (addr >> 1) & 1;

        a0 |= (addr >> 2) & 1;
        a1 |= (addr >> 3) & 1;
        (a0, a1)
      }
      25 => {
        // Vrc2c, Vrc4 b/d

        let mut a0 = (addr >> 1) & 1;
        let mut a1 = addr & 1;

        a0 |= (addr >> 3) & 1;
        a1 |= (addr >> 2) & 1;
        (a0, a1)
      }
      _ => unreachable!()
    };

    (addr & 0xFF00 | (a1 << 1) | a0) & 0xF00F
  }

  fn update_prg_banks(&mut self) {
    match self.swap_mode {
      false => {
        self.prg_banks.set(0, self.prg_select0 as usize);
        self.prg_banks.set(2, self.prg_banks.banks_count-2);
      }
      true  => {
        self.prg_banks.set(0, self.prg_banks.banks_count-2);
        self.prg_banks.set(2, self.prg_select0 as usize);
      }
    }
  }

  fn update_chr_banks(&mut self, addr: usize, val: u8) {
    let res = match addr {
      0xB000 => Some((0, false)),
      0xB001 => Some((0, true)),

      0xB002 => Some((1, false)),
      0xB003 => Some((1, true)),

      0xC000 => Some((2, false)),
      0xC001 => Some((2, true)),

      0xC002 => Some((3, false)),
      0xC003 => Some((3, true)),

      0xD000 => Some((4, false)),
      0xD001 => Some((4, true)),

      0xD002 => Some((5, false)),
      0xD003 => Some((5, true)),

      0xE000 => Some((6, false)),
      0xE001 => Some((6, true)),

      0xE002 => Some((7, false)),
      0xE003 => Some((7, true)),
      _ => None,
    };

    if let Some((reg, is_high)) = res {
      if is_high {
        self.chr_selects[reg].set_hi(val & 0b1_1111);
      } else {
        self.chr_selects[reg].set_lo(val & 0b1111);
      }

      self.chr_banks.set(reg, self.chr_selects[reg].0 as usize);
    }
  }
}

#[typetag::serde]
impl Mapper for VRC2_4 {
  fn new(header: &crate::cart::CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 4);
    let chr_banks = Banking::new_chr(header, 8);

    prg_banks.set(2, prg_banks.banks_count-2);
    prg_banks.set(3, prg_banks.banks_count-1);

    let mapper = Self {
      prg_banks,
      chr_banks,
      prg_select0: 0,
      prg_select1: 0,
      chr_selects: [Default::default(); 8],
      mapper: header.mapper,

      swap_mode: false,
      sram_ctrl: false,
      latch: false,
      irq: Default::default(),
      
      mirroring: Default::default(),
    };

    Box::new(mapper)
  }

  fn write(&mut self, addr: usize, val: u8) {
    let addr = self.translate_addr(addr);
    match addr {
      0x9002 => {
        self.sram_ctrl = val & 0b01 != 0;
        self.swap_mode = val & 0b10 != 0 && self.mapper != 22;
        self.update_prg_banks();
      }

      0x8000..=0x8006 => {
        self.prg_select0 = val & 0b1_1111;
        self.update_prg_banks();
      }
      0xA000..=0xA006 => {
        self.prg_select1 = val & 0b1_1111;
        self.prg_banks.set(1, self.prg_select1 as usize);
      }
      0x9000..=0x9003 => self.mirroring = match val & 0b11 {
        0 => Mirroring::Vertical,
        1 => Mirroring::Horizontal,
        2 => Mirroring::SingleScreenA,
        _ => Mirroring::SingleScreenB,
      },
      0xB000..=0xE003 => self.update_chr_banks(addr, val),

      0xF000 => self.irq.latch = (self.irq.latch & 0xF0) | (val as u16 & 0b1111),
      0xF001 => self.irq.latch = (self.irq.latch & 0x0F) | ((val as u16 & 0b1111) << 4),
      0xF002 => self.irq.write_ctrl(val),
      0xF003 => self.irq.write_ack(),
      _ => {}
    }
  }

  fn sram_read(&self, ram: &[u8], addr: usize) -> u8 {
    if self.mapper == 22 {
      self.latch as u8
    } else { ram[addr - 0x6000] }
  }

  fn sram_write(&mut self, ram: &mut [u8], addr: usize, val: u8) {
    if self.mapper == 22 {
      self.latch = val & 1 != 0;
    } else {
      ram[addr - 0x6000] = val;
    }
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn notify_cpu_cycle(&mut self) {
    if !self.irq.enabled || self.mapper == 22 { return; }

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