use crate::{
    bus::{Banking, Bus, CpuHandler, IrqFlags, PpuHandler},
    emu::Mirroring,
    mapper::Mapper,
};

// https://www.nesdev.org/wiki/MMC3
// https://www.nesdev.org/wiki/MMC6
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct MMC3 {
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

        mem.cpu_handlers_8kb[1] = CpuHandler::PpuMMC3;

        let chr_handler = if mem.header.has_chr_ram {
            PpuHandler::ChrRamMMC3
        } else {
            PpuHandler::ChrRamMMC3
        };
        mem.set_chr_handlers(chr_handler);

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
                    (6, _) => mem.banks.prg.set_page(self.prg_swapped, val),
                    (7, _) => mem.banks.prg.set_page(1, val),
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

    fn ppu_bus_callback(&mut self, mem: &mut Bus, addr: u16, cycles: usize) {
        let rising_edge = if addr & 0x1000 > 0 {
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
