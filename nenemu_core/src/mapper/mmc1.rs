use crate::{
    bus::{Banking, Bus},
    emu::Mirroring,
    mapper::Mapper,
};

#[derive(Default, Debug)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
enum WramKind {
    Bank32,
    Bank16,
    #[default]
    Bank8,
}

// Needs NES2.0 / db support for WRAM (NEW FINDING: only SOROM games have 2 different kind of RAM))

#[derive(Default, Debug)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct MMC1 {
    shift_reg: u8,
    shift_count: u8,

    prg_mode: u8,
    prg_bank: u8,
    prg_hi_bank: u8,

    // 512kb of prg
    has_big_prg: bool,
    first_bank: u16,
    last_bank: u16,
    wram_kind: WramKind,

    chr_mode: bool,
    chr_bank0: u8,
    chr_bank1: u8,

    write_delay: u8,
}
impl MMC1 {
    fn update_all_banks(&mut self, mem: &mut Bus, val: u8) {
        if self.has_big_prg {
            self.prg_hi_bank = val & 0x10;

            if self.prg_hi_bank > 0 {
                self.first_bank = mem.banks.prg.banks_count / 2;
                // last bank is the real last
                self.last_bank = mem.banks.prg.banks_count - 1;
            } else {
                self.first_bank = 0;
                // last bank is the mid one
                self.last_bank = mem.banks.prg.banks_count / 2 - 1;
            }
        }

        let wram = &mut mem.banks.wram;
        match self.wram_kind {
            WramKind::Bank16 => wram.set_page(0, (val >> 3) as u16),
            WramKind::Bank32 => wram.set_page(0, (val >> 2) as u16),
            _ => {}
        }

        self.update_prg_banks(mem);
        self.update_chr_banks(mem);
    }

    fn update_prg_banks(&mut self, mem: &mut Bus) {
        let bank = (self.prg_hi_bank | self.prg_bank) as u16;
        match self.prg_mode {
            2 => {
                // 2: fix first bank at $8000 and switch 16 KB bank at $C000
                mem.banks.prg.set_page(0, self.first_bank);
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
            mem.banks.chr.set_page(0, self.chr_bank0 as u16);
            mem.banks.chr.set_page(1, self.chr_bank1 as u16);
        } else {
            mem.banks.chr.set_pages_aligned2(0, self.chr_bank0 as u16);
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
            WramKind::Bank32
        } else if mem.header.wram_size >= 16 * 1024 {
            WramKind::Bank16
        } else {
            WramKind::Bank8
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

                self.prg_mode = (val >> 2) & 0x3;
                self.update_prg_banks(mem);

                self.chr_mode = val & 0x10 > 0;
                self.update_chr_banks(mem);
            }
            0xa000..=0xbfff => {
                // 0xa000 => {
                self.chr_bank0 = val;
                self.update_all_banks(mem, self.chr_bank0);
            }
            // 0xc000..=0xdfff => {
            0xc000 => {
                self.chr_bank1 = val;
                if self.chr_mode {
                    self.update_all_banks(mem, self.chr_bank1);
                }
            }
            // 0xe000..=0xffff => {
            0xe000 => {
                self.prg_bank = val;
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
