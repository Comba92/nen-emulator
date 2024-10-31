#![allow(unused_imports)]
use std::{cell::RefCell, fmt::Debug, ops::{Shl, Shr}, rc::Rc};

use bitflags::bitflags;
use log::{debug, info, trace};
use super::instr::{AddressingMode, Instruction, INSTRUCTIONS, INSTR_TO_FN};

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
pub const MEM_SIZE: usize = 0x10000;

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
  pub mem: Rc<RefCell<[u8; MEM_SIZE]>>,
}

impl Debug for Cpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cpu").field("pc", &self.pc).field("sp", &self.sp).field("sr", &self.p).field("a", &self.a).field("x", &self.x).field("y", &self.y).field("cycles", &self.cycles).finish()
    }
}

impl Cpu {
  pub fn new() -> Self {    
    Self {
      pc: PC_RESET as u16,
      sp: STACK_RESET,
      a: 0, x: 0, y: 0,
      // At boot, only interrupt flag is enabled
      p: CpuFlags::from(CpuFlags::interrupt),
      cycles: 7,
      mem: Rc::new(RefCell::new([0; MEM_SIZE])),
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

  pub fn write_data(&mut self, addr: usize, data: &[u8]) {
    let mut mem = self.mem.borrow_mut();
    mem[addr..addr+data.len()].copy_from_slice(data);
  }

  pub fn mem_fetch(&mut self, addr: u16) -> u8 {
    //self.cycles = self.cycles.wrapping_add(1);
    self.mem.borrow()[addr as usize]
  }

  pub fn mem_fetch16(&mut self, addr: u16) -> u16 {
    u16::from_le_bytes([self.mem_fetch(addr), self.mem_fetch(addr+1)])
  }

  pub fn zero_fetch16(&mut self, addr: u16) -> u16 {
    if addr == 0xFF {
      let low = self.mem_fetch(0xFF);
      let high = self.mem_fetch(0x00);
      u16::from_le_bytes([low, high])
    } else { self.mem_fetch16(addr) }
  }

  pub fn mem_set(&mut self, addr: u16, val: u8) {
    //self.cycles = self.cycles.wrapping_add(1);
    self.mem.borrow_mut()[addr as usize] = val;
  }

  pub fn mem_set16(&mut self, addr: u16, val: u16) {
    let [low, high] = val.to_le_bytes();
    self.mem.borrow_mut()[addr as usize] = low;
    self.mem.borrow_mut()[(addr + 1) as usize] = high;
  }

  pub fn fetch_at_pc(&mut self) -> u8 {
    let res = self.mem_fetch(self.pc);
    self.pc+=1;
    res
  }

  pub fn fetch16_at_pc(&mut self) -> u16 {
    let res = self.mem_fetch16(self.pc);
    self.pc+=2;
    res
  }

  pub fn sp_addr(&self) -> u16 {
    // Stack grows downward
    STACK_START.wrapping_add(self.sp as usize) as u16
  }

  pub fn stack_push(&mut self, val: u8) {
    debug!("-> Pushing ${:02X} to stack at cycle {}", val, self.cycles);
    self.mem_set(self.sp_addr(), val);
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
    debug!("<- Pulling ${:02X} from stack at cycle {}", self.mem_fetch(self.sp_addr()), self.cycles);
    debug!("\t{}", self.stack_trace());
    self.mem_fetch(self.sp_addr())
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
      s.push_str(&format!("${:02X}, ", self.mem_fetch(addr)));
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

pub type InstrFn = fn(&mut Cpu, &Operand);

impl Cpu {
  pub fn interpret(&mut self) {
    self.interpret_with_callback(|_| {});
  }

  pub fn interpret_with_callback<F: FnMut(&mut Cpu)>(&mut self, mut callback: F) {
    loop {
      callback(self);

      let opcode = self.fetch_at_pc();
      
      let instr = &INSTRUCTIONS[opcode as usize];
      let operand = self.get_operand_with_addressing(&instr);
      
      let opname = instr.name.as_str();

      let (_, inst_fn) = INSTR_TO_FN
        .get_key_value(opname)
        .expect(&format!("Op {opcode}({}) should be in map", instr.name));
      
      inst_fn(self, &operand);

      self.cycles += instr.cycles;
    }
  }

  pub fn get_operand_with_addressing(&mut self, inst: &Instruction) -> Operand {
    let mode = inst.addressing;
    use AddressingMode::*;
    use OperandSrc::*;

    let res = match mode {
      Implicit => Operand {src: None, val: 0},
      Accumulator => Operand {src: Acc, val: self.a},
      Immediate | Relative => Operand {src: None, val: self.fetch_at_pc()},
      ZeroPage => {
        let zero_addr = self.fetch_at_pc() as u16;
        Operand { src: Addr(zero_addr), val: self.mem_fetch(zero_addr) }
      }
      ZeroPageX => {
        let zero_addr = (self.fetch_at_pc().wrapping_add(self.x)) as u16;
        Operand { src: Addr(zero_addr), val: self.mem_fetch(zero_addr) }
      }
      IndirectX => {
        let zero_addr = (self.fetch_at_pc().wrapping_add(self.x)) as u16;
        let addr_effective = self.zero_fetch16(zero_addr);
        info!("[IndirectX] ZeroAddr: {zero_addr:02X}, Effective: {addr_effective:04X}");
        Operand { src: Addr(addr_effective), val: self.mem_fetch(addr_effective) }
      }
      ZeroPageY => {
        let zero_addr = (self.fetch_at_pc().wrapping_add(self.y)) as u16;
        Operand { src: Addr(zero_addr), val: self.mem_fetch(zero_addr) }
      }
      IndirectY => {
        let zero_addr = self.fetch_at_pc() as u16;
        let addr_effective = self.zero_fetch16(zero_addr)
        .wrapping_add(self.y as u16).wrapping_add(self.carry() as u16);
        info!("[IndirectY] ZeroAddr: {zero_addr:02X}, Effective: {addr_effective:04X}");
        // page crossing check
        if addr_effective & 0xFF00 != self.pc & 0xFF00 {
          self.cycles = self.cycles.wrapping_add(1);
        }

        Operand { src: Addr(addr_effective), val: self.mem_fetch(addr_effective) }
      }
      Absolute => {
        let addr = self.fetch16_at_pc();
        Operand { src: Addr(addr), val: self.mem_fetch(addr) }
      }
      //TODO: should be done wrapping add?
      AbsoluteX => { 
        let addr = self.fetch16_at_pc() + self.x as u16 + self.carry() as u16;
        // page crossing check
        if addr & 0xFF00 != self.pc & 0xFF00 {
          self.cycles = self.cycles.wrapping_add(1);
        }

        Operand { src: Addr(addr), val: self.mem_fetch(addr) }
      }
      //TODO: should be done wrapping add?
      AbsoluteY => {
        let addr = self.fetch16_at_pc() + self.y as u16 + self.carry() as u16;
        // page crossing check
        if addr & 0xFF00 != self.pc & 0xFF00 {
          self.cycles = self.cycles.wrapping_add(1);
        }
        
        Operand { src: Addr(addr), val: self.mem_fetch(addr) }
      }
      Indirect => {
        let addr = self.fetch16_at_pc();
        let addr_effective = self.mem_fetch16(addr);
        Operand { src: Addr(addr_effective), val: 0 }
      }
    };

    res
  }

  pub fn set_instr_result(&mut self, dst: InstrDst) {
    match dst {
      InstrDst::Acc(res) => self.a = res,
      InstrDst::X(res) => self.x = res,
      InstrDst::Y(res) => self.y = res,
      InstrDst::Mem(addr, res) => self.mem_set(addr, res),
    }
  }

  pub fn load (&mut self, operand: &Operand, dst: InstrDst) {
    info!("[LOAD] {operand:?} at cycle {}", self.cycles);
    self.set_zn(operand.val);
    self.set_instr_result(dst);
  }

  pub fn lda(&mut self, operand: &Operand) {
    self.load(operand, InstrDst::Acc(operand.val))
  }
  pub fn ldx(&mut self, operand: &Operand) {
    self.load(operand, InstrDst::X(operand.val))
  }
  pub fn ldy(&mut self, operand: &Operand) {
    self.load(operand, InstrDst::Y(operand.val))
  }

  pub fn store(&mut self, operand: &Operand, val: u8) {
    if let OperandSrc::Addr(src) = operand.src {
      self.set_instr_result(InstrDst::Mem(src, val))
    } else { unreachable!() }  
  }

  pub fn sta(&mut self, operand: &Operand) {
    self.store(operand, self.a)
  }
  pub fn stx(&mut self, operand: &Operand) {
    self.store(operand, self.x)
  }
  pub fn sty(&mut self, operand: &Operand) {
    self.store(operand, self.y)
  }

  pub fn transfer(&mut self, src: u8, dst: InstrDst) {
    self.set_zn(src);
    self.set_instr_result(dst);
  }

  pub fn tax(&mut self, _: &Operand) {
    self.transfer(self.a, InstrDst::X(self.a))
  }
  pub fn tay(&mut self, _: &Operand) {
    self.transfer(self.a, InstrDst::Y(self.a))
  }
  pub fn tsx(&mut self, _: &Operand) {
    self.transfer(self.sp, InstrDst::X(self.sp))
  }
  pub fn txa(&mut self, _: &Operand) {
    self.transfer(self.x, InstrDst::Acc(self.x))
  }
  pub fn txs(&mut self, _: &Operand) {
    debug!("SP changed from ${:02X} to ${:02X}", self.sp, self.x);
    self.sp = self.x;
  }
  pub fn tya(&mut self, _: &Operand) {
    self.transfer(self.y, InstrDst::Acc(self.y))
  }

  pub fn pha(&mut self, _: &Operand) {
    trace!("[PHA] Pushing ${:02X} to stack at cycle {}", self.a, self.cycles);
    self.stack_push(self.a);
  }
  pub fn pla(&mut self, _: &Operand) {
    let res = self.stack_pull();
    self.set_zn(res);
    self.a = res;
    trace!("[PLA] Pulled ${:02X} from stack at cycle {}", self.a, self.cycles);
  }
  pub fn php(&mut self, _: &Operand) {
    // Brk is always 1 on pushes
    let pushable = self.p.union(CpuFlags::brkpush);
    trace!("[PHP] Pushing {pushable:?} (${:02X}) to stack at cycle {}", pushable.bits(), self.cycles);
    self.stack_push(pushable.bits());
  }
  pub fn plp(&mut self, _: &Operand) {
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
  pub fn and(&mut self, operand: &Operand) {
    self.logical(self.a & operand.val)
  }
  pub fn eor(&mut self, operand: &Operand) {
    self.logical(self.a ^ operand.val)
  }
  pub fn ora(&mut self, operand: &Operand) {
    self.logical(self.a | operand.val)
  }
  pub fn bit(&mut self, operand: &Operand) {
    let res = self.a & operand.val;
    self.set_zero(res);
    self.p.set(CpuFlags::overflow, operand.val & 0b0100_0000 != 0);
    self.p.set(CpuFlags::negative, operand.val & 0b1000_0000 != 0);
  }

  pub fn adc(&mut self, operand: &Operand) {
    let res = self.a as u16 + operand.val as u16 + self.carry() as u16;
    self.set_overflow(self.a as u16, operand.val as u16, res);
    self.set_czn(res);
    self.a = res as u8;
  }
  pub fn sbc(&mut self, operand: &Operand) {
    self.adc(&Operand { val: !operand.val, src: operand.src });
  }

  pub fn compare(&mut self, a: u8, b: u8) {
    let res = a.wrapping_sub(b);
    self.set_czn(res as u16);
    self.p.set(CpuFlags::carry, a >= b);
  }

  pub fn cmp(&mut self, operand: &Operand) {
    self.compare(self.a, operand.val)
  }
  pub fn cpx(&mut self, operand: &Operand) {
    self.compare(self.x, operand.val)
  }
  pub fn cpy(&mut self, operand: &Operand) {
    self.compare(self.y, operand.val)
  }

  pub fn increase(&mut self, val: u8, f: fn(u8, u8) -> u8) -> u8 {
    let res = f(val, 1);
    self.set_zn(res);
    res
  }
  pub fn inc(&mut self, operand: &Operand) {
    if let OperandSrc::Addr(src) = operand.src {
      let res = self.increase(operand.val, u8::wrapping_add);
      self.mem_set(src, res);
    } else { unreachable!() }
  }
  pub fn inx(&mut self, _: &Operand) {
    self.x = self.increase(self.x, u8::wrapping_add);
  }
  pub fn iny(&mut self, _: &Operand) {
    self.y = self.increase(self.y, u8::wrapping_add)
  }
  pub fn dec(&mut self, operand: &Operand) {
    if let OperandSrc::Addr(src) = operand.src {
      let res = self.increase(operand.val, u8::wrapping_sub);
      self.mem_set(src, res);
    } else { unreachable!() }
  }
  pub fn dex(&mut self, _: &Operand) {
    self.x = self.increase(self.x, u8::wrapping_sub);
  }
  pub fn dey(&mut self, _: &Operand) {
    self.y = self.increase(self.y, u8::wrapping_sub);
  }

  pub fn shift<F: Fn(u8) -> u8>(&mut self, operand: &Operand, carry: bool, f: F) {
    self.p.set(CpuFlags::carry, carry);
    let res = f(operand.val);
    self.set_zn(res);

    match operand.src {
      OperandSrc::Acc => self.a = res,
      OperandSrc::Addr(src) => self.mem_set(src, res),
      OperandSrc::None => { unreachable!() }
    }
  }
  pub fn asl(&mut self, operand: &Operand) {
    self.shift(operand, operand.val & 0b1000_0000 != 0, |v| v.shl(1));
  }
  pub fn lsr(&mut self, operand: &Operand) {
    self.shift(operand, operand.val & 1 != 0, |v| v.shr(1));
  }
  pub fn rol(&mut self, operand: &Operand) {
    let carry = self.carry();
    self.shift(operand, operand.val & 0b1000_0000 != 0, |v| v.shl(1) | carry);
  }
  pub fn ror(&mut self, operand: &Operand) {
    let carry = self.carry();
    self.shift(operand, operand.val & 1 != 0, |v| v.shr(1) | (carry << 7));
  }

  pub fn jmp(&mut self, operand: &Operand) {
    if let OperandSrc::Addr(src) = operand.src {
      self.pc = src;
    } else { unreachable!() }
  }
  pub fn jsr(&mut self, operand: &Operand) {
    self.stack_push16(self.pc - 1);
    self.jmp(operand);
  }
  pub fn rts(&mut self, _: &Operand) {
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
  pub fn bcc(&mut self, operand: &Operand) {
    self.branch(operand.val, self.carry() == 0)
  }
  pub fn bcs(&mut self, operand: &Operand) {
    self.branch(operand.val, self.carry() == 1)
  }
  pub fn beq(&mut self, operand: &Operand) {
    self.branch(operand.val, self.p.contains(CpuFlags::zero))
  }
  pub fn bne(&mut self, operand: &Operand) {
    self.branch(operand.val, !self.p.contains(CpuFlags::zero))
  }
  pub fn bmi(&mut self, operand: &Operand) {
    self.branch(operand.val, self.p.contains(CpuFlags::negative))
  }
  pub fn bpl(&mut self, operand: &Operand) {
    self.branch(operand.val, !self.p.contains(CpuFlags::negative))
  }
  pub fn bvc(&mut self, operand: &Operand) {
    self.branch(operand.val, !self.p.contains(CpuFlags::overflow))
  }
  pub fn bvs(&mut self, operand: &Operand) {
    self.branch(operand.val, self.p.contains(CpuFlags::overflow))
  }

  pub fn clear_stat(&mut self, s: CpuFlags) {
    self.p.remove(s);
  }
  pub fn clc(&mut self, _: &Operand) {
    self.clear_stat(CpuFlags::carry)
  }
  pub fn cld(&mut self, _: &Operand) {
    self.clear_stat(CpuFlags::decimal)
  }
  pub fn cli(&mut self, _: &Operand) {
    self.clear_stat(CpuFlags::interrupt)
  }
  pub fn clv(&mut self, _: &Operand) {
    self.clear_stat(CpuFlags::overflow)
  }
  pub fn set_stat(&mut self, s: CpuFlags) {
    self.p.insert(s);
  }
  pub fn sec(&mut self, _: &Operand) {
    self.set_stat(CpuFlags::carry)
  }
  pub fn sed(&mut self, _: &Operand) {
    self.set_stat(CpuFlags::decimal)
  }
  pub fn sei(&mut self, _: &Operand) {
    self.set_stat(CpuFlags::interrupt)
  }

  pub fn brk(&mut self, operand: &Operand) {
    self.stack_push16(self.pc);
    self.php(operand);
  }
  pub fn rti(&mut self, operand: &Operand) {
    self.plp(operand);
    self.pc = self.stack_pull16();
  }
  pub fn nop(&mut self, _: &Operand) {}
}

#[cfg(test)]
mod tests {
use super::*;

  #[test]
  fn signed_test() {
    let unsigned = 130u8;
    let signed = unsigned as i8;
    let signed16 = (unsigned as i8) as i16;

    assert_eq!(signed, -126);
    assert_eq!(signed16, -126);
  }

  #[test]
  fn cpu_test() {
    let mut cpu = Cpu::new();
    let codes = vec![0x69, 0x01, 0x69, 0x05];
    cpu.write_data(0, &codes);

    cpu.interpret();

    assert_eq!(cpu.a, 6);
  }
}