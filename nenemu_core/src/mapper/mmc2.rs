use crate::{
    bus::{Banking, Bus, PpuHandler},
    emu::Mirroring,
    mapper::Mapper,
};

#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
enum Latch {
    FD,
    FE,
}

// https://www.nesdev.org/wiki/MMC2
// https://www.nesdev.org/wiki/MMC4
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct MMC2 {
    fd0: u8,
    fd1: u8,
    fe0: u8,
    fe1: u8,
    latch0: Latch,
    latch1: Latch,
    is_mmc4: bool,
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
        } else {
            unreachable!()
        };

        mem.banks.chr = Banking::new_chr(&mem.header, 2);
        mem.set_chr_handlers(PpuHandler::ChrMMC2);

        Box::new(Self {
            is_mmc4: mem.header.mapper == 10,

            fe0: 0,
            fe1: 0,
            fd0: 0,
            fd1: 0,

            latch0: Latch::FD,
            latch1: Latch::FD,
        })
    }

    fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
        match addr >> 12 {
            0xa => mem.banks.prg.set_page(0, val as u16),
            0xb => self.fd0 = val,
            0xc => self.fe0 = val,
            0xd => self.fd1 = val,
            0xe => self.fe1 = val,
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

    fn notify_ppu_addr(&mut self, mem: &mut Bus, addr: u16, _: usize) {
        match (addr, self.is_mmc4) {
            (0x0fd8, false) | (0x0fd8..=0x0fdf, true) => self.latch0 = Latch::FD,
            (0xfe8, false) | (0x0fe8..=0xfef, true) => self.latch0 = Latch::FE,
            (0x1fd8..=0x1fdf, _) => self.latch1 = Latch::FD,
            (0x1fe8..=0x1fef, _) => self.latch1 = Latch::FE,
            _ => {}
        }

        match self.latch0 {
            Latch::FD => mem.banks.chr.set_page(0, self.fd0 as u16),
            Latch::FE => mem.banks.chr.set_page(0, self.fe0 as u16),
        }

        match self.latch1 {
            Latch::FD => mem.banks.chr.set_page(1, self.fd1 as u16),
            Latch::FE => mem.banks.chr.set_page(1, self.fe1 as u16),
        }
    }
}
