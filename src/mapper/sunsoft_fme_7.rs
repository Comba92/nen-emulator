use crate::{bus::Bus, cart::{CartHeader, Mirroring}, mmu::{set_byte_hi, set_byte_lo, MemConfig}};
use super::{Banking, Mapper};

#[derive(serde::Serialize, serde::Deserialize)]
enum Command { Chr(u8), Prg0, Prg1(u8), Nametbl, IrqCtrl, IrqLo, IrqHi }
impl Default for Command {
  fn default() -> Self { Self::Chr(0) }
}

// Mapper 69
// https://www.nesdev.org/wiki/Sunsoft_FME-7
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct SunsoftFME7 {
  command: Command,

  sram_banked: bool,
  sram_enabled: bool,

  irq_enabled: bool,
  irq_counter_enabled: bool,
  irq_requested: Option<()>,
  irq_count: u16,
}

#[typetag::serde]
impl Mapper for SunsoftFME7 {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    banks.prg.set_page_to_last_bank(3);
    banks.chr = Banking::new_chr(header, 8);

    let mapper = Self {
      command: Command::Chr(0),

      ..Default::default()
    };
    Box::new(mapper)
  }

  fn prg_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0x8000..=0x9FFF => {
        let val = val & 0b1111;
        self.command = match val {
          0x8 => Command::Prg0,
          0x9 | 0xA | 0xB => Command::Prg1(val - 0x9),
          0xC => Command::Nametbl,
          0xD => Command::IrqCtrl,
          0xE => Command::IrqLo,
          0xF => Command::IrqHi,
          0x0..=0x7 => Command::Chr(val),
          _ => unreachable!("")
        };
      }
      0xA000..=0xBFFF => {
        match self.command {
          Command::Chr(page) => 
            banks.chr.set_page(page as usize, val as usize),
          Command::Prg0 => {
            self.sram_banked = (val >> 6) & 1 != 0;
            self.sram_enabled = val >> 7 != 0;

            let bank = val as usize & 0b11_1111;
            banks.sram.set_page(0, bank);

            if self.sram_banked {
              banks.mapping.cpu_reads[3]  = Bus::sram_read;
              banks.mapping.cpu_writes[3] = Bus::sram_write;
            } else {
              banks.mapping.cpu_reads[3]  = |bus: &mut Bus, addr: u16| {
                let cart = bus.cart.as_mut();
                cart.prg[cart.mapper.sram_translate(&mut cart.cfg, addr)]
              };
              banks.mapping.cpu_writes[3] = |bus: &mut Bus, addr: u16, val: u8| {
                let cart = bus.cart.as_mut();
                cart.prg[cart.mapper.sram_translate(&mut cart.cfg, addr)] = val;
              };
            }
          }
          Command::Prg1(page) => 
            banks.prg.set_page(page as usize, val as usize & 0b11_1111),
          Command::Nametbl => {
            let mirroring = match val & 0b11 {
              0 => Mirroring::Vertical,
              1 => Mirroring::Horizontal,
              2 => Mirroring::SingleScreenA,
              _ => Mirroring::SingleScreenB
            };
            banks.ciram.update(mirroring);
          }
          Command::IrqCtrl => {
            self.irq_enabled = val & 1 != 0;
            self.irq_counter_enabled = val >> 7 != 0;
            self.irq_requested = None;
          }
          Command::IrqLo => self.irq_count = set_byte_lo(self.irq_count, val),
          Command::IrqHi => self.irq_count = set_byte_hi(self.irq_count, val),
        }
      }
      _ => {}
    }
  }

  // fn map_prg_addr_branching(&mut self, banks: &mut MemConfig, addr: usize) -> PrgTarget {
  //   match addr {
  //     0x4020..=0x5FFF => PrgTarget::Cart,
  //     0x6000..=0x7FFF => {
  //       if self.sram_banked {
  //         PrgTarget::SRam(self.sram_enabled, banks.sram.translate(addr))
  //       } else {
  //         PrgTarget::Prg(banks.sram.translate(addr))
  //       }
  //     }
  //     0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
  //     _ => unreachable!()
  //   }
  // }

  fn notify_cpu_cycle(&mut self) {
    if !self.irq_counter_enabled { return; }

    self.irq_count = self.irq_count.wrapping_sub(1);
    if self.irq_count == 0xFFFF && self.irq_enabled {
      self.irq_requested = Some(());
    }
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }
}