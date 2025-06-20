use crate::{
  banks::{Banking, MemConfig},
  cart::{CartHeader, Mirroring},
  mapper::{
    bandai_fcg::BandaiFCG, gtrom::GTROM, mmc1::MMC1, mmc2::MMC2, mmc3::MMC3, mmc5::MMC5,
    namco129_163::Namco129_163, sunsoft4::Sunsoft4, sunsoft_fme_7::SunsoftFME7,
    unrom512::UNROM512, vrc2_4::VRC2_4, vrc3::VRC3, vrc6::VRC6, vrc7::VRC7,
  },
  ppu::RenderingState,
};

mod bandai_fcg;
mod gtrom;
mod konami_irq;
mod mmc1;
mod mmc2;
mod mmc3;
mod mmc5;
mod namco129_163;
mod sunsoft4;
mod sunsoft_fme_7;
mod unrom512;
mod vrc2_4;
mod vrc3;
mod vrc6;
mod vrc7;

pub fn new_mapper(header: &CartHeader, cfg: &mut MemConfig) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header, cfg),
    1 => MMC1::new(header, cfg),
    2 | 180 => UxROM::new(header, cfg),
    3 => CNROM::new(header, cfg),
    4 => MMC3::new(header, cfg),
    5 => MMC5::new(header, cfg),
    7 => AxROM::new(header, cfg),
    9 | 10 => MMC2::new(header, cfg),
    11 => ColorDreams::new(header, cfg),
    13 => CPROM::new(header, cfg),
    16 => BandaiFCG::new(header, cfg),
    19 => Namco129_163::new(header, cfg),
    21 | 22 | 23 | 25 => VRC2_4::new(header, cfg),
    24 | 26 => VRC6::new(header, cfg),
    30 => UNROM512::new(header, cfg),
    31 => INesMapper031::new(header, cfg),
    34 => INesMapper034::new(header, cfg),
    66 => GxROM::new(header, cfg),
    68 => Sunsoft4::new(header, cfg),
    69 => SunsoftFME7::new(header, cfg),
    71 => Codemasters::new(header, cfg),
    73 => VRC3::new(header, cfg),
    75 => VRC1::new(header, cfg),
    78 => INesMapper078::new(header, cfg),
    85 => VRC7::new(header, cfg),
    87 => INesMapper087::new(header, cfg),
    111 => GTROM::new(header, cfg),
    206 => INesMapper206::new(header, cfg),
    _ => return Err(format!("Mapper {} not implemented", header.mapper)),
  };

  Ok(mapper)
}

// pub enum PpuTarget { Chr(usize), vram(usize), ExRam(usize), Value(u8) }
// pub enum PrgTarget { Prg(usize), SRam(bool, usize), Cart }

#[cfg_attr(feature = "serde", typetag::serde)]
pub trait Mapper {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self>
  where
    Self: Sized;
  fn prg_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8);

  fn prg_translate(&mut self, cfg: &mut MemConfig, addr: u16) -> usize {
    cfg.prg.translate(addr as usize)
  }
  fn chr_translate(&mut self, cfg: &mut MemConfig, addr: u16) -> usize {
    cfg.chr.translate(addr as usize)
  }
  fn vram_translate(&mut self, cfg: &mut MemConfig, addr: u16) -> usize {
    cfg.vram.translate(addr as usize)
  }

  fn cart_read(&mut self, _addr: usize) -> u8 {
    0xFF
  }
  fn cart_write(&mut self, _banks: &mut MemConfig, _addr: usize, _val: u8) {}

  fn exram_read(&mut self, _addr: usize) -> u8 {
    0xFF
  }
  fn exram_write(&mut self, _addr: usize, _val: u8) {}

  fn poll_irq(&mut self) -> bool {
    false
  }

  // Generic cpu cycle notify / apu extension clocking
  fn notify_cpu_cycle(&mut self) {}
  fn get_sample(&self) -> u8 {
    0
  }

  // Mmc3 scanline notify
  fn notify_mmc3_scanline(&mut self) {}

  // Mmc5 ppu notify
  fn notify_ppuctrl(&mut self, _val: u8) {}
  fn notify_ppumask(&mut self, _val: u8) {}
  fn notify_ppu_state(&mut self, _state: RenderingState) {}
  fn notify_mmc5_scanline(&mut self) {}
}

pub fn set_byte_hi(dst: u16, val: u8) -> u16 {
  (dst & 0x00FF) | ((val as u16) << 8)
}

pub fn set_byte_lo(dst: u16, val: u8) -> u16 {
  (dst & 0xFF00) | val as u16
}

pub fn mapper_name(id: u16) -> &'static str {
  MAPPERS_TABLE
    .iter()
    .find(|m| m.0 == id)
    .map(|m| m.1)
    .unwrap_or("Not implemented")
}
const MAPPERS_TABLE: [(u16, &'static str); 39] = [
  (0, "NROM"),
  (1, "MMC1"),
  (2, "UxROM"),
  (3, "CNROM"),
  (4, "MMC3"),
  (5, "MMC5"),
  (7, "AxROM"),
  (9, "MMC2 (Punch-Out!!)"),
  (10, "MMC4"),
  (11, "ColorDreams"),
  (13, "CPROM"),
  (16, "Bandai FCG"),
  (19, "Namco 129/163"),
  (21, "Konami VRC2/VRC4"),
  (22, "Konami VRC2/VRC4"),
  (23, "Konami VRC2/VRC4"),
  (24, "Konami VRC6a (Akumajou Densetsu)"),
  (25, "Konami VRC2/VRC4"),
  (26, "Konami VRC6b (Madara and Esper Dream 2)"),
  (30, "UNROM 512"),
  (31, "NSF"),
  (34, "BNROM/NINA-001"),
  (48, "Taito TC0690"),
  (66, "GxROM"),
  (68, "Sunsoft4"),
  (69, "Sunsoft5 FME-7"),
  (71, "Codemasters UNROM"),
  (73, "Konami VRC3 (Salamander)"),
  (75, "Konami VRC1"),
  (78, "Irem 74HC161 (Holy Diver and Cosmo Carrier)"),
  (85, "VRC7 (Lagrange Point and Tiny Toon Adventures 2)"),
  (87, "Jaleco87"),
  (91, "J.Y. Company"),
  (94, "UNROM (Senjou no Ookami)"),
  (111, "GTROM (Cheapocabra)"),
  (163, "FC-001"),
  (180, "UNROM (Crazy Climber)"),
  (206, "Namco 118/Tengen MIMIC-1"),
  (210, "Namco 175/340"),
];

#[derive(Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct DummyMapper;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for DummyMapper {
  fn new(_: &CartHeader, _: &mut MemConfig) -> Box<Self> {
    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut MemConfig, _: usize, _: u8) {}
}

// Mapper 00
// https://www.nesdev.org/wiki/NROM

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct NROM;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for NROM {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);

    if header.prg_size <= 16 * 1024 {
      banks.prg.set_page(1, 0);
    } else {
      banks.prg.set_page(1, 1);
    }

    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut MemConfig, _: usize, _: u8) {}
}

// Mapper 02
// https://www.nesdev.org/wiki/UxROM
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UxROM {
  banked_page: u8,
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for UxROM {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 2);

    // https://www.nesdev.org/wiki/INES_Mapper_180
    let banked_page = if header.mapper == 180 {
      1
    } else {
      cfg.prg.set_page_to_last_bank(1);
      0
    };

    Box::new(Self { banked_page })
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, _: usize, val: u8) {
    let select = val & 0b1111;
    cfg.prg.set_page(self.banked_page as usize, select as usize);
  }
}

// Mapper 03
// https://www.nesdev.org/wiki/INES_Mapper_003
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CNROM;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for CNROM {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.chr = Banking::new_chr(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, _: usize, val: u8) {
    cfg.chr.set_page(0, val as usize);
  }
}

// Mapper 07
// https://www.nesdev.org/wiki/AxROM
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AxROM;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for AxROM {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, _: usize, val: u8) {
    let bank = val as usize & 0b111;
    cfg.prg.set_page(0, bank);
    let mirroring = match val & 0b1_0000 != 0 {
      false => Mirroring::SingleScreenA,
      true => Mirroring::SingleScreenB,
    };
    cfg.vram.update(mirroring);
  }
}

// Mapper 11
// https://www.nesdev.org/wiki/Color_Dreams
// TODO: ColorDreams and GxRom are basically the same, merge into one
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ColorDreams;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for ColorDreams {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 1);
    cfg.chr = Banking::new_chr(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, _: usize, val: u8) {
    let prg_bank = val as usize & 0b11;
    let chr_bank = val as usize >> 4;

    cfg.prg.set_page(0, prg_bank);
    cfg.chr.set_page(0, chr_bank);
  }
}

// Mapper 66
// https://www.nesdev.org/wiki/GxROM
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct GxROM;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for GxROM {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 1);
    cfg.chr = Banking::new_chr(header, 1);

    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, _: usize, val: u8) {
    let chr_bank = val as usize & 0b11;
    let prg_bank = (val as usize >> 4) & 0b11;

    cfg.prg.set_page(0, prg_bank);
    cfg.chr.set_page(0, chr_bank);
  }
}

// Mapper 71
// https://www.nesdev.org/wiki/INES_Mapper_071
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Codemasters;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for Codemasters {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 2);
    cfg.prg.set_page_to_last_bank(1);
    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0x9000..=0x9FFF => {
        let mirroring = match (val >> 4) & 1 != 0 {
          false => Mirroring::SingleScreenA,
          true => Mirroring::SingleScreenB,
        };
        cfg.vram.update(mirroring);
      }
      0xC000..=0xFFFF => {
        let bank = val as usize & 0b1111;
        cfg.prg.set_page(0, bank);
      }
      _ => {}
    }
  }
}

// Mapper 78 (Holy Diver and Cosmo Carrier)
// https://www.nesdev.org/wiki/INES_Mapper_078
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct INesMapper078 {
  uses_hv_mirroring: bool,
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for INesMapper078 {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    let uses_hv_mirroring = header.has_alt_mirroring || header.submapper == 3;

    cfg.prg = Banking::new_prg(header, 2);
    cfg.chr = Banking::new_chr(header, 1);

    let mirroring = if uses_hv_mirroring {
      Mirroring::Horizontal
    } else {
      Mirroring::SingleScreenA
    };

    cfg.prg.set_page_to_last_bank(1);
    cfg.vram.update(mirroring);

    Box::new(Self { uses_hv_mirroring })
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, _: usize, val: u8) {
    let prg_bank = val & 0b111;
    let chr_bank = val >> 4;

    cfg.prg.set_page(0, prg_bank as usize);
    cfg.chr.set_page(0, chr_bank as usize);

    let mirroring = if self.uses_hv_mirroring {
      match (val >> 3) & 1 != 0 {
        false => Mirroring::Horizontal,
        true => Mirroring::Vertical,
      }
    } else {
      match (val >> 3) & 1 != 0 {
        false => Mirroring::SingleScreenA,
        true => Mirroring::SingleScreenB,
      }
    };

    cfg.vram.update(mirroring);
  }
}

// Mapper 31
// https://www.nesdev.org/wiki/INES_Mapper_031
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct INesMapper031;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for INesMapper031 {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 8);
    cfg.prg.set_page_to_last_bank(7);

    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut MemConfig, _: usize, _: u8) {}

  fn cart_write(&mut self, cfg: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0x5000..=0x5FFFF => cfg.prg.set_page(addr, val as usize),
      _ => {}
    }
  }
}

// Mapper 75
// https://www.nesdev.org/wiki/VRC1
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct VRC1;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for VRC1 {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 4);
    cfg.prg.set_page_to_last_bank(3);
    cfg.chr = Banking::new_chr(header, 2);
    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0x8000..=0x8FFF => cfg.prg.set_page(0, val as usize & 0b1111),
      0xA000..=0xAFFF => cfg.prg.set_page(1, val as usize & 0b1111),
      0xC000..=0xCFFF => cfg.prg.set_page(2, val as usize & 0b1111),
      0x9000..=0x9FFF => {
        let mirroring = match val & 1 != 0 {
          false => Mirroring::Vertical,
          true => Mirroring::Horizontal,
        };
        cfg.vram.update(mirroring);

        let bank0 = cfg.chr.bankings[0];
        let bank0_hi = (val as usize >> 1) & 1;
        cfg.chr.set_page(0, (bank0_hi << 5) | bank0);

        let bank1 = cfg.chr.bankings[1];
        let bank1_hi = (val as usize >> 1) & 1;
        cfg.chr.set_page(1, (bank1_hi << 5) | bank1);
      }
      0xE000..=0xEFFF => cfg.chr.set_page(0, val as usize),
      0xF000..=0xFFFF => cfg.chr.set_page(1, val as usize),
      _ => {}
    }
  }
}

// Mapper 206
// https://www.nesdev.org/wiki/INES_Mapper_206
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct INesMapper206 {
  mmc3: MMC3,
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for INesMapper206 {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    let mmc3 = *MMC3::new(header, cfg);
    Box::new(Self { mmc3 })
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, addr: usize, val: u8) {
    self.mmc3.prg_write(cfg, addr, val);
  }
}

// Mapper 87
// https://www.nesdev.org/wiki/INES_Mapper_087
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct INesMapper087;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for INesMapper087 {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 2);

    if header.prg_size <= 16 * 1024 {
      cfg.prg.set_page(1, 0);
    } else {
      cfg.prg.set_page(1, 1);
    }
    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, _: usize, val: u8) {
    let bank = ((val & 0b01) << 1) | ((val & 0b10) >> 1);
    cfg.chr.set_page(0, bank as usize);
  }
}

// Mapper 34
// https://www.nesdev.org/wiki/INES_Mapper_034
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct INesMapper034 {
  submapper: u8,
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for INesMapper034 {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    let submapper = if header.submapper != 0 {
      header.submapper
    } else if header.chr_real_size() > 8 * 1024 {
      1
    } else {
      2
    };

    cfg.prg = Banking::new_prg(header, 1);

    if submapper == 2 {
      cfg.chr = Banking::new_chr(header, 1);
    } else {
      cfg.chr = Banking::new_chr(header, 2);
    }

    Box::new(Self { submapper })
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, addr: usize, val: u8) {
    match (addr, self.submapper) {
      (0x7FFD, 1) | (0x8000..=0xFFFF, 2) => cfg.prg.set_page(0, val as usize & 0b11),
      (0x7FFE, 1) => cfg.chr.set_page(0, val as usize & 0b1111),
      (0x7FFF, 1) => cfg.chr.set_page(1, val as usize & 0b1111),
      _ => {}
    }
  }
}

// Mapper 13
// https://www.nesdev.org/wiki/CPROM
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct CPROM;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for CPROM {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.chr = Banking::new_chr(header, 2);
    Box::new(Self)
  }

  fn prg_write(&mut self, cfg: &mut MemConfig, _: usize, val: u8) {
    cfg.chr.set_page(1, val as usize & 0b11);
  }
}
