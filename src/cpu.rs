use std::ops::{Shl, Shr};

use crate::emu::{self, Emu};

enum AddressingMode {
  Implied,
  Accumulator,
  Immediate,
  ZeroPage,
  ZeroPageX,
  ZeroPageY,
  Relative,
  Absolute,
  AbsoluteX,
  AbsoluteY,
  Indirect,
  IndirectX,
  IndirectY,
}

// #[allow(non_upper_case_globals)]
// mod flags {
  //   pub const Carry:      u8   = 0b1;
  //   pub const Zero:       u8 = 0b10;
  //   pub const IrqDisable: u8 = 0b100;
  //   pub const Decimal:    u8 = 0b1000;
  //   pub const Brk:        u8 = 0b1_0000;
  //   pub const Unused:     u8 = 0b10_0000;
  //   pub const Overflow:   u8 = 0b100_0000;
  //   pub const Negative:   u8 = 0b1000_0000;
  // }
  

bitflags::bitflags! {
  #[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
  pub struct Status: u8 {
    const Carry = 1 << 0;
    const Zero = 1 << 1;
    const IrqDisable = 1 << 2;
    const Decimal = 1 << 3;
    const Brk = 1 << 4;
    const Unused = 1 << 5;
    const Overflow = 1 << 6;
    const Negative = 1 << 7;
  }
}

const STACK_START: u16 = 0x0100;
pub const NMI_VECTOR: u16 = 0xfffa;
pub const RST_VECTOR: u16 = 0xfffc;
pub const IRQ_VECTOR: u16 = 0xfffe;

#[derive(Default, Debug)]
pub struct Cpu6502 {
  pub a:  u8,
  pub x:  u8,
  pub y:  u8,
  pub p:  Status,
  pub sp: u8,
  pub pc: u16,

  op_val: Option<u8>,
  op_addr: u16,

  pub cycles: usize,
}

impl Cpu6502 {
  pub fn new() -> Self {
    Self {
      sp: 0xfd,
      p: Status::Unused | Status::IrqDisable,
      ..Default::default()
    }
  }
}

impl Emu {
  #[cfg(not(feature = "ram64kb"))]
  fn cpu_read8(&mut self, addr: u16) -> u8 {
    let res = self.cpu_dispatch_read(addr);
    // println!("Reading {addr:4x}, got {res:2x}");
    res
  }

  #[cfg(not(feature = "ram64kb"))]
  fn cpu_write8(&mut self, addr: u16, val: u8) {
    // println!("Writing {addr:4x}, got {val:2x}");
    self.cpu_dispatch_write(addr, val);
  }

  #[cfg(feature = "ram64kb")]
  fn cpu_read8(&mut self, addr: u16) -> u8 {
    self.ram[addr as usize]
  }

  #[cfg(feature = "ram64kb")]
  fn cpu_write8(&mut self, addr: u16, val: u8) {
    self.ram[addr as usize] = val;
  }

  pub fn cpu_read16(&mut self, addr: u16) -> u16 {
    let lo = self.cpu_read8(addr);
    let hi = self.cpu_read8(addr.wrapping_add(1));
    u16::from_le_bytes([lo, hi]) 
  }

  fn wrapping_cpu_read16(&mut self, addr: u16) -> u16 {
    if addr & 0x00ff == 0x0ff {
      let page = addr & 0xff00;
      let lo = self.cpu_read8(page | 0xff);
      let hi = self.cpu_read8(page | 0x00);
      u16::from_le_bytes([lo, hi])
    } else {
      self.cpu_read16(addr)
    }
  }

  fn pc_fetch8(&mut self) -> u8 {
    let val = self.cpu_read8(self.cpu.pc);
    self.cpu.pc = self.cpu.pc.wrapping_add(1);
    val
  }

  fn pc_fetch16(&mut self) -> u16 {
    let val = self.cpu_read16(self.cpu.pc);
    self.cpu.pc = self.cpu.pc.wrapping_add(2);
    val
  }

  pub fn cpu_tick(&mut self) {
    self.cpu.cycles += 1;
  }

  pub fn cpu_step(&mut self) {
    self.poll_interrupts();
    
    let opcode = self.pc_fetch8();
    // println!("Opcode: ${opcode:2x}");

    self.fetch_operand(opcode);
    self.decode_n_exec(opcode);

    // TODO: temporary solution
    self.cpu.cycles += CYCLES_TABLE[opcode as usize];
  }

  fn poll_interrupts(&mut self) {
    // https://www.nesdev.org/wiki/CPU_interrupts#IRQ_and_NMI_tick-by-tick_execution
    if self.events.contains(emu::Events::NMI) {
      self.events.remove(emu::Events::NMI);
      self.handle_interrupt(NMI_VECTOR);
    } else if self.events.contains(emu::Events::IRQ) 
      && !self.cpu.p.contains(Status::IrqDisable)
    {
      self.events.remove(emu::Events::IRQ);
      self.handle_interrupt(IRQ_VECTOR);
    }
  }

  fn handle_interrupt(&mut self, int_vector: u16) {
    self.cpu_tick();
    self.cpu_tick();

    self.stack_push16(self.cpu.pc);
    self.stack_push8(self.cpu.p.bits());
    self.cpu.p.insert(Status::IrqDisable);
    self.cpu.pc = self.cpu_read16(int_vector);
  }

  fn fetch_zeropage_op(&mut self, offset: u8) {
    let zero_addr = self.pc_fetch8();
    // self.cpu_tick();
    self.cpu.op_addr = zero_addr.wrapping_add(offset) as u16;
  }


  const RW: &[u8] = &[0x1e, 0xde, 0xfe, 0x5e, 0x3e, 0x7e, 0x9d, 0x99];

  fn fetch_absolute_op(&mut self, offset: u8, opcode: u8) {
    let addr_base = self.pc_fetch16();
    let addr_effective = addr_base.wrapping_add(offset as u16);

    // page crossing check
    if addr_effective & 0xff00 != addr_base & 0xff00 && !Self::RW.contains(&opcode) {
      self.cpu_tick();
    }

    self.cpu.op_addr = addr_effective;
  }

  fn fetch_operand(&mut self, opcode: u8) {
    let mode = &MODES_TABLE[opcode as usize];
    self.cpu.op_val = None;
    
    match mode {
      // Implied => { self.cpu_tick(); },
      Implied => {},
      Accumulator => {
        // self.cpu_tick();
        self.cpu.op_val = Some(self.cpu.a);
      }
      Immediate | Relative => self.cpu.op_val = Some(self.pc_fetch8()),
      ZeroPage => self.cpu.op_addr = self.pc_fetch8() as u16,
      ZeroPageX => self.fetch_zeropage_op(self.cpu.x),
      ZeroPageY => self.fetch_zeropage_op(self.cpu.y),
      Absolute => self.cpu.op_addr = self.pc_fetch16(),
      AbsoluteX => self.fetch_absolute_op(self.cpu.x, opcode),
      AbsoluteY => self.fetch_absolute_op(self.cpu.y, opcode),
      Indirect => {
        let addr = self.pc_fetch16();
        self.cpu.op_addr = self.wrapping_cpu_read16(addr);
      }
      IndirectX => {
        // important to keep it as u8
        let zero_addr = self.pc_fetch8();
        let addr = zero_addr.wrapping_add(self.cpu.x) as u16;
        self.cpu.op_addr = self.wrapping_cpu_read16(addr);
      },
      IndirectY => {
        let zero_addr = self.pc_fetch8() as u16;
        let base_addr = self.wrapping_cpu_read16(zero_addr);
        self.cpu.op_addr = base_addr.wrapping_add(self.cpu.y as u16);

        // page crossing check
        if base_addr & 0xff0 != self.cpu.op_addr & 0xff00 && opcode != 0x91 {
          self.cpu_tick();
        }
      }
    }
  }

  fn set_zn(&mut self, res: u8) {
    // self.cpu.p = bit_change(self.cpu.p, flags::Zero, res == 0);
    // self.cpu.p = bit_change(self.cpu.p, flags::Negative, bit_get(res, 7));
    self.cpu.p.set(Status::Zero, res == 0);
    self.cpu.p.set(Status::Negative, res & 0x80 != 0);
  }

  fn get_op_val(&mut self) -> u8 {
    match self.cpu.op_val {
      Some(val) => val,
      None => self.cpu_read8(self.cpu.op_addr)
    }
  }

  fn set_op_res(&mut self, res: u8) {
    match self.cpu.op_val {
      Some(_) => self.cpu.a = res,
      None => self.cpu_write8(self.cpu.op_addr, res),
    }
  }

  fn stack_curr(&self) -> u16 {
    STACK_START + self.cpu.sp as u16
  }

  fn stack_push8(&mut self, val: u8) {
    self.cpu_write8(self.stack_curr(), val);
    self.cpu.sp = self.cpu.sp.wrapping_sub(1);
  }
  fn stack_pop8(&mut self) -> u8 {
    self.cpu.sp = self.cpu.sp.wrapping_add(1);
    self.cpu_read8(self.stack_curr())
  }
  fn stack_push16(&mut self, val: u16) {
    let [lo, hi] = val.to_le_bytes();
    self.stack_push8(hi);
    self.stack_push8(lo);
  }
  fn stack_pop16(&mut self) -> u16 {
    let lo = self.stack_pop8();
    let hi = self.stack_pop8();
    u16::from_le_bytes([lo, hi])
  }

  fn lda(&mut self) {
    let res = self.get_op_val();
    self.set_zn(res);
    self.cpu.a = res;
  }
  fn ldx(&mut self) {
    let res = self.get_op_val();
    self.set_zn(res);
    self.cpu.x = res;
  }
  fn ldy(&mut self) {
    let res = self.get_op_val();
    self.set_zn(res);
    self.cpu.y = res;
  }

  fn sta(&mut self) {
    self.cpu_write8(self.cpu.op_addr, self.cpu.a);
  }
  fn stx(&mut self) {
    self.cpu_write8(self.cpu.op_addr, self.cpu.x);
  }
  fn sty(&mut self) {
    self.cpu_write8(self.cpu.op_addr, self.cpu.y);
  }

  fn tax(&mut self) {
    self.set_zn(self.cpu.a);
    self.cpu.x = self.cpu.a;
  }
  fn tay(&mut self) {
    self.set_zn(self.cpu.a);
    self.cpu.y = self.cpu.a;
  }
  fn txa(&mut self) {
    self.set_zn(self.cpu.x);
    self.cpu.a = self.cpu.x;
  }
  fn tya(&mut self) {
    self.set_zn(self.cpu.y);
    self.cpu.a = self.cpu.y;
  }

  fn tsx(&mut self) {
    self.set_zn(self.cpu.sp);
    self.cpu.x = self.cpu.sp;
  }
  fn txs(&mut self) {
    self.cpu.sp = self.cpu.x;
  }
  fn pha(&mut self) {
    self.stack_push8(self.cpu.a);
  }
  fn php(&mut self) {
    // self.stack_push8(bit_set(self.cpu.p, flags::Brk));
    self.stack_push8((self.cpu.p | Status::Brk).bits());
  }
  fn pla(&mut self) {
    let res = self.stack_pop8();
    self.set_zn(res);
    self.cpu.a = res;
  }
  fn plp(&mut self) {
    // https://www.nesdev.org/wiki/Instruction_reference#PLP
    // TODO: The effect of changing IrqDisable flag is delayed 1 instruction. 
    let res = self.stack_pop8();
    // self.cpu.p = bit_clear(res, flags::Brk);
    self.cpu.p = (Status::from_bits_retain(res) | Status::Unused) - Status::Brk;
  }

  fn and(&mut self) {
    let res = self.cpu.a & self.get_op_val();
    self.set_zn(res);
    self.cpu.a = res;
  }
  fn eor(&mut self) {
    let res = self.cpu.a ^ self.get_op_val();
    self.set_zn(res);
    self.cpu.a = res;
  }
  fn ora(&mut self) {
    let res = self.cpu.a | self.get_op_val();
    self.set_zn(res);
    self.cpu.a = res;
  }
  fn bit(&mut self) {
    let val = self.get_op_val();
    let res = self.cpu.a & val;
    // self.cpu.p = bit_change(self.cpu.p, flags::Zero, res == 0);
    // self.cpu.p = bit_change(self.cpu.p, flags::Overflow, bit_get(val, 6));
    // self.cpu.p = bit_change(self.cpu.p, flags::Negative, bit_get(val, 7));
    self.cpu.p.set(Status::Zero, res == 0);
    self.cpu.p.set(Status::Overflow, val & 0x40 != 0);
    self.cpu.p.set(Status::Negative, val & 0x80 != 0);
  }

  fn addition(&mut self, val: u8) {
    let res = self.cpu.a as u16 
      + val as u16
      // + bit_get(self.cpu.p, flags::Carry) as u16;
      + self.cpu.p.contains(Status::Carry) as u16;

    // self.cpu.p = bit_change(self.cpu.p, flags::Carry, res > u8::MAX as u16);
    self.cpu.p.set(Status::Carry, res > u8::MAX as u16);
    self.set_zn(res as u8);

    // https://www.righto.com/2012/12/the-6502-overflow-flag-explained.html
    let overflow = (self.cpu.a ^ res as u8) & (val ^ res as u8) & 0x80 != 0;  
    // self.cpu.p = bit_change(self.cpu.p, flags::Overflow, overflow);
    self.cpu.p.set(Status::Overflow, overflow);
    
    self.cpu.a = res as u8;
  }

  fn adc(&mut self) {
    let val = self.get_op_val();
    self.addition(val);
  }
  fn sbc(&mut self) {
    let val = self.get_op_val();
    // SBC simply takes the ones complement of the second value and then performs an ADC.
    self.addition(!val);
  }

  fn compare(&mut self, a: u8) {
    let b = self.get_op_val();
    let res = a.wrapping_sub(b);

    // self.cpu.p = bit_change(self.cpu.p, flags::Carry, a >= b);
    self.cpu.p.set(Status::Carry, a >= b);
    self.set_zn(res);
  }

  fn cmp(&mut self) {
    self.compare(self.cpu.a);
  }
  fn cpx(&mut self) {
    self.compare(self.cpu.x);
  }
  fn cpy(&mut self) {
    self.compare(self.cpu.y);
  }

  fn inc(&mut self) {
    let res = self.get_op_val().wrapping_add(1);
    self.cpu_write8(self.cpu.op_addr, res);
    self.set_zn(res);
  }
  fn inx(&mut self) {
    let res = self.cpu.x.wrapping_add(1);
    self.cpu.x = res;
    self.set_zn(res);
  }
  fn iny(&mut self) {
    let res = self.cpu.y.wrapping_add(1);
    self.cpu.y = res;
    self.set_zn(res);
  }

  fn dec(&mut self) {
    let res = self.get_op_val().wrapping_sub(1);
    self.cpu_write8(self.cpu.op_addr, res);
    self.set_zn(res);
  }
  fn dex(&mut self) {
    let res = self.cpu.x.wrapping_sub(1);
    self.cpu.x = res;
    self.set_zn(res);
  }
  fn dey(&mut self) {
    let res = self.cpu.y.wrapping_sub(1);
    self.cpu.y = res;
    self.set_zn(res);
  }

  fn shift<F: FnOnce(u8) -> u8>(&mut self, op: F, carry_bit: u8) {
    let val = self.get_op_val();
    let res = op(val);
    // self.cpu.p = bit_change(self.cpu.p, flags::Carry, bit_get(val, carry_bit));
    self.cpu.p.set(Status::Carry, val.shr(carry_bit) & 1 == 1);
    self.set_zn(res);
    self.set_op_res(res);
  }

  fn asl(&mut self) {
    self.shift( |x| x.shl(1), 7);
  }
  fn lsr(&mut self) {
    self.shift( |x| x.shr(1), 0);
  }
  fn rol(&mut self) {
    let carry = self.cpu.p.contains(Status::Carry) as u8;
    self.shift( |x| x.shl(1) | carry, 7);
  }
  fn ror(&mut self) {
    let carry = self.cpu.p.contains(Status::Carry) as u8;
    self.shift( |x| x.shr(1) | (carry << 7), 0);
  }

  fn jmp(&mut self) {
    self.cpu.pc = self.cpu.op_addr;
  }
  fn jsr(&mut self) {
    self.stack_push16(self.cpu.pc.wrapping_sub(1));
    self.jmp();
  }
  fn rts(&mut self) {
    self.cpu.pc = self.stack_pop16().wrapping_add(1);
  }

  fn branch(&mut self, cond: bool) {
    if cond {
      // if branch is taken, costs 1 cycle more
      self.cpu_tick();

      let res = self.cpu.pc
        .wrapping_add_signed((self.get_op_val() as i8) as i16);

      if res & 0xff00 != self.cpu.pc & 0xff00 {
        self.cpu_tick();
      }

      self.cpu.pc = res;
    }
  }
  fn bcc(&mut self) {
    // self.branch(!bit_get(self.cpu.p, flags::Carry));
    self.branch(!self.cpu.p.contains(Status::Carry));
  }
  fn bcs(&mut self) {
    // self.branch(bit_get(self.cpu.p, flags::Carry));
    self.branch(self.cpu.p.contains(Status::Carry));
  }
  fn beq(&mut self) {
    // self.branch(bit_get(self.cpu.p, flags::Zero));
    self.branch(self.cpu.p.contains(Status::Zero));
  }
  fn bmi(&mut self) {
    // self.branch(bit_get(self.cpu.p, flags::Negative));
    self.branch(self.cpu.p.contains(Status::Negative));
  }
  fn bne(&mut self) {
    // self.branch(!bit_get(self.cpu.p, flags::Zero));
    self.branch(!self.cpu.p.contains(Status::Zero));
  }
  fn bpl(&mut self) {
    // self.branch(!bit_get(self.cpu.p, flags::Negative));
    self.branch(!self.cpu.p.contains(Status::Negative));
  }
  fn bvc(&mut self) {
    // self.branch(!bit_get(self.cpu.p, flags::Overflow));
    self.branch(!self.cpu.p.contains(Status::Overflow));
  }
  fn bvs(&mut self) {
    // self.branch(bit_get(self.cpu.p, flags::Overflow));
    self.branch(self.cpu.p.contains(Status::Overflow));
  }

  fn clc(&mut self) {
    // self.cpu.p = bit_clear(self.cpu.p, flags::Carry);
    self.cpu.p.remove(Status::Carry);
  }
  fn cld(&mut self) {
    // self.cpu.p = bit_clear(self.cpu.p, flags::Decimal);
    self.cpu.p.remove(Status::Decimal);
  }
  fn cli(&mut self) {
    // self.cpu.p = bit_clear(self.cpu.p, flags::IrqDisable);

    // https://www.nesdev.org/wiki/Instruction_reference#CLI
    // TODO: The effect of changing this flag is delayed 1 instruction. 
    self.cpu.p.remove(Status::IrqDisable);
  }
  fn clv(&mut self) {
    // self.cpu.p = bit_clear(self.cpu.p, flags::Overflow);
    self.cpu.p.remove(Status::Overflow);
  }
  fn sec(&mut self) {
    // self.cpu.p = bit_set(self.cpu.p, flags::Carry);
    self.cpu.p.insert(Status::Carry);
  }
  fn sed(&mut self) {
    // self.cpu.p = bit_set(self.cpu.p, flags::Decimal);
    self.cpu.p.insert(Status::Decimal);
  }
  fn sei(&mut self) {
    // self.cpu.p = bit_set(self.cpu.p, flags::IrqDisable);

    // https://www.nesdev.org/wiki/Instruction_reference#SEI
    // TODO: The effect of changing this flag is delayed 1 instruction. 
    self.cpu.p.insert(Status::IrqDisable);
  }

  fn brk(&mut self) {
    self.stack_push16(self.cpu.pc.wrapping_add(1));
    self.php();
    // self.cpu.p = bit_set(self.cpu.p, flags::IrqDisable);
    self.cpu.p.insert(Status::IrqDisable);
    self.cpu.pc = self.cpu_read16(IRQ_VECTOR);
  }
  fn rti(&mut self) {
    self.plp();
    self.cpu.pc = self.stack_pop16();
  }


  fn nop(&self) {}

  fn lax(&mut self) {
    self.lda();
    self.ldx();
  }

  fn sax(&mut self) {
    let res = self.cpu.a & self.cpu.x;
    self.cpu_write8(self.cpu.op_addr, res);
  }

  fn dcp(&mut self) {
    self.dec();
    self.cmp();
  }

  fn isc(&mut self) {
    self.inc();
    self.sbc();
  }

  fn slo(&mut self) {
    self.asl();
    self.ora();
  }

  fn rla(&mut self) {
    self.rol();
    self.and();
  }

  fn sre(&mut self) {
    self.lsr();
    self.eor();
  }

  fn rra(&mut self) {
    self.ror();
    self.adc();
  }

  fn alr(&mut self) {
    self.and();
    self.lsr();
  }

  fn arr(&mut self) {
    self.and();
    self.ror();
  }
}

use AddressingMode::*;
const MODES_TABLE: [AddressingMode; 256] = [
  Implied,
  IndirectX,
  Implied,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Accumulator,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Absolute,
  IndirectX,
  Implied,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Accumulator,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Implied,
  IndirectX,
  Implied,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Accumulator,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Implied,
  IndirectX,
  Implied,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Accumulator,
  Immediate,
  Indirect,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Immediate,
  IndirectX,
  Immediate,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Implied,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageY,
  ZeroPageY,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteY,
  AbsoluteY,
  Immediate,
  IndirectX,
  Immediate,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Implied,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageY,
  ZeroPageY,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteY,
  AbsoluteY,
  Immediate,
  IndirectX,
  Immediate,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Implied,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  Immediate,
  IndirectX,
  Immediate,
  IndirectX,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  ZeroPage,
  Implied,
  Immediate,
  Implied,
  Immediate,
  Absolute,
  Absolute,
  Absolute,
  Absolute,
  Relative,
  IndirectY,
  Implied,
  IndirectY,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  ZeroPageX,
  Implied,
  AbsoluteY,
  Implied,
  AbsoluteY,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
  AbsoluteX,
];

const CYCLES_TABLE: &[usize] = &[
  7, 6, 2, 8, 3, 3, 5, 5, 3, 2, 2, 2, 4, 4, 6, 6, 2, 5, 2, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, 6, 6, 2, 8, 3, 3, 5, 5, 4, 2, 2, 2, 4, 4, 6, 6, 2, 5, 2, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, 6, 6, 2, 8, 3, 3, 5, 5, 3, 2, 2, 2, 3, 4, 6, 6, 2, 5, 2, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, 6, 6, 2, 8, 3, 3, 5, 5, 4, 2, 2, 2, 5, 4, 6, 6, 2, 5, 2, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, 2, 6, 2, 6, 3, 3, 3, 3, 2, 2, 2, 2, 4, 4, 4, 4, 2, 6, 2, 6, 4, 4, 4, 4, 2, 5, 2, 5, 5, 5, 5, 5, 2, 6, 2, 6, 3, 3, 3, 3, 2, 2, 2, 2, 4, 4, 4, 4, 2, 5, 2, 5, 4, 4, 4, 4, 2, 4, 2, 4, 4, 4, 4, 4, 2, 6, 2, 8, 3, 3, 5, 5, 2, 2, 2, 2, 4, 4, 6, 6, 2, 5, 2, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7, 2, 6, 2, 8, 3, 3, 5, 5, 2, 2, 2, 2, 4, 4, 6, 6, 2, 5, 2, 8, 4, 4, 6, 6, 2, 4, 2, 7, 4, 4, 7, 7
];

impl Emu {
  fn decode_n_exec(&mut self, opcode: u8) {
    match opcode {
      0x00 => self.brk(),
      0x01 => self.ora(),
      0x05 => self.ora(),
      0x06 => self.asl(),
      0x08 => self.php(),
      0x09 => self.ora(),
      0x0a => self.asl(),
      0x0d => self.ora(),
      0x0e => self.asl(),
      0x10 => self.bpl(),
      0x11 => self.ora(),
      0x15 => self.ora(),
      0x16 => self.asl(),
      0x18 => self.clc(),
      0x19 => self.ora(),
      0x1d => self.ora(),
      0x1e => self.asl(),
      0x20 => self.jsr(),
      0x21 => self.and(),
      0x24 => self.bit(),
      0x25 => self.and(),
      0x26 => self.rol(),
      0x28 => self.plp(),
      0x29 => self.and(),
      0x2a => self.rol(),
      0x2c => self.bit(),
      0x2d => self.and(),
      0x2e => self.rol(),
      0x30 => self.bmi(),
      0x31 => self.and(),
      0x35 => self.and(),
      0x36 => self.rol(),
      0x38 => self.sec(),
      0x39 => self.and(),
      0x3d => self.and(),
      0x3e => self.rol(),
      0x40 => self.rti(),
      0x41 => self.eor(),
      0x45 => self.eor(),
      0x46 => self.lsr(),
      0x48 => self.pha(),
      0x49 => self.eor(),
      0x4a => self.lsr(),
      0x4c => self.jmp(),
      0x4d => self.eor(),
      0x4e => self.lsr(),
      0x50 => self.bvc(),
      0x51 => self.eor(),
      0x55 => self.eor(),
      0x56 => self.lsr(),
      0x58 => self.cli(),
      0x59 => self.eor(),
      0x5d => self.eor(),
      0x5e => self.lsr(),
      0x60 => self.rts(),
      0x61 => self.adc(),
      0x65 => self.adc(),
      0x66 => self.ror(),
      0x68 => self.pla(),
      0x69 => self.adc(),
      0x6a => self.ror(),
      0x6c => self.jmp(),
      0x6d => self.adc(),
      0x6e => self.ror(),
      0x70 => self.bvs(),
      0x71 => self.adc(),
      0x75 => self.adc(),
      0x76 => self.ror(),
      0x78 => self.sei(),
      0x79 => self.adc(),
      0x7d => self.adc(),
      0x7e => self.ror(),
      0x81 => self.sta(),
      0x84 => self.sty(),
      0x85 => self.sta(),
      0x86 => self.stx(),
      0x88 => self.dey(),
      0x8a => self.txa(),
      0x8c => self.sty(),
      0x8d => self.sta(),
      0x8e => self.stx(),
      0x90 => self.bcc(),
      0x91 => self.sta(),
      0x94 => self.sty(),
      0x95 => self.sta(),
      0x96 => self.stx(),
      0x98 => self.tya(),
      0x99 => self.sta(),
      0x9a => self.txs(),
      0x9d => self.sta(),
      0xa0 => self.ldy(),
      0xa1 => self.lda(),
      0xa2 => self.ldx(),
      0xa4 => self.ldy(),
      0xa5 => self.lda(),
      0xa6 => self.ldx(),
      0xa8 => self.tay(),
      0xa9 => self.lda(),
      0xaa => self.tax(),
      0xac => self.ldy(),
      0xad => self.lda(),
      0xae => self.ldx(),
      0xb0 => self.bcs(),
      0xb1 => self.lda(),
      0xb4 => self.ldy(),
      0xb5 => self.lda(),
      0xb6 => self.ldx(),
      0xb8 => self.clv(),
      0xb9 => self.lda(),
      0xba => self.tsx(),
      0xbc => self.ldy(),
      0xbd => self.lda(),
      0xbe => self.ldx(),
      0xc0 => self.cpy(),
      0xc1 => self.cmp(),
      0xc4 => self.cpy(),
      0xc5 => self.cmp(),
      0xc6 => self.dec(),
      0xc8 => self.iny(),
      0xc9 => self.cmp(),
      0xca => self.dex(),
      0xcc => self.cpy(),
      0xcd => self.cmp(),
      0xce => self.dec(),
      0xd0 => self.bne(),
      0xd1 => self.cmp(),
      0xd5 => self.cmp(),
      0xd6 => self.dec(),
      0xd8 => self.cld(),
      0xd9 => self.cmp(),
      0xdd => self.cmp(),
      0xde => self.dec(),
      0xe0 => self.cpx(),
      0xe1 => self.sbc(),
      0xe4 => self.cpx(),
      0xe5 => self.sbc(),
      0xe6 => self.inc(),
      0xe8 => self.inx(),
      0xe9 => self.sbc(),
      0xea => self.nop(),
      0xec => self.cpx(),
      0xed => self.sbc(),
      0xee => self.inc(),
      0xf0 => self.beq(),
      0xf1 => self.sbc(),
      0xf5 => self.sbc(),
      0xf6 => self.inc(),
      0xf8 => self.sed(),
      0xf9 => self.sbc(),
      0xfd => self.sbc(),
      0xfe => self.inc(),

      // 0x02 => self.jam(),
      0x03 => self.slo(),
      0x04 => self.nop(),
      0x07 => self.slo(),
      // 0x0b => self.anc(),
      0x0c => self.nop(),
      0x0f => self.slo(),
      // 0x12 => self.jam(),
      0x13 => self.slo(),
      0x14 => self.nop(),
      0x17 => self.slo(),
      0x1a => self.nop(),
      0x1b => self.slo(),
      0x1c => self.nop(),
      0x1f => self.slo(),
      // 0x22 => self.jam(),
      0x23 => self.rla(),
      0x27 => self.rla(),
      // 0x2b => self.anc(),
      0x2f => self.rla(),
      // 0x32 => self.jam(),
      0x33 => self.rla(),
      0x34 => self.nop(),
      0x37 => self.rla(),
      0x3a => self.nop(),
      0x3b => self.rla(),
      0x3c => self.nop(),
      0x3f => self.rla(),
      // 0x42 => self.jam(),
      0x43 => self.sre(),
      0x44 => self.nop(),
      0x47 => self.sre(),
      0x4b => self.alr(),
      0x4f => self.sre(),
      // 0x52 => self.jam(),
      0x53 => self.sre(),
      0x54 => self.nop(),
      0x57 => self.sre(),
      0x5a => self.nop(),
      0x5b => self.sre(),
      0x5c => self.nop(),
      0x5f => self.sre(),
      // 0x62 => self.jam(),
      0x63 => self.rra(),
      0x64 => self.nop(),
      0x67 => self.rra(),
      0x6b => self.arr(),
      0x6f => self.rra(),
      // 0x72 => self.jam(),
      0x73 => self.rra(),
      0x74 => self.nop(),
      0x77 => self.rra(),
      0x7a => self.nop(),
      0x7b => self.rra(),
      0x7c => self.nop(),
      0x7f => self.rra(),
      0x80 => self.nop(),
      0x82 => self.nop(),
      0x83 => self.sax(),
      0x87 => self.sax(),
      0x89 => self.nop(),
      // 0x8b => self.ane(),
      0x8f => self.sax(),
      // 0x92 => self.jam(),
      // 0x93 => self.sha(),
      0x97 => self.sax(),
      // 0x9b => self.tas(),
      // 0x9c => self.shy(),
      // 0x9e => self.shx(),
      // 0x9f => self.sha(),
      0xa3 => self.lax(),
      0xa7 => self.lax(),
      // 0xab => self.lxa(),
      0xaf => self.lax(),
      // 0xb2 => self.jam(),
      0xb3 => self.lax(),
      0xb7 => self.lax(),
      // 0xbb => self.las(),
      0xbf => self.lax(),
      0xc2 => self.nop(),
      0xc3 => self.dcp(),
      0xc7 => self.dcp(),
      // 0xcb => self.sbx(),
      0xcf => self.dcp(),
      // 0xd2 => self.jam(),
      0xd3 => self.dcp(),
      0xd4 => self.nop(),
      0xd7 => self.dcp(),
      0xda => self.nop(),
      0xdb => self.dcp(),
      0xdc => self.nop(),
      0xdf => self.dcp(),
      0xe2 => self.nop(),
      0xe3 => self.isc(),
      0xe7 => self.isc(),
      0xeb => self.sbc(),
      0xef => self.isc(),
      // 0xf2 => self.jam(),
      0xf3 => self.isc(),
      0xf4 => self.nop(),
      0xf7 => self.isc(),
      0xfa => self.nop(),
      0xfb => self.isc(),
      0xfc => self.nop(),
      0xff => self.isc(),
      _ => unreachable!("illegal opcode reached")
      // _ => {}
    }
  }
}