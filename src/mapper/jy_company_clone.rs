#[derive(serde::Serialize, serde::Deserialize)]
pub struct INesMapper091 {
  submapper: u8,
  irq_latch: u16,
  irq_count: u16,
  irq_requested: Option<()>,
}

#[typetag::serde]
impl Mapper for INesMapper091 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    banks.prg.set(2, banks.prg.banks_count-2);
    banks.prg.set(3, banks.prg.banks_count-1);

    banks.chr = Banking::new_chr(header, 4);

    Box::new(Self {
      submapper: header.submapper,
      irq_latch: 0,
      irq_count: 0,
      irq_requested: None,
    })
  }

  fn write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    let mask = if self.submapper == 1 { 0xF007 } else { 0xF003 }; 
    
    match addr & mask {
      0x6000..=0x6003 => {
        let page = (addr - 0x6000);
        banks.chr.set(page, val as usize);
      }
      0x6004 => banks.vram.update(Mirroring::Horizontal),
      0x6005 => banks.vram.update(Mirroring::Vertical),

      0x6006 => self.irq_latch = (self.irq_latch & 0xF0) | (val as u16),
      0x6007 => self.irq_latch = (self.irq_latch & 0x0F) | ((val as u16) << 8),

      0x7000 => banks.prg.set(0, val as usize),
      0x7001 => banks.prg.set(1, val as usize),

      0x7006 => self.irq_requested = None,
      0x7007 => self.irq_count = 0,

      0x8000..=0x9FFF => {

      }
      _ => {}
    }
  }

  fn map_prg_addr(&self, banks: &mut CartBanking, addr:usize) -> PrgTarget {
    match addr {
      0x6000..=0x7FFF => PrgTarget::Prg(addr),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.addr(addr)),
      _ => unreachable!()
    }
  }

  poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }
}