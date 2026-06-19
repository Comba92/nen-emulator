use crate::{
    apu,
    bus::{Banking, Bus, CpuHandler, IrqFlags},
    emu::Mirroring,
    mapper::Mapper,
    utils::{byte_set_hi, byte_set_lo},
};

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
            let mut lut = [0.0; 16];

            let mut i = 1;
            let mut out: f64 = 1.0;
            while i < 16 {
                out *= 1.1885022274370184377301224648922;
                out *= 1.1885022274370184377301224648922;
                lut[i] = out as f32;
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
                self.step = (self.step + 1) % 16;
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
pub struct SunsoftFME7 {
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
