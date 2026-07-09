use crate::{
    bus::{Banking, Bus, IrqFlags, PpuHandler},
    emu::Mirroring,
    mapper::Mapper,
    utils::{byte_set_hi, byte_set_lo},
};

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
pub struct Namcot {
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
impl Mapper for Namcot {
    fn new(mem: &mut Bus) -> Box<Self> {
        mem.banks.prg = Banking::new_prg(&mem.header, 4);

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
            return mem.cpu_open_bus;
        }

        match addr {
            0x4800..=0x4fff => self.audio.read_data(),
            0x5000..=0x57ff => self.irq_count as u8,
            0x5800..=0x5fff => ((self.irq_enabled as u8) << 7) | (self.irq_count >> 8) as u8,
            _ => mem.cpu_open_bus,
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
                    mem.ppu_handlers_1kb[page as usize] = PpuHandler::Chr;
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
                mem.banks.prg.set_page(0, val as u16);
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
                mem.banks.prg.set_page(1, val as u16);
                self.chr_ram0 = val & 0x40 > 0;
                self.chr_ram1 = val & 0x80 > 0;
            }
            (0xf000..=0xf7ff, _) => {
                mem.banks.prg.set_page(2, val as u16);
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
        // this is very loud, attenuate it a little
        self.audio.output as f32 * (crate::apu::EXT_MIX * 0.5)
    }
}
