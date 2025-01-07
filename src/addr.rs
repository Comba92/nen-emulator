// trait Addressing<M: Memory> {
//   pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>);
//   pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8;
//   pub fn dummy_read(&self, cpu: &mut Cpu<M>) {}
// }

// #[derive(Default)]
// struct Implicit;
// impl<M: Memory> Addressing<M> for Implicit {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//       cpu.read(cpu.pc + 1);
//     }

//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.pc_fetch()
//     }
// }

// #[derive(Default)]
// struct Accumulator;
// impl<M: Memory> Addressing<M> for Accumulator {
//   pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//       cpu.read(cpu.pc + 1);
//   }
//   pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 { cpu.a }
// }

// #[derive(Default)]
// struct Immediate;
// impl<M: Memory> Addressing<M> for Immediate {
//     pub fn get_operand_addr(&mut self, _cpu: &mut Cpu<M>) {}
//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.pc_fetch()
//     }
// }

// #[derive(Default)]
// struct ZeroPage {
//   address: u8
// }
// impl<M: Memory> Addressing<M> for ZeroPage {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         self.address = cpu.pc_fetch();
//     }
//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.address as u16)
//     }
// }

// #[derive(Default)]
// struct ZeroPageX {
//   address: u8
// }
// impl<M: Memory> Addressing<M> for ZeroPageX {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         self.address = cpu.pc_fetch();
//         cpu.read(self.address as u16);
//     }

//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.address.wrapping_add(cpu.x) as u16)
//     }
// }

// #[derive(Default)]
// struct ZeroPageY {
//   address: u8
// }
// impl<M: Memory> Addressing<M> for ZeroPageY {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         self.address = cpu.pc_fetch();
//         cpu.read(self.address as u16);
//     }

//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.address.wrapping_add(cpu.y) as u16)
//     }
// }

// #[derive(Default)]
// struct Absolute {
//   address: u16
// }
// impl<M: Memory> Addressing<M> for Absolute {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         self.address = cpu.pc_fetch16();
//     }
//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.address)
//     }
// }

// #[derive(Default)]
// struct AbsoluteX {
//   addr_effective: u16,
//   addr_wrong: u16,
//   dummy_readed: bool,
// }
// impl<M: Memory> Addressing<M> for AbsoluteX {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         let addr_base = cpu.pc_fetch16();
//         self.addr_effective = addr_base.wrapping_add(cpu.x as u16);
//         self.addr_wrong = (addr_base & 0xFF00) | (self.addr_effective & 0x00FF);

//         if self.addr_effective != self.addr_wrong {
//           self.dummy_readed = true;
//           cpu.read(self.addr_wrong);
//         }
//     }

//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.addr_effective)
//     }

//     pub fn dummy_read(&self, cpu: &mut Cpu<M>) {
//         if !self.dummy_readed {
//           cpu.read(self.addr_wrong);
//         }
//     }
// }

// #[derive(Default)]
// struct AbsoluteY {
//   addr_effective: u16,
//   addr_wrong: u16,
//   dummy_readed: bool,
// }

// impl<M: Memory> Addressing<M> for AbsoluteY {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         let addr_base = cpu.pc_fetch16();
//         self.addr_effective = addr_base.wrapping_add(cpu.y as u16);
//         self.addr_wrong = (addr_base & 0xFF00) | (self.addr_effective & 0x00FF);

//         if self.addr_effective != self.addr_wrong {
//           self.dummy_readed = true;
//           cpu.read(self.addr_wrong);
//         }
//     }

//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.addr_effective)
//     }

//     pub fn dummy_read(&self, cpu: &mut Cpu<M>) {
//       if !self.dummy_readed {
//         cpu.read(self.addr_wrong);
//       }
//   }
// }

// #[derive(Default)]
// struct Indirect {
//   addr: u16
// }
// impl<M: Memory> Addressing<M> for Indirect {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         let addr_base = cpu.pc_fetch16();
//         self.addr = cpu.wrapping_read16(addr_base);
//     }

//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.addr)
//     }
// }

// #[derive(Default)]
// struct IndirectX {
//   addr: u16,
// }
// impl<M: Memory> Addressing<M> for IndirectX {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         let zero_addr = cpu.pc_fetch();
//         cpu.read(zero_addr as u16);
//         let addr_base = zero_addr.wrapping_add(cpu.x) as u16;
//         self.addr = cpu.wrapping_read16(addr_base);
//     }

//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.addr)
//     }
// }

// #[derive(Default)]
// struct IndirectY {
//   addr_wrong: u16,
//   addr_effective: u16,
//   dummy_readed: bool,
// }
// impl<M: Memory> Addressing<M> for IndirectY {
//     pub fn get_operand_addr(&mut self, cpu: &mut Cpu<M>) {
//         let zero_addr = cpu.pc_fetch();
//         let addr_base = cpu.wrapping_read16(zero_addr as u16);
//         self.addr_effective = addr_base.wrapping_add(cpu.y as u16);
//         self.addr_wrong = (addr_base & 0xFF00) | (self.addr_effective & 0x00FF);

//         if self.addr_effective != self.addr_wrong {
//           self.dummy_readed = true;
//           cpu.read(self.addr_wrong);
//         }
//     }

//     pub fn get_operand_value(&self, cpu: &mut Cpu<M>) -> u8 {
//       cpu.read(self.addr_effective)
//     }

//     pub fn dummy_read(&self, cpu: &mut Cpu<M>) {
//       if !self.dummy_readed {
//         cpu.read(self.addr_wrong);
//       }
//   }
// }

// 0x00 => self.brk(op, Implied::default()),
// 0x01 => self.ora(op, IndirectX::default()),
// 0x02 => self.jam(op, NaN::default()),
// 0x03 => self.slo(op, IndirectX::default()),
// 0x04 => self.nop(op, Zeropage::default()),
// 0x05 => self.ora(op, Zeropage::default()),
// 0x06 => self.asl(op, Zeropage::default()),
// 0x07 => self.slo(op, Zeropage::default()),
// 0x08 => self.php(op, Implied::default()),
// 0x09 => self.ora(op, Immediate::default()),
// 0x0A => self.asl(op, Accumulator::default()),
// 0x0B => self.anc(op, Immediate::default()),
// 0x0C => self.nop(op, Absolute::default()),
// 0x0D => self.ora(op, Absolute::default()),
// 0x0E => self.asl(op, Absolute::default()),
// 0x0F => self.slo(op, Absolute::default()),
// 0x10 => self.bpl(op, Relative::default()),
// 0x11 => self.ora(op, IndirectY::default()),
// 0x12 => self.jam(op, NaN::default()),
// 0x13 => self.slo(op, IndirectY::default()),
// 0x14 => self.nop(op, ZeropageX::default()),
// 0x15 => self.ora(op, ZeropageX::default()),
// 0x16 => self.asl(op, ZeropageX::default()),
// 0x17 => self.slo(op, ZeropageX::default()),
// 0x18 => self.clc(op, Implied::default()),
// 0x19 => self.ora(op, AbsoluteY::default()),
// 0x1A => self.nop(op, Implied::default()),
// 0x1B => self.slo(op, AbsoluteY::default()),
// 0x1C => self.nop(op, AbsoluteX::default()),
// 0x1D => self.ora(op, AbsoluteX::default()),
// 0x1E => self.asl(op, AbsoluteX::default()),
// 0x1F => self.slo(op, AbsoluteX::default()),
// 0x20 => self.jsr(op, Absolute::default()),
// 0x21 => self.and(op, IndirectX::default()),
// 0x22 => self.jam(op, NaN::default()),
// 0x23 => self.rla(op, IndirectX::default()),
// 0x24 => self.bit(op, Zeropage::default()),
// 0x25 => self.and(op, Zeropage::default()),
// 0x26 => self.rol(op, Zeropage::default()),
// 0x27 => self.rla(op, Zeropage::default()),
// 0x28 => self.plp(op, Implied::default()),
// 0x29 => self.and(op, Immediate::default()),
// 0x2A => self.rol(op, Accumulator::default()),
// 0x2B => self.anc(op, Immediate::default()),
// 0x2C => self.bit(op, Absolute::default()),
// 0x2D => self.and(op, Absolute::default()),
// 0x2E => self.rol(op, Absolute::default()),
// 0x2F => self.rla(op, Absolute::default()),
// 0x30 => self.bmi(op, Relative::default()),
// 0x31 => self.and(op, IndirectY::default()),
// 0x32 => self.jam(op, NaN::default()),
// 0x33 => self.rla(op, IndirectY::default()),
// 0x34 => self.nop(op, ZeropageX::default()),
// 0x35 => self.and(op, ZeropageX::default()),
// 0x36 => self.rol(op, ZeropageX::default()),
// 0x37 => self.rla(op, ZeropageX::default()),
// 0x38 => self.sec(op, Implied::default()),
// 0x39 => self.and(op, AbsoluteY::default()),
// 0x3A => self.nop(op, Implied::default()),
// 0x3B => self.rla(op, AbsoluteY::default()),
// 0x3C => self.nop(op, AbsoluteX::default()),
// 0x3D => self.and(op, AbsoluteX::default()),
// 0x3E => self.rol(op, AbsoluteX::default()),
// 0x3F => self.rla(op, AbsoluteX::default()),
// 0x40 => self.rti(op, Implied::default()),
// 0x41 => self.eor(op, IndirectX::default()),
// 0x42 => self.jam(op, NaN::default()),
// 0x43 => self.sre(op, IndirectX::default()),
// 0x44 => self.nop(op, Zeropage::default()),
// 0x45 => self.eor(op, Zeropage::default()),
// 0x46 => self.lsr(op, Zeropage::default()),
// 0x47 => self.sre(op, Zeropage::default()),
// 0x48 => self.pha(op, Implied::default()),
// 0x49 => self.eor(op, Immediate::default()),
// 0x4A => self.lsr(op, Accumulator::default()),
// 0x4B => self.alr(op, Immediate::default()),
// 0x4C => self.jmp(op, Absolute::default()),
// 0x4D => self.eor(op, Absolute::default()),
// 0x4E => self.lsr(op, Absolute::default()),
// 0x4F => self.sre(op, Absolute::default()),
// 0x50 => self.bvc(op, Relative::default()),
// 0x51 => self.eor(op, IndirectY::default()),
// 0x52 => self.jam(op, NaN::default()),
// 0x53 => self.sre(op, IndirectY::default()),
// 0x54 => self.nop(op, ZeropageX::default()),
// 0x55 => self.eor(op, ZeropageX::default()),
// 0x56 => self.lsr(op, ZeropageX::default()),
// 0x57 => self.sre(op, ZeropageX::default()),
// 0x58 => self.cli(op, Implied::default()),
// 0x59 => self.eor(op, AbsoluteY::default()),
// 0x5A => self.nop(op, Implied::default()),
// 0x5B => self.sre(op, AbsoluteY::default()),
// 0x5C => self.nop(op, AbsoluteX::default()),
// 0x5D => self.eor(op, AbsoluteX::default()),
// 0x5E => self.lsr(op, AbsoluteX::default()),
// 0x5F => self.sre(op, AbsoluteX::default()),
// 0x60 => self.rts(op, Implied::default()),
// 0x61 => self.adc(op, IndirectX::default()),
// 0x62 => self.jam(op, NaN::default()),
// 0x63 => self.rra(op, IndirectX::default()),
// 0x64 => self.nop(op, Zeropage::default()),
// 0x65 => self.adc(op, Zeropage::default()),
// 0x66 => self.ror(op, Zeropage::default()),
// 0x67 => self.rra(op, Zeropage::default()),
// 0x68 => self.pla(op, Implied::default()),
// 0x69 => self.adc(op, Immediate::default()),
// 0x6A => self.ror(op, Accumulator::default()),
// 0x6B => self.arr(op, Immediate::default()),
// 0x6C => self.jmp(op, Indirect::default()),
// 0x6D => self.adc(op, Absolute::default()),
// 0x6E => self.ror(op, Absolute::default()),
// 0x6F => self.rra(op, Absolute::default()),
// 0x70 => self.bvs(op, Relative::default()),
// 0x71 => self.adc(op, IndirectY::default()),
// 0x72 => self.jam(op, NaN::default()),
// 0x73 => self.rra(op, IndirectY::default()),
// 0x74 => self.nop(op, ZeropageX::default()),
// 0x75 => self.adc(op, ZeropageX::default()),
// 0x76 => self.ror(op, ZeropageX::default()),
// 0x77 => self.rra(op, ZeropageX::default()),
// 0x78 => self.sei(op, Implied::default()),
// 0x79 => self.adc(op, AbsoluteY::default()),
// 0x7A => self.nop(op, Implied::default()),
// 0x7B => self.rra(op, AbsoluteY::default()),
// 0x7C => self.nop(op, AbsoluteX::default()),
// 0x7D => self.adc(op, AbsoluteX::default()),
// 0x7E => self.ror(op, AbsoluteX::default()),
// 0x7F => self.rra(op, AbsoluteX::default()),
// 0x80 => self.nop(op, Immediate::default()),
// 0x81 => self.sta(op, IndirectX::default()),
// 0x82 => self.nop(op, Immediate::default()),
// 0x83 => self.sax(op, IndirectX::default()),
// 0x84 => self.sty(op, Zeropage::default()),
// 0x85 => self.sta(op, Zeropage::default()),
// 0x86 => self.stx(op, Zeropage::default()),
// 0x87 => self.sax(op, Zeropage::default()),
// 0x88 => self.dey(op, Implied::default()),
// 0x89 => self.nop(op, Immediate::default()),
// 0x8A => self.txa(op, Implied::default()),
// 0x8B => self.ane(op, Immediate::default()),
// 0x8C => self.sty(op, Absolute::default()),
// 0x8D => self.sta(op, Absolute::default()),
// 0x8E => self.stx(op, Absolute::default()),
// 0x8F => self.sax(op, Absolute::default()),
// 0x90 => self.bcc(op, Relative::default()),
// 0x91 => self.sta(op, IndirectY::default()),
// 0x92 => self.jam(op, NaN::default()),
// 0x93 => self.sha(op, IndirectY::default()),
// 0x94 => self.sty(op, ZeropageX::default()),
// 0x95 => self.sta(op, ZeropageX::default()),
// 0x96 => self.stx(op, ZeropageY::default()),
// 0x97 => self.sax(op, ZeropageY::default()),
// 0x98 => self.tya(op, Implied::default()),
// 0x99 => self.sta(op, AbsoluteY::default()),
// 0x9A => self.txs(op, Implied::default()),
// 0x9B => self.tas(op, AbsoluteY::default()),
// 0x9C => self.shy(op, AbsoluteX::default()),
// 0x9D => self.sta(op, AbsoluteX::default()),
// 0x9E => self.shx(op, AbsoluteY::default()),
// 0x9F => self.sha(op, AbsoluteY::default()),
// 0xA0 => self.ldy(op, Immediate::default()),
// 0xA1 => self.lda(op, IndirectX::default()),
// 0xA2 => self.ldx(op, Immediate::default()),
// 0xA3 => self.lax(op, IndirectX::default()),
// 0xA4 => self.ldy(op, Zeropage::default()),
// 0xA5 => self.lda(op, Zeropage::default()),
// 0xA6 => self.ldx(op, Zeropage::default()),
// 0xA7 => self.lax(op, Zeropage::default()),
// 0xA8 => self.tay(op, Implied::default()),
// 0xA9 => self.lda(op, Immediate::default()),
// 0xAA => self.tax(op, Implied::default()),
// 0xAB => self.lxa(op, Immediate::default()),
// 0xAC => self.ldy(op, Absolute::default()),
// 0xAD => self.lda(op, Absolute::default()),
// 0xAE => self.ldx(op, Absolute::default()),
// 0xAF => self.lax(op, Absolute::default()),
// 0xB0 => self.bcs(op, Relative::default()),
// 0xB1 => self.lda(op, IndirectY::default()),
// 0xB2 => self.jam(op, NaN::default()),
// 0xB3 => self.lax(op, IndirectY::default()),
// 0xB4 => self.ldy(op, ZeropageX::default()),
// 0xB5 => self.lda(op, ZeropageX::default()),
// 0xB6 => self.ldx(op, ZeropageY::default()),
// 0xB7 => self.lax(op, ZeropageY::default()),
// 0xB8 => self.clv(op, Implied::default()),
// 0xB9 => self.lda(op, AbsoluteY::default()),
// 0xBA => self.tsx(op, Implied::default()),
// 0xBB => self.las(op, AbsoluteY::default()),
// 0xBC => self.ldy(op, AbsoluteX::default()),
// 0xBD => self.lda(op, AbsoluteX::default()),
// 0xBE => self.ldx(op, AbsoluteY::default()),
// 0xBF => self.lax(op, AbsoluteY::default()),
// 0xC0 => self.cpy(op, Immediate::default()),
// 0xC1 => self.cmp(op, IndirectX::default()),
// 0xC2 => self.nop(op, Immediate::default()),
// 0xC3 => self.dcp(op, IndirectX::default()),
// 0xC4 => self.cpy(op, Zeropage::default()),
// 0xC5 => self.cmp(op, Zeropage::default()),
// 0xC6 => self.dec(op, Zeropage::default()),
// 0xC7 => self.dcp(op, Zeropage::default()),
// 0xC8 => self.iny(op, Implied::default()),
// 0xC9 => self.cmp(op, Immediate::default()),
// 0xCA => self.dex(op, Implied::default()),
// 0xCB => self.sbx(op, Immediate::default()),
// 0xCC => self.cpy(op, Absolute::default()),
// 0xCD => self.cmp(op, Absolute::default()),
// 0xCE => self.dec(op, Absolute::default()),
// 0xCF => self.dcp(op, Absolute::default()),
// 0xD0 => self.bne(op, Relative::default()),
// 0xD1 => self.cmp(op, IndirectY::default()),
// 0xD2 => self.jam(op, NaN::default()),
// 0xD3 => self.dcp(op, IndirectY::default()),
// 0xD4 => self.nop(op, ZeropageX::default()),
// 0xD5 => self.cmp(op, ZeropageX::default()),
// 0xD6 => self.dec(op, ZeropageX::default()),
// 0xD7 => self.dcp(op, ZeropageX::default()),
// 0xD8 => self.cld(op, Implied::default()),
// 0xD9 => self.cmp(op, AbsoluteY::default()),
// 0xDA => self.nop(op, Implied::default()),
// 0xDB => self.dcp(op, AbsoluteY::default()),
// 0xDC => self.nop(op, AbsoluteX::default()),
// 0xDD => self.cmp(op, AbsoluteX::default()),
// 0xDE => self.dec(op, AbsoluteX::default()),
// 0xDF => self.dcp(op, AbsoluteX::default()),
// 0xE0 => self.cpx(op, Immediate::default()),
// 0xE1 => self.sbc(op, IndirectX::default()),
// 0xE2 => self.nop(op, Immediate::default()),
// 0xE3 => self.isc(op, IndirectX::default()),
// 0xE4 => self.cpx(op, Zeropage::default()),
// 0xE5 => self.sbc(op, Zeropage::default()),
// 0xE6 => self.inc(op, Zeropage::default()),
// 0xE7 => self.isc(op, Zeropage::default()),
// 0xE8 => self.inx(op, Implied::default()),
// 0xE9 => self.sbc(op, Immediate::default()),
// 0xEA => self.nop(op, Implied::default()),
// 0xEB => self.usbc(op, Immediate::default()),
// 0xEC => self.cpx(op, Absolute::default()),
// 0xED => self.sbc(op, Absolute::default()),
// 0xEE => self.inc(op, Absolute::default()),
// 0xEF => self.isc(op, Absolute::default()),
// 0xF0 => self.beq(op, Relative::default()),
// 0xF1 => self.sbc(op, IndirectY::default()),
// 0xF2 => self.jam(op, NaN::default()),
// 0xF3 => self.isc(op, IndirectY::default()),
// 0xF4 => self.nop(op, ZeropageX::default()),
// 0xF5 => self.sbc(op, ZeropageX::default()),
// 0xF6 => self.inc(op, ZeropageX::default()),
// 0xF7 => self.isc(op, ZeropageX::default()),
// 0xF8 => self.sed(op, Implied::default()),
// 0xF9 => self.sbc(op, AbsoluteY::default()),
// 0xFA => self.nop(op, Implied::default()),
// 0xFB => self.isc(op, AbsoluteY::default()),
// 0xFC => self.nop(op, AbsoluteX::default()),
// 0xFD => self.sbc(op, AbsoluteX::default()),
// 0xFE => self.inc(op, AbsoluteX::default()),
// 0xFF => self.isc(op, AbsoluteX::default()),