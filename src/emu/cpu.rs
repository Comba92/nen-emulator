#![allow(unused_imports)]
use std::{cell::RefCell, fmt::Debug, ops::{Shl, Shr}, rc::Rc};

use bitflags::bitflags;
use log::{debug, info, trace};

use super::{bus::Bus, instr::{AddressingMode, Instruction, INSTRUCTIONS, INSTR_TO_FN}};

bitflags! {
  #[derive(Debug, Clone, Copy)]
  pub struct CpuFlags: u8 {
    const carry     = 0b0000_0001;
    const zero      = 0b0000_0010;
    const interrupt = 0b0000_0100;
    const decimal   = 0b0000_1000;
    const brk       = 0b0001_0000;
    const unused    = 0b0010_0000;
    const brkpush   = 0b0011_0000;
    const overflow  = 0b0100_0000;
    const negative  = 0b1000_0000;
  }
}

// https://www.nesdev.org/wiki/CPU_ALL
pub const STACK_START: usize = 0x0100;
pub const STACK_END: usize = 0x0200;
// SP is always initialized at itself minus 3
// At boot, it is 0x00 - 0x03 = 0xFD
// After every successive restart, it will be SP - 0x03
const STACK_RESET: u8 = 0xFD;
const PC_RESET: u16 = 0xFFFC;
pub const MEM_SIZE: usize = 0x1_0000;

pub const INTERRUPT_NON_MASKABLE: u16 = 0xFFFA;
pub const INTERRUPT_RESET: u16 = 0xFFFC;
pub const INTERRUPT_REQUEST: u16 = 0xFFFE;


#[derive(Clone)]
pub struct Cpu {
  pub pc: u16,
  pub sp: u8,
  pub p: CpuFlags,
  pub a: u8,
  pub x: u8,
  pub y: u8,
  pub cycles: usize,
  pub bus: Rc<Bus>,
}

impl Debug for Cpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cpu").field("pc", &self.pc).field("sp", &self.sp).field("sr", &self.p).field("a", &self.a).field("x", &self.x).field("y", &self.y).field("cycles", &self.cycles).finish()
    }
}

impl Cpu {
  pub fn new(bus: Rc<Bus>) -> Self {    
    Self {
      pc: PC_RESET as u16,
      sp: STACK_RESET,
      a: 0, x: 0, y: 0,
      // At boot, only interrupt flag is enabled
      p: CpuFlags::from(CpuFlags::interrupt),
      cycles: 7,
      bus,
    }
  }

  pub fn set_carry(&mut self, res: u16) {
    self.p.set(CpuFlags::carry, res > u8::MAX as u16);
  }

  pub fn set_zero(&mut self, res: u8) {
    self.p.set(CpuFlags::zero, res == 0);
  }

  pub fn set_neg(&mut self, res: u8) {
    self.p.set(CpuFlags::negative, res & 0b1000_0000 != 0);
  }

  pub fn set_zn(&mut self, res: u8) {
    self.set_zero(res);
    self.set_neg(res);
  }
  
  pub fn set_czn(&mut self, res: u16) {
    self.set_carry(res);
    self.set_zn(res as u8);
  }

  // https://forums.nesdev.org/viewtopic.php?t=6331
  fn set_overflow(&mut self, a: u16, v: u16, s: u16) {
    let overflow = (a ^ s) & (v ^ s) & 0b1000_0000 != 0;
    self.p.set(CpuFlags::overflow, overflow);
  }

  pub fn carry(&self) -> u8 {
    self.p.contains(CpuFlags::carry).into()
  }

  pub fn mem_read(&mut self, addr: u16) -> u8 {
    //self.cycles = self.cycles.wrapping_add(1);
    self.bus.read(addr)
  }

  pub fn mem_read16(&mut self, addr: u16) -> u16 {
    u16::from_le_bytes([self.mem_read(addr), self.mem_read(addr+1)])
  }

  pub fn wrapping_read16(&mut self, addr: u16) -> u16 {
    if addr & 0x00FF == 0x00FF {
      let page = addr & 0xFF00;
      let low = self.mem_read(page | 0xFF);
      let high = self.mem_read(page | 0x00);
      u16::from_le_bytes([low, high])
    } else { self.mem_read16(addr) }
  }

  pub fn mem_write(&mut self, addr: u16, val: u8) {
    //self.cycles = self.cycles.wrapping_add(1);
    self.bus.write(addr, val);
  }

  pub fn mem_write16(&mut self, addr: u16, val: u16) {
    let [low, high] = val.to_le_bytes();
    self.bus.write(addr, low);
    self.bus.write(addr+1, high);
  }

  pub fn pc_fetch(&mut self) -> u8 {
    let res = self.mem_read(self.pc);
    self.pc = self.pc.wrapping_add(1);
    res
  }

  pub fn pc_fetch16(&mut self) -> u16 {
    let res = self.mem_read16(self.pc);
    self.pc = self.pc.wrapping_add(2);
    res
  }

  pub fn sp_addr(&self) -> u16 {
    // Stack grows downward
    STACK_START.wrapping_add(self.sp as usize) as u16
  }

  pub fn stack_push(&mut self, val: u8) {
    debug!("-> Pushing ${:02X} to stack at cycle {}", val, self.cycles);
    self.mem_write(self.sp_addr(), val);
    debug!("\t{}", self.stack_trace());
    self.sp = self.sp.wrapping_sub(1);
  }

  pub fn stack_push16(&mut self, val: u16) {
    let [low, high] = val.to_le_bytes();
    self.stack_push(high);
    self.stack_push(low);
  }

  pub fn stack_pull(&mut self) -> u8 {
    self.sp = self.sp.wrapping_add(1);
    debug!("<- Pulling ${:02X} from stack at cycle {}", self.mem_read(self.sp_addr()), self.cycles);
    debug!("\t{}", self.stack_trace());
    self.mem_read(self.sp_addr())
  }

  pub fn stack_pull16(&mut self) -> u16 {
    let low = self.stack_pull();
    let high = self.stack_pull();

    u16::from_le_bytes([low, high])
  }

  pub fn stack_trace(self: &mut Cpu) -> String {
    let mut s = String::new();
    const RANGE: i16 = 5;
    for i in -RANGE..=0 {
      let addr = self.sp_addr().wrapping_add_signed(i);
      s.push_str(&format!("${:02X}, ", self.mem_read(addr)));
    }

    s
  }
}

#[derive(Debug, Clone, Copy)]
enum OperandSrc {
  Acc, Addr(u16), None
}
#[derive(Debug)]
pub struct Operand {
  src: OperandSrc,
  val: u8,
}
pub enum InstrDst {
  Acc(u8), X(u8), Y(u8), Mem(u16, u8) 
}

pub type InstrFn = fn(&mut Cpu, &mut Operand);

impl Cpu {
  pub fn interpret(&mut self) {
    self.interpret_with_callback(|_| { false });
  }

  pub fn interpret_with_callback<F: FnMut(&mut Cpu) -> bool>(&mut self, mut callback: F) {
    loop {
      if callback(self) { break; };

      let opcode = self.pc_fetch();
      
      let instr = &INSTRUCTIONS[opcode as usize];
      let mut op = self.get_operand_with_addressing(&instr);
      
      let opname = instr.name.as_str();

      let (_, inst_fn) = INSTR_TO_FN
        .get_key_value(opname)
        .expect(&format!("Op {opcode}({}) should be in map", instr.name));
      
      inst_fn(self, &mut op);

      self.cycles += instr.cycles;
    }
  }

  fn get_zeropage_operand(&mut self, offset: u8) -> Operand {
    let zero_addr = (self.pc_fetch().wrapping_add(offset)) as u16;
    Operand { src: OperandSrc::Addr(zero_addr), val: self.mem_read(zero_addr) }
  }

  fn get_absolute_operand(&mut self, offset: u8, instr: &Instruction) -> Operand {
    let addr_base = self.pc_fetch16();
    let addr_effective = addr_base.wrapping_add(offset as u16);

    // page crossing check
    if instr.page_boundary_cycle && addr_effective & 0xFF00 != addr_base & 0xFF00 {
      self.cycles = self.cycles.wrapping_add(1);
    }

    Operand { src: OperandSrc::Addr(addr_effective), val: self.mem_read(addr_effective) }
  }

  pub fn get_operand_with_addressing(&mut self, instr: &Instruction) -> Operand {
    let mode = instr.addressing;
    use AddressingMode::*;
    use OperandSrc::*;
    
    let res = match mode {
      Implicit => Operand {src: None, val: 0},
      Accumulator => Operand {src: Acc, val: self.a},
      Immediate | Relative => Operand {src: None, val: self.pc_fetch()},
      ZeroPage => self.get_zeropage_operand(0),
      ZeroPageX => self.get_zeropage_operand(self.x),
      ZeroPageY => self.get_zeropage_operand(self.y),
      Absolute => self.get_absolute_operand(0, instr),
      AbsoluteX => self.get_absolute_operand(self.x, instr),
      AbsoluteY => self.get_absolute_operand(self.y, instr),
      Indirect => {
        let addr = self.pc_fetch16();
        let addr_effective = self.wrapping_read16(addr);
        Operand { src: Addr(addr_effective), val: 0 }
      }
      IndirectX => {
        let zero_addr = (self.pc_fetch().wrapping_add(self.x)) as u16;
        let addr_effective = self.wrapping_read16(zero_addr);
        trace!("[IndirectX] ZeroAddr: {zero_addr:02X}, Effective: {addr_effective:04X}");
        Operand { src: Addr(addr_effective), val: self.mem_read(addr_effective) }
      }
      IndirectY => {
        let zero_addr = self.pc_fetch() as u16;
        let addr_base = self.wrapping_read16(zero_addr);
        let addr_effective = addr_base.wrapping_add(self.y as u16);

        trace!("[IndirectY] ZeroAddr: {zero_addr:04X}, BaseAddr: {addr_base:04X}, Effective: {addr_effective:04X}");
        trace!(" | Has crossed boundaries? {}", addr_effective & 0xFF00 != addr_base & 0xFF00);
        
        // page crossing check
        if instr.page_boundary_cycle && addr_effective & 0xFF00 != addr_base & 0xFF00 {
          trace!(" | Boundary crossed at cycle {}", self.cycles);
          self.cycles = self.cycles.wrapping_add(1);
        }
        Operand { src: Addr(addr_effective), val: self.mem_read(addr_effective) }
      }
    };

    res
  }

  pub fn set_instr_result(&mut self, dst: InstrDst) {
    match dst {
      InstrDst::Acc(res) => self.a = res,
      InstrDst::X(res) => self.x = res,
      InstrDst::Y(res) => self.y = res,
      InstrDst::Mem(addr, res) => self.mem_write(addr, res),
    }
  }

  pub fn load (&mut self, op: &mut Operand, dst: InstrDst) {
    trace!("[LOAD] {op:?} at cycle {}", self.cycles);
    self.set_zn(op.val);
    self.set_instr_result(dst);
  }

  pub fn lda(&mut self, op: &mut Operand) {
    self.load(op, InstrDst::Acc(op.val))
  }
  pub fn ldx(&mut self, op: &mut Operand) {
    self.load(op, InstrDst::X(op.val))
  }
  pub fn ldy(&mut self, op: &mut Operand) {
    self.load(op, InstrDst::Y(op.val))
  }

  pub fn store(&mut self, op: &mut Operand, val: u8) {
    if let OperandSrc::Addr(src) = op.src {
      self.set_instr_result(InstrDst::Mem(src, val))
    } else { unreachable!() }  
  }

  pub fn sta(&mut self, op: &mut Operand) {
    self.store(op, self.a)
  }
  pub fn stx(&mut self, op: &mut Operand) {
    self.store(op, self.x)
  }
  pub fn sty(&mut self, op: &mut Operand) {
    self.store(op, self.y)
  }

  pub fn transfer(&mut self, src: u8, dst: InstrDst) {
    self.set_zn(src);
    self.set_instr_result(dst);
  }

  pub fn tax(&mut self, _: &mut Operand) {
    self.transfer(self.a, InstrDst::X(self.a))
  }
  pub fn tay(&mut self, _: &mut Operand) {
    self.transfer(self.a, InstrDst::Y(self.a))
  }
  pub fn tsx(&mut self, _: &mut Operand) {
    self.transfer(self.sp, InstrDst::X(self.sp))
  }
  pub fn txa(&mut self, _: &mut Operand) {
    self.transfer(self.x, InstrDst::Acc(self.x))
  }
  pub fn txs(&mut self, _: &mut Operand) {
    debug!("SP changed from ${:02X} to ${:02X}", self.sp, self.x);
    self.sp = self.x;
  }
  pub fn tya(&mut self, _: &mut Operand) {
    self.transfer(self.y, InstrDst::Acc(self.y))
  }

  pub fn pha(&mut self, _: &mut Operand) {
    trace!("[PHA] Pushing ${:02X} to stack at cycle {}", self.a, self.cycles);
    self.stack_push(self.a);
  }
  pub fn pla(&mut self, _: &mut Operand) {
    let res = self.stack_pull();
    self.set_zn(res);
    self.a = res;
    trace!("[PLA] Pulled ${:02X} from stack at cycle {}", self.a, self.cycles);
  }
  pub fn php(&mut self, _: &mut Operand) {
    // Brk is always 1 on pushes
    let pushable = self.p.union(CpuFlags::brkpush);
    trace!("[PHP] Pushing {pushable:?} (${:02X}) to stack at cycle {}", pushable.bits(), self.cycles);
    self.stack_push(pushable.bits());
  }
  pub fn plp(&mut self, _: &mut Operand) {
    let res = self.stack_pull();
    // Brk is always 0 on pulls, but unused is always 1
    self.p = CpuFlags::from_bits_retain(res)
      .difference(CpuFlags::brk)
      .union(CpuFlags::unused);
    trace!("[PLP] Pulled {:?} (${:02X}) from stack at cycle {}", self.p, self.p.bits(), self.cycles);
  }

  pub fn logical(&mut self, res: u8) {
    self.set_zero(res);
    self.set_neg(res);
    self.a = res;
  }
  pub fn and(&mut self, op: &mut Operand) {
    self.logical(self.a & op.val)
  }
  pub fn eor(&mut self, op: &mut Operand) {
    self.logical(self.a ^ op.val)
  }
  pub fn ora(&mut self, op: &mut Operand) {
    self.logical(self.a | op.val)
  }
  pub fn bit(&mut self, op: &mut Operand) {
    let res = self.a & op.val;
    self.set_zero(res);
    self.p.set(CpuFlags::overflow, op.val & 0b0100_0000 != 0);
    self.p.set(CpuFlags::negative, op.val & 0b1000_0000 != 0);
  }

  pub fn adc(&mut self, op: &mut Operand) {
    let res = self.a as u16 + op.val as u16 + self.carry() as u16;
    self.set_overflow(self.a as u16, op.val as u16, res);
    self.set_czn(res);
    self.a = res as u8;
  }
  pub fn sbc(&mut self, op: &mut Operand) {
    self.adc(&mut Operand { val: !op.val, src: op.src });
  }

  pub fn compare(&mut self, a: u8, b: u8) {
    let res = a.wrapping_sub(b);
    self.set_czn(res as u16);
    self.p.set(CpuFlags::carry, a >= b);
  }

  pub fn cmp(&mut self, op: &mut Operand) {
    self.compare(self.a, op.val)
  }
  pub fn cpx(&mut self, op: &mut Operand) {
    self.compare(self.x, op.val)
  }
  pub fn cpy(&mut self, op: &mut Operand) {
    self.compare(self.y, op.val)
  }

  pub fn increase(&mut self, val: u8, f: fn(u8, u8) -> u8) -> u8 {
    let res = f(val, 1);
    self.set_zn(res);
    res
  }
  pub fn inc(&mut self, op: &mut Operand) {
    if let OperandSrc::Addr(src) = op.src {
      op.val = self.increase(op.val, u8::wrapping_add);
      self.mem_write(src, op.val);
    } else { unreachable!() }
  }
  pub fn inx(&mut self, _: &mut Operand) {
    self.x = self.increase(self.x, u8::wrapping_add);
  }
  pub fn iny(&mut self, _: &mut Operand) {
    self.y = self.increase(self.y, u8::wrapping_add)
  }
  pub fn dec(&mut self, op: &mut Operand) {
    if let OperandSrc::Addr(src) = op.src {
      op.val = self.increase(op.val, u8::wrapping_sub);
      self.mem_write(src, op.val);
    } else { unreachable!() }
  }
  pub fn dex(&mut self, _: &mut Operand) {
    self.x = self.increase(self.x, u8::wrapping_sub);
  }
  pub fn dey(&mut self, _: &mut Operand) {
    self.y = self.increase(self.y, u8::wrapping_sub);
  }

  pub fn shift<F: Fn(u8) -> u8>(&mut self, op: &mut Operand, carry: bool, f: F) {
    self.p.set(CpuFlags::carry, carry);
    op.val = f(op.val);
    self.set_zn(op.val);

    match op.src {
      OperandSrc::Acc => self.a = op.val,
      OperandSrc::Addr(src) => self.mem_write(src, op.val),
      OperandSrc::None => { unreachable!() }
    }
  }
  pub fn asl(&mut self, op: &mut Operand) {
    self.shift(op, op.val & 0b1000_0000 != 0, |v| v.shl(1));
  }
  pub fn lsr(&mut self, op: &mut Operand) {
    self.shift(op, op.val & 1 != 0, |v| v.shr(1));
  }
  pub fn rol(&mut self, op: &mut Operand) {
    let carry = self.carry();
    self.shift(op, op.val & 0b1000_0000 != 0, |v| v.shl(1) | carry);
  }
  pub fn ror(&mut self, op: &mut Operand) {
    let carry = self.carry();
    self.shift(op, op.val & 1 != 0, |v| v.shr(1) | (carry << 7));
  }

  pub fn jmp(&mut self, op: &mut Operand) {
    if let OperandSrc::Addr(src) = op.src {
      self.pc = src;
    } else { unreachable!() }
  }
  pub fn jsr(&mut self, op: &mut Operand) {
    self.stack_push16(self.pc - 1);
    self.jmp(op);
  }
  pub fn rts(&mut self, _: &mut Operand) {
    self.pc = self.stack_pull16() + 1;
  }

  pub fn branch(&mut self, offset: u8, cond: bool) {
    if cond {
      let offset = offset as i8;
      let new_pc = self.pc.wrapping_add_signed(offset as i16);
      
      // page boundary cross check
      if self.pc & 0xFF00 != new_pc & 0xFF00 {
        // page cross branch costs 2
        self.cycles = self.cycles.wrapping_add(2);
      } else {
        // same page branch costs 1
        self.cycles = self.cycles.wrapping_add(1);
      }

      self.pc = new_pc;
    }
  }
  pub fn bcc(&mut self, op: &mut Operand) {
    self.branch(op.val, self.carry() == 0)
  }
  pub fn bcs(&mut self, op: &mut Operand) {
    self.branch(op.val, self.carry() == 1)
  }
  pub fn beq(&mut self, op: &mut Operand) {
    self.branch(op.val, self.p.contains(CpuFlags::zero))
  }
  pub fn bne(&mut self, op: &mut Operand) {
    self.branch(op.val, !self.p.contains(CpuFlags::zero))
  }
  pub fn bmi(&mut self, op: &mut Operand) {
    self.branch(op.val, self.p.contains(CpuFlags::negative))
  }
  pub fn bpl(&mut self, op: &mut Operand) {
    self.branch(op.val, !self.p.contains(CpuFlags::negative))
  }
  pub fn bvc(&mut self, op: &mut Operand) {
    self.branch(op.val, !self.p.contains(CpuFlags::overflow))
  }
  pub fn bvs(&mut self, op: &mut Operand) {
    self.branch(op.val, self.p.contains(CpuFlags::overflow))
  }

  pub fn clear_stat(&mut self, s: CpuFlags) {
    self.p.remove(s);
  }
  pub fn clc(&mut self, _: &mut Operand) {
    self.clear_stat(CpuFlags::carry)
  }
  pub fn cld(&mut self, _: &mut Operand) {
    self.clear_stat(CpuFlags::decimal)
  }
  pub fn cli(&mut self, _: &mut Operand) {
    self.clear_stat(CpuFlags::interrupt)
  }
  pub fn clv(&mut self, _: &mut Operand) {
    self.clear_stat(CpuFlags::overflow)
  }
  pub fn set_stat(&mut self, s: CpuFlags) {
    self.p.insert(s);
  }
  pub fn sec(&mut self, _: &mut Operand) {
    self.set_stat(CpuFlags::carry)
  }
  pub fn sed(&mut self, _: &mut Operand) {
    self.set_stat(CpuFlags::decimal)
  }
  pub fn sei(&mut self, _: &mut Operand) {
    self.set_stat(CpuFlags::interrupt)
  }

  pub fn brk(&mut self, op: &mut Operand) {
    self.stack_push16(self.pc);
    self.php(op);
  }
  pub fn rti(&mut self, op: &mut Operand) {
    self.plp(op);
    self.pc = self.stack_pull16();
  }
  pub fn nop(&mut self, _: &mut Operand) {}
}

impl Cpu {
  pub fn asr(&mut self, op: &mut Operand) {
    self.and(op);
    self.lsr(op);
  }

  pub fn slo(&mut self, op: &mut Operand) {
    self.asl(op);
    self.ora(op);
  }

  pub fn sre(&mut self, op: &mut Operand) {
    self.lsr(op);
    self.eor(op);
  }

  pub fn anc(&mut self, op: &mut Operand) {
    self.and(op);
    self.p.set(CpuFlags::carry, self.p.contains(CpuFlags::negative));
  }

  pub fn arr(&mut self, op: &mut Operand) {
    self.and(op);
    self.ror(op);
  }

  pub fn dcp(&mut self, op: &mut Operand) {
    self.dec(op);
    self.cmp(op);
  }

  pub fn isc(&mut self, op: &mut Operand) {
    self.inc(op);
    self.sbc(op); 
  }

  pub fn rla(&mut self, op: &mut Operand) {
    self.rol(op);
    self.and(op);
  }

  pub fn rra(&mut self, op: &mut Operand) {
    self.ror(op);
    self.adc(op);
  }

  pub fn las(&mut self, op: &mut Operand) {
    let res = op.val & self.sp;
    self.a = res;
    self.x = res;
    self.sp = res;
    self.set_zn(res);
  }

  pub fn lax(&mut self, op: &mut Operand) {
    self.a = op.val;
    self.x = op.val;
    self.set_zn(op.val);
  }

  pub fn sax(&mut self, op: &mut Operand) {
    if let OperandSrc::Addr(src) = op.src {
      self.set_instr_result(InstrDst::Mem(src, self.a & self.x));
    } else { unreachable!() }
  }

  pub fn sha(&mut self, _op: &mut Operand) {
    todo!("sha")
  }

  pub fn tas(&mut self, _op: &mut Operand) {
    todo!("shs/tas")
  }

  pub fn shx(&mut self, _op: &mut Operand) {
    todo!("shx")
  }

  pub fn shy(&mut self, _op: &mut Operand) {
    todo!("shy")
  }

  pub fn sbx(&mut self, op: &mut Operand) {
    if let OperandSrc::Addr(_src) = op.src {
      todo!("sbx")
    } else { unreachable!() }
  }

  pub fn ane(&mut self, _op: &mut Operand) {
    todo!("ane/xaa")
  }

  pub fn lxa(&mut self, _op: &mut Operand) {
    todo!("lxa")
  }

  pub fn usbc(&mut self, op: &mut Operand) {
    self.sbc(op);
  }

  pub fn jam(&mut self, _: &mut Operand) {
    todo!("jam")
  }
}
