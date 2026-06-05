use crate::{
    apu,
    bus::{Banking, Bus, ChrBank, CpuHandler, IrqFlags, PpuHandler},
    emu::Mirroring,
    utils::{byte_set_hi, byte_set_lo},
};

mod konami;
use konami::*;

mod mmc5;
use mmc5::*;

pub mod fds;
use fds::*;

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
        mem.cpu_data_bus
    }
    // TODO: consider getting rid of this and use handlers
    fn io_write(&mut self, _mem: &mut Bus, _addr: u16, _val: u8) {}
    fn step(&mut self, _mem: &mut Bus, _cycles: usize) {}

    fn notify_ppu_addr(&mut self, _mem: &mut Bus, _cycles: usize) {}
    fn notify_cpu_addr(&mut self, _mem: &mut Bus, _addr: u16, _val: Option<u8>) {}

    fn ppu_special_read(&mut self, _mem: &mut Bus, _addr: u16) -> u8 {
        0
    }
    fn special_input(&mut self) {}

    fn sample(&self) -> f32 {
        0.0
    }
}

pub type BoxedMapper = Box<dyn Mapper + Send>;

pub fn new(mem: &mut Bus) -> Result<BoxedMapper, String> {
    let mapper: BoxedMapper = match mem.header.mapper {
        0 => NROM::new(mem),
        1 => MMC1::new(mem),
        2 | 94 | 180 => UxROM::new(mem),
        3 | 185 => CNROM::new(mem),
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
    fn new(mem: &mut Bus) -> Box<Self> {
        if mem.header.prg_size > 16 * 1024 {
            // if we have 32 kb, no mirroring, we have mirroring by default
            mem.banks.prg = Banking::new_prg(&mem.header, 1);
        }

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
        let (swapped, fixed) = if mem.header.mapper == 180 {
            (1, 0)
        } else {
            (0, 1)
        };
        mem.banks.prg.fix_last_page();

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
        if mem.header.prg_size <= 16 * 1024 {
            mem.banks.prg.fix_last_page();
        } else {
            mem.banks.prg = Banking::new_prg(&mem.header, 1);
        }

        mem.banks.chr = Banking::new_chr(&mem.header, 1);
        // The Namco game Hayauchi Super Igo adds 2 KiB of PRG-RAM, denoted using mapper 3 and the appropriate value in the header's PRG-RAM size field.
        mem.banks.wram = Banking::new(0x6000, 2 * 1024, 2 * 1024, 4);
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.chr.set_page(0, val as u16 & 0xf);
    }
}

// https://www.nesdev.org/wiki/GxROM
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct GxROM;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for GxROM {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new_prg(&mem.header, 1);

        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, (val >> 4) & 0b11);
        mem.banks.chr.set_page(0, val & 0b1111);
    }
}

// https://www.nesdev.org/wiki/AxROM
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct AxROM;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for AxROM {
    fn new(mem: &mut Bus) -> Box<Self>
    where
        Self: Sized,
    {
        mem.banks.prg = Banking::new_prg(&mem.header, 1);
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.prg.set_page(0, val as u16 & 0b111);

        let mirroring = if val & 0x10 == 0 {
            Mirroring::LowTable
        } else {
            Mirroring::HighTable
        };
        mem.banks.vram.mirror(&mirroring);
    }
}

mod mmc1 {
    #[derive(Default, Debug)]
    #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
    pub enum WramKind {
        Bank32,
        Bank16,
        #[default]
        Bank8,
    }
}

// Needs NES2.0 / db support for WRAM (NEW FINDING: only SOROM games have 2 different kind of RAM))

#[derive(Default, Debug)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct MMC1 {
    shift_reg: u8,
    shift_count: u8,

    prg_mode: u8,
    prg_bank: u16,
    prg_hi_bank: u16,

    // 512kb of prg
    has_big_prg: bool,
    last_bank: u16,
    wram_kind: mmc1::WramKind,

    chr_mode: bool,
    chr_bank0: u16,
    chr_bank1: u16,

    write_delay: u8,
}
impl MMC1 {
    fn update_all_banks(&mut self, mem: &mut Bus, val: u16) {
        if self.has_big_prg {
            self.prg_hi_bank = val & 0x10;

            if self.prg_hi_bank > 0 {
                // last bank is the real last
                self.last_bank = mem.banks.prg.banks_count - 1;
            } else {
                // last bank is the mid one
                self.last_bank = mem.banks.prg.banks_count / 2 - 1;
            }
        }

        use mmc1::WramKind;
        let wram = &mut mem.banks.wram;
        match self.wram_kind {
            WramKind::Bank16 => wram.set_page(0, (val >> 3) & 0x1),
            WramKind::Bank32 => wram.set_page(0, (val >> 2) & 0x3),
            _ => {}
        }

        self.update_prg_banks(mem);
        self.update_chr_banks(mem);
    }

    fn update_prg_banks(&mut self, mem: &mut Bus) {
        let bank = self.prg_hi_bank | self.prg_bank;
        match self.prg_mode {
            2 => {
                // 2: fix first bank at $8000 and switch 16 KB bank at $C000
                mem.banks.prg.set_page(0, 0);
                mem.banks.prg.set_page(1, bank);
            }
            3 => {
                // 3: fix last bank at $C000 and switch 16 KB bank at $8000)
                mem.banks.prg.set_page(0, bank);
                // CAREFUL HERE: if we have 512kb, this has still the be the last 256kb bank of the current block
                mem.banks.prg.set_page(1, self.last_bank);
            }
            _ => {
                // 0, 1: switch 32 KB at $8000, ignoring low bit of bank number;
                mem.banks.prg.set_pages_aligned2(0, bank);
            }
        }
    }

    fn update_chr_banks(&mut self, mem: &mut Bus) {
        if self.chr_mode {
            mem.banks.chr.set_page(0, self.chr_bank0);
            mem.banks.chr.set_page(1, self.chr_bank1);
        } else {
            mem.banks.chr.set_pages_aligned2(0, self.chr_bank0 << 0);
        }
    }
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for MMC1 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.chr = Banking::new_chr(&mem.header, 2);

        let has_big_prg = mem.header.prg_size >= 512 * 1024;
        let last_bank = if has_big_prg {
            // start with mid bank
            mem.banks.prg.banks_count / 2 - 1
        } else {
            // will always be real last
            mem.banks.prg.banks_count - 1
        };

        let wram_kind = if mem.header.wram_size >= 32 * 1024 {
            mmc1::WramKind::Bank32
        } else if mem.header.wram_size >= 16 * 1024 {
            mmc1::WramKind::Bank16
        } else {
            mmc1::WramKind::Bank8
        };

        let mut res = Self {
            has_big_prg,
            wram_kind,
            last_bank,
            prg_mode: 3,
            ..Default::default()
        };

        res.update_prg_banks(mem);
        res.update_chr_banks(mem);

        Box::new(res)
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        if self.write_delay > 0 {
            self.write_delay = 2;
            return;
        }
        self.write_delay = 2;

        if val & 0x80 != 0 {
            self.shift_reg = 0;
            self.shift_count = 0;

            // back to mode3
            self.prg_mode = 3;
            self.update_prg_banks(mem);

            return;
        }

        self.shift_reg |= (val & 1) << self.shift_count;
        self.shift_count += 1;

        if self.shift_count < 5 {
            return;
        }

        let val = self.shift_reg;
        self.shift_reg = 0;
        self.shift_count = 0;

        match addr & 0xe000 {
            // 0x8000..=0x9fff => {
            0x8000 => {
                let mirroring = match val & 0x3 {
                    0 => Mirroring::LowTable,
                    1 => Mirroring::HighTable,
                    2 => Mirroring::Vertical,
                    _ => Mirroring::Horizontal,
                };
                mem.banks.vram.mirror(&mirroring);

                self.prg_mode = (val as u8 >> 2) & 0x3;
                self.update_prg_banks(mem);

                self.chr_mode = val & 0x10 > 0;
                self.update_chr_banks(mem);
            }
            0xa000..=0xbfff => {
                // 0xa000 => {
                self.chr_bank0 = val as u16;
                self.update_all_banks(mem, self.chr_bank0);
            }
            // 0xc000..=0xdfff => {
            0xc000 => {
                self.chr_bank1 = val as u16;
                if self.chr_mode {
                    self.update_all_banks(mem, self.chr_bank1);
                }
            }
            // 0xe000..=0xffff => {
            0xe000 => {
                self.prg_bank = val as u16 & 0xf;
                self.update_prg_banks(mem);

                mem.wram_enable(val & 0x10 == 0);
            }
            _ => {}
        }
    }

    fn step(&mut self, _mem: &mut Bus, _cycles: usize) {
        if self.write_delay > 0 {
            self.write_delay -= 1;
        }
    }
}

mod mmc2 {
    #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
    pub enum Latch {
        FD,
        FE,
    }
}

// https://www.nesdev.org/wiki/MMC2
// https://www.nesdev.org/wiki/MMC4
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct MMC2 {
    // TODO: do we really need a banking object here? probably just four registers
    // we can do that (tested) but we'd like to precompute the set_page() on prg_write
    bank_fd: Banking<ChrBank>,
    bank_fe: Banking<ChrBank>,
    latch0: mmc2::Latch,
    latch1: mmc2::Latch,
    mapper: u16,
}

#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for MMC2 {
    fn new(mem: &mut Bus) -> Box<Self>
    where
        Self: Sized,
    {
        if mem.header.mapper == 9 {
            // MMC2
            mem.banks.prg = Banking::new_prg(&mem.header, 4);
            let last_bank = mem.banks.prg.banks_count - 1;
            mem.banks.prg.set_page(1, last_bank - 2);
            mem.banks.prg.set_page(2, last_bank - 1);
            mem.banks.prg.set_page(3, last_bank);
        } else if mem.header.mapper == 10 {
            // MMC4
            // only two 16 kb pages
            mem.banks.prg.fix_last_page();
        }

        mem.banks.chr = Banking::new_chr(&mem.header, 2);

        Box::new(Self {
            bank_fd: Banking::new_chr(&mem.header, 2),
            bank_fe: Banking::new_chr(&mem.header, 2),
            latch0: mmc2::Latch::FD,
            latch1: mmc2::Latch::FD,
            mapper: mem.header.mapper,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        let val = val as u16;

        match addr >> 12 {
            0xa => mem.banks.prg.set_page(0, val & 0xf),
            0xb => self.bank_fd.set_page(0, val & 0x1f),
            0xc => self.bank_fe.set_page(0, val & 0x1f),
            0xd => self.bank_fd.set_page(1, val & 0x1f),
            0xe => self.bank_fe.set_page(1, val & 0x1f),
            0xf => {
                let mirroring = match val & 1 {
                    0 => Mirroring::Vertical,
                    _ => Mirroring::Horizontal,
                };

                mem.banks.vram.mirror(&mirroring);
            }
            _ => {}
        }
    }

    fn notify_ppu_addr(&mut self, mem: &mut Bus, _: usize) {
        use mmc2::Latch;
        let banks = &mut mem.banks;

        match (mem.ppu_addr_bus, self.mapper) {
            (0x0fd8, 9) | (0xfd8..=0xfdf, 10) => self.latch0 = Latch::FD,
            (0x0fe8, 9) | (0xfe8..=0xfef, 10) => self.latch0 = Latch::FE,
            (0x1fd8..=0x1fdf, _) => self.latch1 = Latch::FD,
            (0x1fe8..=0x1fef, _) => self.latch1 = Latch::FE,
            _ => {}
        }

        match self.latch0 {
            Latch::FD => banks.chr.bankings[0] = self.bank_fd.bankings[0],
            Latch::FE => banks.chr.bankings[0] = self.bank_fe.bankings[0],
        }

        match self.latch1 {
            Latch::FD => banks.chr.bankings[1] = self.bank_fd.bankings[1],
            Latch::FE => banks.chr.bankings[1] = self.bank_fe.bankings[1],
        }
    }
}

// https://www.nesdev.org/wiki/MMC3
// https://www.nesdev.org/wiki/MMC6
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct MMC3 {
    bank_select: u8,
    chr_invert: bool,

    prg_mode: u8,
    prg_swapped: u8,

    irq_count: u8,
    irq_latch: u8,
    irq_reload: bool,
    irq_enabled: bool,

    a12_low_count: usize,

    is_mmc6: bool,
}
// https://forums.nesdev.org/viewtopic.php?t=14056
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for MMC3 {
    fn new(mem: &mut Bus) -> Box<Self> {
        if mem.header.alt_mirroring || mem.header.mirroring == Mirroring::FourScreens {
            // MMC3 can have 4 screen mirroring
            mem.set_4screen_mirroring();
        }

        mem.banks.prg = Banking::new_prg(&mem.header, 4);
        // start with prg mode0
        mem.banks.prg.set_page(2, mem.banks.prg.banks_count - 2);
        mem.banks.prg.fix_last_page();

        mem.banks.chr = Banking::new_chr(&mem.header, 8);
        mem.banks.chr.set_pages_aligned2(0, 0);
        mem.banks.chr.set_pages_aligned2(2, 0);

        let is_mmc6 = mem.header.submapper == 1;

        if is_mmc6 {
            mem.banks.wram = Banking::new_wram(&mem.header, 8);
        }

        Box::new(Self {
            is_mmc6,
            ..Default::default()
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        match addr & 0xe001 {
            // (0x8000..=0x9fff, true)
            0x8000 => {
                self.bank_select = val & 0x7;

                let chr_invert = val & 0x80 > 0;
                if self.chr_invert != chr_invert {
                    for i in 0..4 {
                        mem.banks.chr.swap_pages(i, i + 4);
                    }

                    self.chr_invert = chr_invert;
                }

                let prg_mode = val & 0x40;
                if self.prg_mode != prg_mode {
                    mem.banks.prg.swap_pages(0, 2);

                    self.prg_swapped = if prg_mode == 0 { 0 } else { 2 };
                    self.prg_mode = prg_mode;
                }

                if self.is_mmc6 {
                    mem.wram_enable(val & 0x20 > 0);
                }
            }

            // (0x8000..=0x9fff, false)
            0x8001 => {
                let val = val as u16;
                match (self.bank_select, self.chr_invert) {
                    (6, _) => mem.banks.prg.set_page(self.prg_swapped, val & 0x3f),
                    (7, _) => mem.banks.prg.set_page(1, val & 0x3f),
                    (0 | 1, false) => mem.banks.chr.set_pages_aligned2(self.bank_select * 2, val),
                    (0 | 1, true) => mem
                        .banks
                        .chr
                        .set_pages_aligned2(self.bank_select * 2 + 4, val),
                    // cases 2..=5
                    (_, false) => mem.banks.chr.set_page((self.bank_select - 2) + 4, val),
                    (_, true) => mem.banks.chr.set_page(self.bank_select - 2, val),
                }
            }

            // (0xa000..=0xbfff, true)
            0xa000 => {
                // This bit has no effect on cartridges with hardwired 4-screen VRAM.
                if mem.vram.len() > 2 * 1024 {
                    return;
                }

                // inverted from what wiki says...
                let mirroring = match val & 1 {
                    0 => Mirroring::Vertical,
                    _ => Mirroring::Horizontal,
                };
                mem.banks.vram.mirror(&mirroring);
            }

            // (0xa000..=0xbfff, false)
            0xa001 => {
                let mode = val >> 6;
                let handler = match mode {
                    // 0b10, enabled, allow writes
                    2 => CpuHandler::Wram,
                    // 0b11, enabled, deny writes
                    3 => CpuHandler::WramReadOnly,
                    _ => CpuHandler::Mapper,
                };

                if self.is_mmc6 {
                    // TODO: sets writing protection for 512 byte blocks of wram!! we can't do that...
                } else {
                    mem.set_wram_handlers(handler);
                }
            }

            // (0xc000..=0xdfff, true)
            0xc000 => {
                self.irq_latch = val;
            }

            // (0xc000..=0xdfff, false)
            0xc001 => {
                self.irq_reload = true;
                self.irq_count = 0;
            }

            // (0xe000..=0xffff, true)
            0xe000 => {
                self.irq_enabled = false;
                mem.irq.remove(IrqFlags::MAPPER);
            }
            // (0xe000..=0xffff, false)
            0xe001 => self.irq_enabled = true,
            _ => {}
        }
    }

    fn notify_ppu_addr(&mut self, mem: &mut Bus, cycles: usize) {
        let a12_low = mem.ppu_addr_bus & 0x1000 == 0;

        let rising_edge = if !a12_low {
            let res = self.a12_low_count > 0 && cycles - self.a12_low_count >= 3;
            self.a12_low_count = 0;
            res
        } else if self.a12_low_count == 0 {
            self.a12_low_count = cycles;
            false
        } else {
            false
        };

        if rising_edge {
            if self.irq_reload || self.irq_count == 0 {
                self.irq_count = self.irq_latch;
                self.irq_reload = false;
            } else {
                self.irq_count -= 1;
            }

            if self.irq_enabled && self.irq_count == 0 {
                mem.irq.insert(IrqFlags::MAPPER);
            }
        }
    }
}

// https://www.nesdev.org/wiki/Color_Dreams
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct ColorDreams;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for ColorDreams {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new_prg(&mem.header, 1);
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, val & 0b11);
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
            (0xc000..=0xf000, 71) => mem.banks.prg.set_page(0, val as u16 & 0b1111),
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
        mem.banks.prg = Banking::new_prg(&mem.header, 1);
        mem.banks.chr = Banking::new_chr(&mem.header, 2);
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.chr.set_page(1, val as u16 & 0b11);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_031
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct NSF;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for NSF {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new_prg(&mem.header, 8);
        mem.banks.prg.fix_last_page();
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
        mem.banks.prg.fix_last_page();

        Box::new(Self {
            is_holy_diver: mem.header.submapper == 3 || mem.header.alt_mirroring,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, val & 0b111);
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
        mem.cpu_data_bus
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
        mem.banks.prg.fix_last_page();
        Box::new(Self {
            mapper: mem.header.mapper,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.chr.set_page(0, val & 0xf);

        if self.mapper == 152 {
            mem.banks.prg.set_page(0, (val >> 4) & 0b111);
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
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg.fix_last_page();
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.prg.set_page(1, val as u16 & 0x1f);
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
                // if the software doesn't acknowledge the interrupt for another 4096 cycles it will self-acknowledge.
                mem.irq.remove(IrqFlags::MAPPER);
            }
        }
    }
}

// TODO
// https://www.nesdev.org/wiki/INES_Mapper_032
// #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct IremG101;

// https://www.nesdev.org/wiki/INES_Mapper_065
// #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct IremH3001;

// https://www.nesdev.org/wiki/INES_Mapper_019
// https://www.nesdev.org/wiki/INES_Mapper_210

mod namco {
    #[cfg(feature = "savestates")]
    use serde_big_array::BigArray;

    #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
    pub(super) struct Audio {
        pub enabled: bool,
        #[cfg_attr(feature = "savestates", serde(with = "BigArray"))]
        ram: [u8; 128],
        addr: u8,
        auto_incr: bool,
        channel_curr: u8,
        outputs: [i16; 8],
        pub output: i16,
    }
    impl Default for Audio {
        fn default() -> Self {
            Self {
                enabled: false,
                ram: [0; 128],
                addr: 0,
                auto_incr: false,
                channel_curr: 0,
                outputs: [0; 8],
                output: 0,
            }
        }
    }
    impl Audio {
        const FREQ_LO: usize = 0;
        const PHASE_LO: usize = 1;
        const FREQ_MI: usize = 2;
        const PHASE_MI: usize = 3;
        const FREQ_HI: usize = 4;
        const PHASE_HI: usize = 5;
        const WAVE_ADDR: usize = 6;
        const VOLUME: usize = 7;

        pub fn write_addr(&mut self, val: u8) {
            self.addr = val & 0x7f;
            self.auto_incr = val & 0x80 > 0;
        }

        pub fn read_data(&mut self) -> u8 {
            let res = self.ram[self.addr as usize];

            if self.auto_incr {
                self.addr = (self.addr + 1) & 0x7f;
            }
            res
        }

        pub fn write_data(&mut self, val: u8) {
            self.ram[self.addr as usize] = val;

            if self.auto_incr {
                self.addr = (self.addr + 1) & 0x7f;
            }
        }

        fn wave_freq(&self, channel: u8) -> u32 {
            let channel = 0x40 + 8 * channel as usize;
            let mut freq = 0;
            freq |= self.ram[channel + Self::FREQ_LO] as u32;
            freq |= (self.ram[channel + Self::FREQ_MI] as u32) << 8;
            freq |= (self.ram[channel + Self::FREQ_HI] as u32 & 0x3) << 16;
            freq
        }

        fn wave_phase(&self, channel: u8) -> u32 {
            let channel = 0x40 + 8 * channel as usize;
            let mut phase = 0;
            phase |= self.ram[channel + Self::PHASE_LO] as u32;
            phase |= (self.ram[channel + Self::PHASE_MI] as u32) << 8;
            phase |= (self.ram[channel + Self::PHASE_HI] as u32) << 16;
            phase
        }

        fn set_wave_phase(&mut self, channel: u8, val: u32) {
            let channel = 0x40 + 8 * channel as usize;
            self.ram[channel + Self::PHASE_LO] = val as u8;
            self.ram[channel + Self::PHASE_MI] = (val >> 8) as u8;
            self.ram[channel + Self::PHASE_HI] = (val >> 16) as u8;
        }

        fn wave_len(&self, channel: u8) -> u32 {
            256 - (self.ram[0x40 + 8 * channel as usize + Self::FREQ_HI] as u32 & 0xfc)
        }

        fn wave_addr(&self, channel: u8) -> u8 {
            // address in 4bit samples, not bytes
            self.ram[0x40 + 8 * channel as usize + Self::WAVE_ADDR]
        }

        fn wave_pos(&self, channel: u8) -> u8 {
            // The high byte of the 24-bit phase value directly determines the current sample position of the channel
            self.ram[0x40 + 8 * channel as usize + Self::PHASE_HI]
        }

        fn wave_volume(&self, channel: u8) -> u8 {
            self.ram[0x40 + 8 * channel as usize + Self::VOLUME] & 0xf
        }

        fn channels_enabled(&self) -> u8 {
            1 + ((self.ram[0x7f] >> 4) & 0x7)
        }

        fn waves_update(&mut self) {
            let channel = 7 - self.channel_curr;

            let freq = self.wave_freq(channel);
            let mut phase = self.wave_phase(channel);
            let len = self.wave_len(channel);
            let addr = self.wave_addr(channel);
            let volume = self.wave_volume(channel);

            phase = (phase + freq) % (len << 16);
            self.set_wave_phase(channel, phase);
            let offset = self.wave_pos(channel);

            // The 'A' bits dictate where in the internal sound RAM the waveform starts. 'A' is the address in 4-bit samples, therefore a value of $02 would be the low 4 bits of address $01. A value of $03 would be the high 4 bits of address $01.
            let sample_pos = (addr + offset) as usize & 0xff;
            let sample = if sample_pos % 2 == 0 {
                // low bits
                self.ram[sample_pos / 2] & 0xf
            } else {
                // high bits
                self.ram[sample_pos / 2] >> 4
            };

            self.outputs[channel as usize] = (sample as i16 - 8) * volume as i16;
            self.channel_curr = (self.channel_curr + 1) % self.channels_enabled();

            let sum = self
                .outputs
                .iter()
                .rev() // start from last
                .take(self.channels_enabled() as usize) // take only enabled
                .sum::<i16>();

            self.output = sum / self.channels_enabled() as i16;
        }

        pub fn step(&mut self, cycles: usize) {
            if !self.enabled {
                return;
            }

            if cycles % 15 == 0 {
                self.waves_update();
            }
        }
    }
}

#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Namco129_163 {
    irq_count: u16,
    irq_enabled: bool,

    chr_ram0: bool,
    chr_ram1: bool,

    audio: namco::Audio,

    mapper: u16,
    submapper: u8,
}
// TODO: games with wram.len == 0 and battery should save the 128 bytes ram in audio struct
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Namco129_163 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new_prg(&mem.header, 4);
        mem.banks.prg.fix_last_page();

        if mem.header.mapper == 19 {
            // namco 129/163
            mem.banks.chr = Banking::new(0x0000, mem.header.chr_size, 12 * 1024, 12);
            mem.banks.vram = Banking::new(0x0000, 2 * 1024, 12 * 1024, 12);
        } else if mem.header.mapper == 210 {
            // namco 175/340
            mem.banks.chr = Banking::new_chr(&mem.header, 8);
        }

        Box::new(Self {
            irq_count: 0,
            irq_enabled: false,
            chr_ram0: false,
            chr_ram1: false,
            audio: namco::Audio::default(),
            mapper: mem.header.mapper,
            submapper: mem.header.submapper,
        })
    }

    fn io_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
        // TODO: use mask
        if self.mapper != 19 {
            return mem.cpu_data_bus;
        }

        match addr {
            0x4800..=0x4fff => self.audio.read_data(),
            0x5000..=0x57ff => self.irq_count as u8,
            0x5800..=0x5fff => ((self.irq_enabled as u8) << 7) | (self.irq_count >> 8) as u8,
            _ => mem.cpu_data_bus,
        }
    }

    fn io_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        if self.mapper != 19 {
            return;
        }

        // TODO: use mask
        match addr {
            0x4800..=0x4fff => self.audio.write_data(val),
            0x5000..=0x57ff => {
                self.irq_count = byte_set_lo(self.irq_count, val);
                mem.irq.remove(IrqFlags::MAPPER);
            }
            0x5800..=0x5fff => {
                self.irq_count = byte_set_hi(self.irq_count, val & 0x7f);
                self.irq_enabled = val & 0x7f > 0;
                mem.irq.remove(IrqFlags::MAPPER);
            }

            _ => {}
        }
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        // TODO: use mask
        match (addr, self.mapper) {
            (0x8000..=0xdfff, 19) => {
                let page = ((addr - 0x8000) / 0x800) as u8;
                let nametbl_enabled =
                    (page >= 8) || (page < 4 && !self.chr_ram0) || (page >= 4 && !self.chr_ram1);

                if val >= 0xe0 && nametbl_enabled {
                    // use nametables
                    mem.banks.vram.set_page(page, val as u16 & 1);
                    mem.ppu_handlers_1kb[page as usize] = PpuHandler::Vram;
                } else {
                    // use chr
                    mem.banks.chr.set_page(page, val as u16);
                    // All commercial-era titles only come with CHR-ROM.
                    mem.ppu_handlers_1kb[page as usize] = PpuHandler::ChrRom;
                }
            }

            (0x8000..=0xbfff, 210) => {
                let page = ((addr - 0x8000) / 0x800) as u8;
                mem.banks.chr.set_page(page, val as u16);
            }

            (0xc000..=0xc7ff, 210) => {
                // namco 175 only
                if self.submapper == 1 {
                    mem.wram_enable(val & 1 > 0);
                }
            }

            (0xe000..=0xe7ff, _) => {
                mem.banks.prg.set_page(0, val as u16 & 0x3f);
                self.audio.enabled = val & 0x40 == 0;

                // namco 340 only
                if self.mapper == 210 && self.submapper == 2 {
                    let mirroring = match val & 0xc0 {
                        0 => Mirroring::LowTable,
                        1 => Mirroring::Vertical,
                        2 => Mirroring::HighTable,
                        _ => Mirroring::Horizontal,
                    };
                    mem.banks.vram.mirror(&mirroring);
                }
            }

            (0xe800..=0xefff, _) => {
                mem.banks.prg.set_page(1, val as u16 & 0x3f);
                self.chr_ram0 = val & 0x40 > 0;
                self.chr_ram1 = val & 0x80 > 0;
            }
            (0xf000..=0xf7ff, _) => {
                mem.banks.prg.set_page(2, val as u16 & 0x3f);
            }
            (0xf800..=0xffff, 19) => {
                self.audio.write_addr(val);
                // TODO: write protect for exram for mapper 19
                // this works with 2kb windows, we cant really do it with 8kb handlers...
            }
            _ => {}
        }
    }

    fn step(&mut self, mem: &mut Bus, cycles: usize) {
        if self.mapper == 19 {
            if self.irq_enabled && self.irq_count < 0x7fff {
                self.irq_count += 1;
                if self.irq_count >= 0x7fff {
                    mem.irq.insert(IrqFlags::MAPPER);
                }
            }

            self.audio.step(cycles);
        }
    }

    fn sample(&self) -> f32 {
        self.audio.output as f32 * (apu::EXT_MIX * 0.5)
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_087
// https://www.nesdev.org/wiki/INES_Mapper_101
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct J87 {
    shift: u8,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for J87 {
    fn new(mem: &mut Bus) -> Box<Self> {
        if mem.header.prg_size > 16 * 1024 {
            mem.banks.prg = Banking::new_prg(&mem.header, 1);
        }
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
        mem.banks.prg = Banking::new_prg(&mem.header, 1);

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
    fn new(mem: &mut Bus) -> Box<Self>
    where
        Self: Sized,
    {
        mem.banks.prg = Banking::new_prg(&mem.header, 1);
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
                    6 | 7 => mem.banks.prg.set_page(self.select - 6, val & 0x3f),
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
        mem.banks.prg = Banking::new_prg(&mem.header, 1);
        mem.banks.chr = Banking::new(0x0000, mem.header.chr_size, 2 * 1024, 1);

        // this games provides 8kb of chr ram + 2kb of vram
        // we simulate chr ram by extending our vram from 0x0800 to 0x2fff
        mem.vram.resize(10 * 1024, 0);
        mem.banks.vram = Banking::new(0x800, 10 * 1024, 10 * 1024, 5);

        for i in 0..5 {
            mem.banks.vram.bankings[i] = i * 2048;
            // I HAVE NO CLUE WHAT THIS DOENST WORK and have to manually set the pages..
            // mem.banks.vram.set_page(i as u8, i);
        }

        for i in 2..12 {
            mem.ppu_handlers_1kb[i] = PpuHandler::Vram;
        }

        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, val & 0xf);
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
        mem.banks.chr.set_page(0, val & 0b111);
        mem.banks.chr.set_page(1, (val >> 4) & 0b111);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_093
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Sunsoft93;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Sunsoft93 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg.fix_last_page();
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.prg.set_page(0, (val >> 4) as u16 & 0b111);
    }
}

// https://www.nesdev.org/wiki/INES_Mapper_089
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Sunsoft89;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Sunsoft89 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg.fix_last_page();
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        let val = val as u16;
        mem.banks.prg.set_page(0, (val >> 4) & 0b111);
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
        mem.banks.prg.fix_last_page();
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
        mem.banks.prg.fix_last_page();
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
                mem.banks.prg.set_page(0, val & 0b1111);
                mem.wram_enable(val & 0x10 > 0);
            }
            _ => {}
        }
    }
}

mod sunsoft_fme7 {
    use crate::apu;

    // https://www.nesdev.org/wiki/Sunsoft_5B_audio
    #[derive(Default)]
    #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
    pub(super) struct Tone {
        pub enabled: bool,
        pub div: apu::DividerCounter,
        pub volume: u8,
        step: u16,
        pub output: f32,
    }

    impl Tone {
        // https://github.com/SourMesen/Mesen2/blob/fabc9a62174f8734a113df6d244f5539ef6b8fcf/Core/NES/Mappers/Audio/Sunsoft5bAudio.h#L99
        pub const TABLE: [f32; 16] = {
            let mut lut = [0.0; 0x10];

            let mut i = 1;
            let mut out: f32 = 1.0;
            while i < 16 {
                out *= 1.1885022274370184377301224648922;
                out *= 1.1885022274370184377301224648922;
                lut[i] = out;
                i += 1;
            }

            lut
        };

        pub fn step(&mut self) {
            // Unlike the 2A03 and VRC6 pulse channels' frequency formulas, the formula for 5B does not add 1 to the period.
            // A period value of 0 appears to produce the same result as a period value of 1, for tone[1], noise and envelope[2].

            // Correct behaviour can be implemented as a counter that counts up on every 16th clock cycle until it is equal to or greater than the period register,
            // at which point the output flips and the counter resets to 0.
            if self.div.step() {
                self.step = (self.step + 1) & 0xf;
                self.update_output();
            }
        }

        pub fn update_output(&mut self) {
            self.output = if self.enabled && self.step < 0x8 {
                Self::TABLE[self.volume as usize]
            } else {
                0.0
            }
        }
    }
}

// https://www.nesdev.org/wiki/Sunsoft_FME-7
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct SunsoftFME7 {
    cpu_command: u8,

    irq_enabled: bool,
    irq_count_enabled: bool,
    irq_count: u16,

    audio_command: u8,
    audio_enabled: bool,

    ta: sunsoft_fme7::Tone,
    tb: sunsoft_fme7::Tone,
    tc: sunsoft_fme7::Tone,
}
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for SunsoftFME7 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new(0x6000, mem.header.prg_size, 40 * 1024, 5);
        mem.banks.prg.fix_last_page();
        mem.banks.chr = Banking::new_chr(&mem.header, 8);

        Box::new(Self {
            ..Default::default()
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        match addr & 0xe000 {
            // 0x8000..=0x9fff
            0x8000 => self.cpu_command = val as u8 & 0xf,
            // 0xa000..=0xbfff
            0xa000 => match self.cpu_command {
                0..=7 => mem.banks.chr.set_page(self.cpu_command, val as u16),
                8 => {
                    let handler = if val & 0x40 > 0 {
                        CpuHandler::Wram
                    } else {
                        CpuHandler::Prg
                    };
                    mem.set_wram_handlers(handler);

                    mem.banks.wram.set_page(0, val as u16 & 0x3f);
                    mem.banks.prg.set_page(0, val as u16 & 0x3f);

                    mem.wram_enable(val & 0x80 > 0);
                }
                0x9..=0xb => mem
                    .banks
                    .prg
                    .set_page(self.cpu_command - 9 + 1, val as u16 & 0x3f),
                0xc => {
                    let mirroring = match val & 0b11 {
                        0 => Mirroring::Vertical,
                        1 => Mirroring::Horizontal,
                        2 => Mirroring::LowTable,
                        _ => Mirroring::HighTable,
                    };
                    mem.banks.vram.mirror(&mirroring);
                }
                0xd => {
                    self.irq_enabled = val & 1 > 0;
                    self.irq_count_enabled = val & 0x80 > 0;
                    mem.irq.remove(IrqFlags::MAPPER);
                }
                0xe => self.irq_count = byte_set_lo(self.irq_count, val),
                0xf => self.irq_count = byte_set_hi(self.irq_count, val),
                _ => {}
            },

            // 0xc000..=0xdfff
            0xc000 => {
                self.audio_command = val & 0x0f;
                self.audio_enabled = val & 0xf0 == 0;
            }
            // 0xe000..=0xffff
            0xe000 => {
                if !self.audio_enabled {
                    return;
                }

                match self.audio_command {
                    0x0 => self.ta.div.period = byte_set_lo(self.ta.div.period, val),
                    0x1 => self.ta.div.period = byte_set_hi(self.ta.div.period, val & 0xf),

                    0x2 => self.tb.div.period = byte_set_lo(self.tb.div.period, val),
                    0x3 => self.tb.div.period = byte_set_hi(self.tb.div.period, val & 0xf),

                    0x4 => self.tc.div.period = byte_set_lo(self.tc.div.period, val),
                    0x5 => self.tc.div.period = byte_set_hi(self.tc.div.period, val & 0xf),

                    0x7 => {
                        self.ta.enabled = val & 0x1 == 0;
                        self.tb.enabled = val & 0x2 == 0;
                        self.tc.enabled = val & 0x4 == 0;
                    }

                    0x8 => self.ta.volume = val & 0xf,
                    0x9 => self.tb.volume = val & 0xf,
                    0xa => self.tc.volume = val & 0xf,

                    // This audio hardware was only used in one game, Gimmick!
                    // Because this game did not use many features of the chip (e.g. noise, envelope), its features are often only partially implemented by emulators.
                    _ => {}
                }

                self.ta.update_output();
                self.tb.update_output();
                self.tc.update_output();
            }
            _ => {}
        }
    }

    fn step(&mut self, mem: &mut Bus, cycles: usize) {
        if self.irq_count_enabled {
            self.irq_count = self.irq_count.wrapping_sub(1);

            if self.irq_count == 0xffff && self.irq_enabled {
                mem.irq.insert(IrqFlags::MAPPER);
            }
        }

        if cycles % 2 == 1 {
            self.ta.step();
            self.tb.step();
            self.tc.step();
        }
    }

    fn sample(&self) -> f32 {
        // It is very loud compared to other audio expansion carts.
        (apu::EXT_MIX * 0.3) * (self.ta.output + self.tb.output + self.tc.output)
    }
}

#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Homebrews29;
#[cfg_attr(feature = "savestates", typetag::serde)]
impl Mapper for Homebrews29 {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg.fix_last_page();
        Box::new(Self)
    }

    fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
        mem.banks.prg.set_page(0, (val >> 2) as u16 & 0x7);
        mem.banks.chr.set_page(0, val as u16 & 0x3);
    }
}
