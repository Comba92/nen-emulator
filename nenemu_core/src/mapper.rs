use crate::{
    bus::{Banking, Bus, CpuHandler, IrqFlags, PpuHandler},
    emu::Mirroring,
    utils::{byte_set_hi, byte_set_lo},
};

use crate::mapper::{fds::*, konami::*, mmc1::*, mmc2::*, mmc3::*, mmc5::*, namco::*, sunsoft::*};

pub mod fds;
mod konami;
mod mmc1;
mod mmc2;
mod mmc3;
mod mmc5;
mod namco;
mod sunsoft;

// https://www.nesdev.org/wiki/Mapper
#[cfg_attr(feature = "savestates", typetag::serde)]
pub trait Mapper: Send {
    fn new(mem: &mut Bus) -> Box<Self>
    where
        Self: Sized;

    // 0x8000..=0xffff
    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8);

    // 0x4020..=0x5fff
    fn io_read(&mut self, mem: &mut Bus, _addr: u16) -> u8 {
        mem.cpu_open_bus
    }
    fn io_write(&mut self, _mem: &mut Bus, _addr: u16, _val: u8) {}

    fn step(&mut self, _mem: &mut Bus, _cycles: usize) {}

    fn ppu_bus_callback(&mut self, _mem: &mut Bus, _addr: u16, _cycles: usize) {}
    fn cpu_bus_callback(&mut self, _mem: &mut Bus, _addr: u16, _val: Option<u8>) {}

    fn ppu_special_read(&mut self, _mem: &mut Bus, _addr: u16) -> u8 {
        0
    }

    fn special_input(&mut self) {}

    fn sample(&self) -> f32 {
        0.0
    }
}

pub type BoxedMapper = Box<dyn Mapper>;

pub fn new(mem: &mut Bus) -> Result<BoxedMapper, String> {
    let mapper: BoxedMapper = match mem.header.mapper {
        0 => NROM::new(mem),
        1 => MMC1::new(mem),
        2 | 94 | 180 => UxROM::new(mem),
        // 3 | 185 => CNROM::new(mem),
        3 => CNROM::new(mem),
        4 => MMC3::new(mem),
        5 => MMC5::new(mem),
        7 => AxROM::new(mem),
        9 | 10 => MMC2::new(mem),
        11 => ColorDreams::new(mem),
        13 => CPROM::new(mem),
        16 | 153 | 157 | 159 => BandaiFCG::new(mem),
        19 | 210 => Namco129_163::new(mem),
        20 => FDS::new(mem),
        21 | 22 | 23 | 25 => VRC2_4::new(mem),
        24 | 26 => VRC6::new(mem),
        29 => Homebrews29::new(mem),
        31 => NSF::new(mem),
        34 | 177 | 241 => NINA00x_BNROM::new(mem),
        // 32 => IremG101::new(mem),
        40 => NTDEC2722::new(mem),
        // 65 => IremH3001::new(mem),
        66 => GxROM::new(mem),
        67 => Sunsoft3::new(mem),
        68 => Sunsoft4::new(mem),
        69 => SunsoftFME7::new(mem),
        70 | 152 => Bandai74::new(mem),
        71 | 232 => Codemasters::new(mem),
        73 => VRC3::new(mem),
        75 => VRC1::new(mem),
        77 => NapoleonSenki::new(mem),
        78 => Irem74HCx::new(mem),
        79 => NINA003_006::new(mem),
        85 => VRC7::new(mem),
        87 | 101 => J87::new(mem),
        89 => Sunsoft89::new(mem),
        93 => Sunsoft93::new(mem),
        97 => IremTAMS1::new(mem),
        184 => Sunsoft1::new(mem),
        206 | 154 | 95 | 88 | 76 => DxROM::new(mem),
        _ => return Err(format!("mapper {} not implemented", mem.header.mapper)),
    };

    Ok(mapper)
}

// https://www.nesdev.org/wiki/NROM

#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub(crate) struct NROM;

#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for NROM {
    fn new(_: &mut Bus) -> Box<Self> {
        Box::new(Self)
    }

    fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}
}

// https://www.nesdev.org/wiki/UxROM
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct UxROM {
    bank: u8,
    shift: u8,
}

#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for UxROM {
    fn new(mem: &mut Bus) -> Box<Self> {
        let shift = if mem.header.mapper == 94 { 2 } else { 0 };
        let swapped = if mem.header.mapper == 180 { 1 } else { 0 };

        Box::new(Self {
            bank: swapped,
            shift,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks
            .prg
            .set_page(self.bank, (val >> self.shift) as u16);
    }
}

// https://www.nesdev.org/wiki/CNROM
// https://www.nesdev.org/wiki/CNROM#Mapper_185
// TODO: mapper 185
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct CNROM;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for CNROM {
    fn new(mem: &mut Bus) -> Box<Self> {
        // The Namco game Hayauchi Super Igo adds 2 KiB of PRG-RAM, denoted using mapper 3 and the appropriate value in the header's PRG-RAM size field.
        if mem.header.wram_size > 0 {
            mem.banks.wram = Banking::new(0x6000, 2 * 1024, 8 * 1024, 4);
        }
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.chr.set_page(0, val as u16);
    }
}

// https://www.nesdev.org/wiki/GxROM
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct GxROM;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for GxROM {
    fn new(_: &mut Bus) -> Box<Self> {
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, val >> 4);
        mem.banks.chr.set_page(0, val);
    }
}

// https://www.nesdev.org/wiki/AxROM
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct AxROM;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for AxROM {
    fn new(_: &mut Bus) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.prg.set_page(0, val as u16);

        let mirroring = if val & 0x10 == 0 {
            Mirroring::LowTable
        } else {
            Mirroring::HighTable
        };
        mem.banks.vram.mirror(&mirroring);
    }
}

// https://www.nesdev.org/wiki/Color_Dreams
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct ColorDreams;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for ColorDreams {
    fn new(_: &mut Bus) -> Box<Self> {
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, val & 0x3);
        mem.banks.chr.set_page(0, val >> 4);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_071
// https://www.nesdev.org/wiki/INES_Mapper_232
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Codemasters {
    mapper: u16,
    prg_block: u8,
    prg_bank: u8,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Codemasters {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new_prg(&mem.header, 2);
        // this starts at last bank for some reason
        mem.banks.prg.fix_last_page();

        Box::new(Self {
            mapper: mem.header.mapper,
            ..Default::default()
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        match (addr & 0xf000, self.mapper) {
            (0x8000..=0xb000, 232) => {
                // https://www.nesdev.org/wiki/INES_Mapper_232
                self.prg_block = (val >> 3) & 0b11;
                self.prg_bank = (self.prg_block << 2) | (self.prg_bank & 0x3);
                mem.banks.prg.set_page(0, self.prg_bank as u16);
                // CAREFUL: last page should be relative to current block
                mem.banks
                    .prg
                    .set_page(1, (self.prg_block << 2) as u16 | 0x3);
            }
            // For compatibility without using a submapper, FCEUX begins all games with fixed mirroring, and applies single screen mirroring only once $9000-9FFF is written, ignoring writes to $8000-8FFF.
            (0x9000, _) => {
                if val & 0x10 == 0 {
                    mem.banks.vram.mirror(&Mirroring::LowTable);
                } else {
                    mem.banks.vram.mirror(&Mirroring::HighTable);
                }
            }
            (0xc000..=0xf000, 71) => mem.banks.prg.set_page(0, val as u16),
            (0xc000..=0xf000, 232) => {
                self.prg_bank = (self.prg_bank & 0xc) | (val & 0b11);
                mem.banks.prg.set_page(0, self.prg_bank as u16);
            }
            _ => {}
        }
    }
}

// https://www.nesdev.org/wiki/CPROM
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct CPROM;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for CPROM {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.chr = Banking::new_chr(&mem.header, 2);
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.chr.set_page(1, val as u16);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_031
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct NSF;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for NSF {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new_prg(&mem.header, 8);
        Box::new(Self)
    }

    fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}
    fn io_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        if (addr >> 12) == 0x5 {
            mem.banks.prg.set_page(addr as u8 & 0b111, val as u16);
        }
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_078
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Irem74HCx {
    is_holy_diver: bool,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Irem74HCx {
    fn new(mem: &mut Bus) -> Box<Self> {
        Box::new(Self {
            is_holy_diver: mem.header.submapper == 3 || mem.header.alt_mirroring,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, val);
        mem.banks.chr.set_page(0, val >> 4);

        let mirroring = match (self.is_holy_diver, val & 0x8) {
            (true, 0) => Mirroring::Horizontal,
            (true, _) => Mirroring::Vertical,
            (false, 0) => Mirroring::LowTable,
            (false, _) => Mirroring::HighTable,
        };
        mem.banks.vram.mirror(&mirroring);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_016
// https://www.nesdev.org/wiki/INES_Mapper_153
// https://www.nesdev.org/wiki/INES_Mapper_157
// https://www.nesdev.org/wiki/INES_Mapper_159
// TODO: eeprom
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct BandaiFCG {
    mapper: u16,
    submapper: u8,
    prg_bank: u8,
    irq_enabled: bool,
    irq_latch: u16,
    irq_count: u16,
}
impl BandaiFCG {
    fn write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        match (addr & 0xf, self.mapper) {
            (0x0..=0x7, 16 | 159) => mem.banks.chr.set_page(addr as u8 & 0xf, val as u16),
            (0x0..=0x3, 153) => {
                let prg_block = val & 1;
                self.prg_bank = (prg_block << 4) | (self.prg_bank & 0x0f);
                mem.banks.prg.set_page(0, self.prg_bank as u16);

                let last_bank = if prg_block == 0 {
                    mem.banks.prg.banks_count / 2 - 1
                } else {
                    mem.banks.prg.banks_count - 1
                };
                mem.banks.prg.set_page(1, last_bank);
            }
            (0x0..=0x3, 157) => {
                // TODO: eeprom clock
            }
            (0x8, _) => {
                self.prg_bank = (self.prg_bank & 0xf0) | val;
                mem.banks.prg.set_page(0, self.prg_bank as u16);
            }
            (0x9, _) => {
                let mirroring = match val & 0x3 {
                    0 => Mirroring::Vertical,
                    1 => Mirroring::Vertical,
                    2 => Mirroring::LowTable,
                    _ => Mirroring::HighTable,
                };
                mem.banks.vram.mirror(&mirroring);
            }
            (0xa, _) => {
                self.irq_enabled = val & 1 > 0;
                if self.irq_enabled && self.irq_count == 0 {
                    mem.irq.insert(IrqFlags::MAPPER);
                } else {
                    mem.irq.remove(IrqFlags::MAPPER);
                }

                if self.submapper == 5 {
                    self.irq_count = self.irq_latch;
                }
            }
            (0xb, _) => {
                if self.submapper == 4 {
                    self.irq_count = byte_set_lo(self.irq_count, val);
                } else if self.submapper == 5 {
                    self.irq_latch = byte_set_lo(self.irq_latch, val);
                }
            }
            (0xc, _) => {
                if self.submapper == 4 {
                    self.irq_count = byte_set_hi(self.irq_count, val);
                } else if self.submapper == 5 {
                    self.irq_latch = byte_set_hi(self.irq_latch, val);
                }
            }
            (0xd, 16 | 159) => {
                if self.submapper == 5 {
                    // TODO: eeprom ctrl
                }
            }
            (0xd, 157) => {
                // TODO: eeprom ctrl
            }
            (0xd, 153) => mem.wram_enable(val & 0x20 > 0),
            _ => {}
        }
    }
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for BandaiFCG {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.chr = Banking::new_chr(&mem.header, 8);

        if mem.header.mapper == 153 {
            // needed for Famicom Jump II
            _ = getrandom::fill(&mut mem.wram);

            // has two prg blocks, last bank should be mid
            mem.banks.prg.set_page(1, mem.banks.prg.banks_count / 2 - 1);
        } else {
            // has eeprom
            mem.set_wram_handlers(CpuHandler::Mapper);

            // no prg blocks
            mem.banks.prg.fix_last_page();
        }

        if matches!(mem.header.mapper, 153 | 157) {
            // chr is unbanked
            for i in 0..8 {
                mem.banks.chr.set_page(i, i as u16);
            }
        }

        let submapper = if mem.header.mapper == 16 {
            mem.header.submapper
        } else {
            // all other work as submapper 5
            5
        };

        Box::new(Self {
            mapper: mem.header.mapper,
            submapper,
            ..Default::default()
        })
    }

    fn io_read(&mut self, mem: &mut Bus, _addr: u16) -> u8 {
        // TODO: eeprom read for 16, 157, 159
        mem.cpu_open_bus
    }

    fn io_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        if self.submapper == 4 {
            self.write(mem, addr, val);
        }
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        if self.submapper == 5 {
            self.write(mem, addr, val);
        }
    }

    fn step(&mut self, mem: &mut Bus, _cycles: usize) {
        if self.irq_enabled {
            if self.irq_count == 0 {
                if self.submapper == 5 {
                    self.irq_count = self.irq_latch;
                }
                mem.irq.insert(IrqFlags::MAPPER);
            }
            self.irq_count -= 1;
        }
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_152
// https://www.nesdev.org/wiki/INES_Mapper_070
// very similiar to Sunsoft89
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Bandai74 {
    mapper: u16,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Bandai74 {
    fn new(mem: &mut Bus) -> Box<Self> {
        Box::new(Self {
            mapper: mem.header.mapper,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.chr.set_page(0, val);

        if self.mapper == 152 {
            mem.banks.prg.set_page(0, val >> 4);
            let mirroring = if val & 0x80 == 0 {
                Mirroring::LowTable
            } else {
                Mirroring::HighTable
            };
            mem.banks.vram.mirror(&mirroring);
        } else {
            mem.banks.prg.set_page(0, val >> 4);
        }
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_097
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct IremTAMS1;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for IremTAMS1 {
    fn new(_: &mut Bus) -> Box<Self> {
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.prg.set_page(1, val as u16);
        let mirroring = if val & 0x80 == 0 {
            Mirroring::Horizontal
        } else {
            Mirroring::Vertical
        };
        mem.banks.vram.mirror(&mirroring);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_040
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct NTDEC2722 {
    irq_enabled: bool,
    irq_count: u16,
    submapper: u8,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for NTDEC2722 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new(0x6000, mem.header.prg_size, 40 * 1024, 5);
        mem.set_wram_handlers(CpuHandler::Prg);

        mem.banks.prg.set_page(0, 6);
        mem.banks.prg.set_page(1, 4);
        mem.banks.prg.set_page(2, 5);
        // page 3 is controlled by register 0xe000
        mem.banks.prg.set_page(4, 7);

        Box::new(Self::default())
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        match addr & 0xe000 {
            0x8000 => {
                self.irq_enabled = false;
                self.irq_count = 0;
                mem.irq.remove(IrqFlags::MAPPER);
            }
            0xa000 => self.irq_enabled = true,

            0xc000 => {
                if self.submapper == 1 {
                    // TODO: submapper 1 stuff
                    todo!("NTDEC2722 submapper 1 stuff")
                }
            }
            0xe000 => mem.banks.prg.set_page(3, val as u16),
            _ => {}
        }
    }

    fn step(&mut self, mem: &mut Bus, _cycles: usize) {
        if self.irq_enabled {
            self.irq_count = self.irq_count.wrapping_add(1);
            if self.irq_count == 0x1000 {
                mem.irq.insert(IrqFlags::MAPPER)
            } else if self.irq_count == 0x2000 {
                // TODO: if the software doesn't acknowledge the interrupt for another 4096 cycles it will self-acknowledge.
                mem.irq.remove(IrqFlags::MAPPER);
            }
        }
    }
}

// TODO
// https://www.nesdev.org/wiki/INES_Mapper_032
// #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
// struct IremG101;

// https://www.nesdev.org/wiki/INES_Mapper_065
// #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
// struct IremH3001;

// https://www.nesdev.org/wiki/INES_Mapper_087
// https://www.nesdev.org/wiki/INES_Mapper_101
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct J87 {
    shift: u8,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for J87 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.set_wram_handlers(CpuHandler::Mapper);
        let shift = if mem.header.mapper == 87 { 1 } else { 0 };
        Box::new(Self { shift })
    }

    fn io_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let bank = ((val & 0x1) << self.shift) | ((val & 0x2) >> self.shift);
        mem.banks.chr.set_page(0, bank as u16);
    }

    fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}
}

// https://www.nesdev.org/wiki/INES_Mapper_034
// https://www.nesdev.org/wiki/INES_Mapper_177
// https://www.nesdev.org/wiki/INES_Mapper_241
#[allow(non_camel_case_types)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct NINA00x_BNROM {
    mapper: u16,
    submapper: u8,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for NINA00x_BNROM {
    fn new(mem: &mut Bus) -> Box<Self> {
        // should be considered BNROM when the CHR-ROM size is 0-8 KiB, and NINA-001/NINA-002 when the CHR-ROM size is above 8 KiB.
        let mut submapper = 0;
        if mem.header.submapper == 1 || mem.header.chr_size > 8 * 1024 {
            mem.banks.chr = Banking::new_chr(&mem.header, 2);
            submapper = 1;
        } else if mem.header.submapper == 2 || mem.header.chr_size <= 8 * 1024 {
            mem.banks.chr = Banking::new_chr(&mem.header, 1);
            submapper = 2;
        }

        let submapper = if mem.header.mapper == 34 {
            submapper
        } else {
            2
        };

        Box::new(Self {
            mapper: mem.header.mapper,
            submapper,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        match (addr, self.submapper) {
            (0x7ffd, 1) | (0x8000..=0xffff, 2) => {
                mem.banks.prg.set_page(0, val as u16);
                if self.mapper == 177 {
                    if val & 0x20 > 0 {
                        mem.banks.vram.mirror(&Mirroring::Vertical);
                    } else {
                        mem.banks.vram.mirror(&Mirroring::Horizontal);
                    }
                }
            }
            (0x7ffe, 1) => mem.banks.chr.set_page(0, val as u16),
            (0x7fff, 1) => mem.banks.chr.set_page(1, val as u16),
            _ => {}
        }
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_034
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct NINA003_006;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for NINA003_006 {
    fn new(_: &mut Bus) -> Box<Self>
    where
        Self: Sized,
    {
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        if addr & 0xe100 == 0x4100 {
            let val = val as u16;
            mem.banks.prg.set_page(0, (val >> 3) & 1);
            mem.banks.chr.set_page(0, val & 0x7);
        }
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_206
// https://www.nesdev.org/wiki/INES_Mapper_088
// https://www.nesdev.org/wiki/INES_Mapper_095
// https://www.nesdev.org/wiki/INES_Mapper_154
// https://www.nesdev.org/wiki/INES_Mapper_076
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct DxROM {
    select: u8,
    mapper: u16,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for DxROM {
    fn new(mem: &mut Bus) -> Box<Self> {
        if mem.header.alt_mirroring || mem.header.mirroring == Mirroring::FourScreens {
            mem.set_4screen_mirroring();
        }

        // same as MMC3
        mem.banks.prg = Banking::new_prg(&mem.header, 4);
        mem.banks.prg.set_page(2, mem.banks.prg.banks_count - 2);
        mem.banks.prg.fix_last_page();

        mem.banks.chr = Banking::new_chr(&mem.header, 8);
        mem.banks.chr.set_pages_aligned2(0, 0);
        mem.banks.chr.set_pages_aligned2(2, 0);

        if mem.header.mapper == 76 {
            mem.banks.chr = Banking::new_chr(&mem.header, 4);
        }

        Box::new(Self {
            select: 0,
            mapper: mem.header.mapper,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        let val = val as u16;

        if self.mapper == 154 {
            // Note that this bit is present over the entire 32kB range; it is not present in only odd or even addresses unlike the associated Namcot 108.
            if val & 0x40 > 0 {
                mem.banks.vram.mirror(&Mirroring::HighTable);
            } else {
                mem.banks.vram.mirror(&Mirroring::LowTable);
            }
        }

        match addr & 0xe001 {
            // (0x8000..=0x9fff, true)
            0x8000 => {
                self.select = val as u8 & 0x7;
            }

            // (0x8000..=0x9fff, false)
            0x8001 => {
                let mut val = val;
                if matches!(self.mapper, 88 | 154) {
                    // A possible way to implement this would be to mask the CHR ROM 1K bank output from the mapper by ANDing with $3F, and then OR it with $40 for N108 registers 2, 3, 4, and 5.
                    // https://github.com/SourMesen/Mesen2/blob/master/Core/NES/Mappers/Namco/Namco108_88.h
                    match self.select {
                        0 | 1 => val &= 0x3f,
                        6 | 7 => {}
                        _ => val |= 0x40,
                    }
                } else if self.mapper == 95 {
                    if self.select == 0 {
                        mem.banks.vram.set_page(0, (val & 0x20) >> 5);
                        mem.banks.vram.set_page(1, (val & 0x20) >> 5);
                    } else if self.select == 1 {
                        mem.banks.vram.set_page(2, (val & 0x20) >> 5);
                        mem.banks.vram.set_page(3, (val & 0x20) >> 5);
                    }
                }

                match self.select {
                    6 | 7 => mem.banks.prg.set_page(self.select - 6, val),
                    0 | 1 => mem.banks.chr.set_pages_aligned2(2 * self.select, val),
                    // cases 2..=5
                    _ => {
                        if self.mapper == 76 {
                            mem.banks.chr.set_page(self.select - 2, val);
                        } else {
                            mem.banks.chr.set_page((self.select - 2) + 4, val)
                        }
                    }
                }
            }

            _ => {}
        }
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_077
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct NapoleonSenki;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for NapoleonSenki {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.chr = Banking::new(0x0000, mem.header.chr_size, 2 * 1024, 1);

        // this games provides 8kb of chr ram + 2kb of vram
        // we simulate chr ram by extending our vram from 0x0800 to 0x2fff
        mem.vram = vec![0; 10 * 1024].into_boxed_slice();
        mem.banks.vram = Banking::new(0x800, 10 * 1024, 10 * 1024, 5);

        for i in 0..5 {
            mem.banks.vram.mappings[i] = i as u32 * 2048;
            // we have to set these manually because odd numbers of pages breaks everything
        }

        for i in 2..12 {
            mem.ppu_handlers_1kb[i] = PpuHandler::Vram;
        }

        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, val);
        mem.banks.chr.set_page(0, val >> 4);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_184
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Sunsoft1;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Sunsoft1 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.chr = Banking::new_chr(&mem.header, 2);
        mem.set_wram_handlers(CpuHandler::Mapper);
        Box::new(Self)
    }

    fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}

    fn io_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.chr.set_page(0, val);
        mem.banks.chr.set_page(1, val >> 4);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_093
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Sunsoft93;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Sunsoft93 {
    fn new(_: &mut Bus) -> Box<Self> {
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.prg.set_page(0, (val >> 4) as u16);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_089
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Sunsoft89;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Sunsoft89 {
    fn new(_: &mut Bus) -> Box<Self> {
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, val >> 4);
        mem.banks
            .chr
            .set_page(0, ((val & 0x80) >> 4) | (val & 0b111));

        let mirroring = if val & 0x8 == 0 {
            Mirroring::LowTable
        } else {
            Mirroring::HighTable
        };
        mem.banks.vram.mirror(&mirroring);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_067
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Sunsoft3 {
    irq_write: bool,
    irq_count: u16,
    irq_enabled: bool,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Sunsoft3 {
    fn new(mem: &mut Bus) -> Box<Self>
    where
        Self: Sized,
    {
        mem.banks.chr = Banking::new_chr(&mem.header, 4);

        Box::new(Self::default())
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        if addr & 0x8800 == 0x8000 {
            mem.irq.remove(IrqFlags::MAPPER);
        }

        let val = val as u16;
        match addr & 0xf800 {
            0x8800 => mem.banks.chr.set_page(0, val),
            0x9800 => mem.banks.chr.set_page(1, val),
            0xa800 => mem.banks.chr.set_page(2, val),
            0xb800 => mem.banks.chr.set_page(3, val),

            0xc800 => {
                self.irq_count = if !self.irq_write {
                    byte_set_hi(self.irq_count, val as u8)
                } else {
                    byte_set_lo(self.irq_count, val as u8)
                };
                self.irq_write = !self.irq_write;
            }

            0xd800 => {
                self.irq_enabled = val & 0x10 > 0;
                self.irq_write = false;
            }
            0xe800 => {
                let mirroring = match val & 0x3 {
                    0 => Mirroring::Vertical,
                    1 => Mirroring::Horizontal,
                    2 => Mirroring::LowTable,
                    _ => Mirroring::HighTable,
                };
                mem.banks.vram.mirror(&mirroring);
            }

            0xf800 => mem.banks.prg.set_page(0, val & 0xf),
            _ => {}
        }
    }

    fn step(&mut self, mem: &mut Bus, _cycles: usize) {
        if self.irq_enabled {
            if self.irq_count == 0 {
                mem.irq.insert(IrqFlags::MAPPER);
                self.irq_enabled = false;
            } else {
                self.irq_count -= 1;
            }
        }
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_068
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Sunsoft4 {
    uses_chr_rom: bool,
    mirroring: Mirroring,
    chr_table0: u16,
    chr_table1: u16,
}
impl Sunsoft4 {
    fn update_chr_banks(&mut self, mem: &mut Bus) {
        if !self.uses_chr_rom {
            mem.banks.vram.mirror(&self.mirroring);
            return;
        }

        let chr = &mut mem.banks.chr;
        match &self.mirroring {
            Mirroring::Vertical => {
                chr.set_page(8 + 0, self.chr_table0);
                chr.set_page(8 + 1, self.chr_table1);
                chr.set_page(8 + 2, self.chr_table0);
                chr.set_page(8 + 3, self.chr_table1);
            }
            Mirroring::Horizontal => {
                chr.set_page(8 + 0, self.chr_table0);
                chr.set_page(8 + 1, self.chr_table0);
                chr.set_page(8 + 2, self.chr_table1);
                chr.set_page(8 + 3, self.chr_table1);
            }
            Mirroring::LowTable => {
                for i in 8..12 {
                    chr.set_page(i, self.chr_table0);
                }
            }
            Mirroring::HighTable => {
                for i in 8..12 {
                    chr.set_page(i, self.chr_table1);
                }
            }
            // shouldn't have 4 screens mirroring
            _ => {}
        }
    }
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Sunsoft4 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.chr = Banking::new(0, mem.header.chr_size, 12 * 1024, 12);
        mem.banks.chr.set_pages_aligned2(0, 0);
        mem.banks.chr.set_pages_aligned2(2, 2);
        mem.banks.chr.set_pages_aligned2(4, 4);
        mem.banks.chr.set_pages_aligned2(6, 6);

        Box::new(Self {
            mirroring: mem.header.mirroring.clone(),
            ..Default::default()
        })
    }

    fn io_write(&mut self, _mem: &mut Bus, addr: u16, _val: u8) {
        // TODO: licensing IC
        match addr >> 12 {
            0x6 | 0x7 => {
                // Licensing IC Nantettatte Baseball
            }

            _ => {}
        }
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        let val = val as u16;
        match addr >> 12 {
            // mapper expects 2kb banks number, but we have 1kb bank slots, we need to shift
            0x8 => mem.banks.chr.set_pages_aligned2(0, val << 1),
            0x9 => mem.banks.chr.set_pages_aligned2(2, val << 1),
            0xa => mem.banks.chr.set_pages_aligned2(4, val << 1),
            0xb => mem.banks.chr.set_pages_aligned2(6, val << 1),

            0xc => {
                self.chr_table0 = 0x80 | val;
                if self.uses_chr_rom {
                    self.update_chr_banks(mem);
                }
            }

            0xd => {
                self.chr_table1 = 0x80 | val;
                if self.uses_chr_rom {
                    self.update_chr_banks(mem);
                }
            }

            0xe => {
                self.mirroring = match val & 0b11 {
                    0 => Mirroring::Vertical,
                    1 => Mirroring::Horizontal,
                    2 => Mirroring::LowTable,
                    _ => Mirroring::HighTable,
                };

                let mode = val & 0x10 > 0;
                if mode != self.uses_chr_rom {
                    let handler = if mode {
                        PpuHandler::ChrRom
                    } else {
                        PpuHandler::Vram
                    };
                    mem.set_vram_handlers(handler);
                    self.uses_chr_rom = mode;
                }

                self.update_chr_banks(mem);
            }
            0xf => {
                mem.banks.prg.set_page(0, val);
                mem.wram_enable(val & 0x10 > 0);
            }
            _ => {}
        }
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_029
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Homebrews29;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Homebrews29 {
    fn new(_: &mut Bus) -> Box<Self> {
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.prg.set_page(0, (val >> 2) as u16);
        mem.banks.chr.set_page(0, val as u16);
    }
}
