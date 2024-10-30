#![allow(dead_code)]
use std::{cell::RefCell, fmt::Debug, rc::Rc};

use bitflags::bitflags;
use super::instr::{AddressingMode, Instruction, INSTRUCTIONS, INSTR_TO_FN};

bitflags! {
  #[derive(Debug, Clone, Copy)]
  pub struct Status: u8 {
    const carry     = 0b0000_0001;
    const zero      = 0b0000_0010;
    const interrupt = 0b0000_0100;
    const decimal   = 0b0000_1000;
    const r#break   = 0b0001_0000;
    const overflow  = 0b0100_0000;
    const negative  = 0b1000_0000;
  }
}

// https://www.nesdev.org/wiki/CPU_ALL
const STACK_START: usize = 0x0100;
// SP is always initialized at itself minus 3
// At boot, it is 0x00 - 0x03 = 0xFD
// After every successive restart, it will be SP - 0x03
const STACK_RESET: u8 = 0xFD;
const PC_RESET: u16 = 0xFFFC;
const MEM_SIZE: usize = 0x10000;
const INTERRUPT_TABLE: usize = 0xFFFA;

#[derive(Clone)]
pub struct Cpu {
  pub pc: u16,
  pub sp: u8,
  pub p: Status,
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
      p: Status::from(Status::interrupt),
      cycles: 7,
      mem: Rc::new(RefCell::new([0; MEM_SIZE])),
    }
  }

  pub fn set_czn(&mut self, res: u16) {
    if res > u8::MAX as u16 { self.p.insert(Status::carry); } 
    if res == 0 { self.p.insert(Status::zero); }
    if res & 0b1000_0000 == 1 { self.p.insert(Status::negative); }
  }

  // https://forums.nesdev.org/viewtopic.php?t=6331
  fn set_overflow(&mut self, a: u16, v: u16, s: u16) {
    let overflow = (a ^ s) & (v ^ s) & 0b1000_0000 != 0;
    if overflow { self.p.insert(Status::overflow); }
  }

  pub fn carry(&self) -> u8 {
    self.p.contains(Status::carry).into()
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
    //self.cycles = self.cycles.wrapping_add(2);
    u16::from_le_bytes([self.mem_fetch(addr), self.mem_fetch(addr+1)])
  }

  pub fn mem_set(&mut self, addr: u16, val: u8) {
    //self.cycles = self.cycles.wrapping_add(1);
    self.mem.borrow_mut()[addr as usize] = val;
  }

  pub fn mem_set16(&mut self, addr: u16, val: u16) {
    //self.cycles = self.cycles.wrapping_add(2);

    let [first, second] = val.to_le_bytes();
    self.mem.borrow_mut()[addr as usize] = first;
    self.mem.borrow_mut()[(addr + 1) as usize] = second;
  }

  pub fn fetch_at_pc(&mut self) -> u8 {
    self.mem_fetch(self.pc)
  }

  pub fn fetch16_at_pc(&mut self) -> u16 {
    self.mem_fetch16(self.pc)
  }

  pub fn stack_push(&mut self, val: u8) {
    self.mem_set(STACK_START as u16 + self.sp as u16, val);
    self.sp -= 1;
  }

  pub fn stack_push16(&mut self, val: u16) {
    self.mem_set16(STACK_START as u16 + self.sp as u16, val);
    self.sp -= 2;
  }

  pub fn stack_pop(&mut self) -> u8 {
    self.sp += 1;
    self.mem_fetch(STACK_START as u16 + self.sp as u16)
  }

  pub fn stack_pop16(&mut self) -> u16 {
    self.sp += 2;
    self.mem_fetch16(STACK_START as u16 + self.sp as u16)
  }
}


pub fn interpret(cpu: &mut Cpu) {
  interpret_with_callback(cpu, |_| {});
}

pub fn interpret_with_callback<F: FnMut(&mut Cpu)>(mut cpu: &mut Cpu, mut callback: F) {
  for _ in 0..200 {
    callback(&mut cpu);

    let opcode = cpu.fetch_at_pc();
    cpu.pc += 1;
    let old_pc = cpu.pc;

    let inst = &INSTRUCTIONS[opcode as usize];
    let operand = get_operand_with_addressing(&mut cpu, &inst);

    let opname = inst.name.as_str();
    let (_, inst_fn) = INSTR_TO_FN
      .get_key_value(opname).expect("Op should be in map");
    
    inst_fn(&mut cpu, &operand);

    if cpu.pc == old_pc {
      cpu.pc += inst.bytes as u16 - 1;
    }
    cpu.cycles += inst.cycles;
  }
}

enum OperandSrc {
  Acc, Addr(u16), None
}
pub struct Operand {
  src: OperandSrc,
  val: u8,
}
pub enum InstrDst {
  None, Acc(u8), X(u8), Y(u8), Mem(u16, u8) 
}

pub type InstrFn = fn(&mut Cpu, &Operand);

pub fn get_operand_with_addressing(cpu: &mut Cpu, inst: &Instruction) -> Operand {
  let mode = inst.addressing;
  use AddressingMode::*;
  use OperandSrc::*;

  let res = match mode {
    Implicit => Operand {src: None, val: 0},
    Accumulator => Operand {src: Acc, val: cpu.a},
    Immediate | Relative => Operand {src: None, val: cpu.fetch_at_pc()},
    ZeroPage => {
      let zero_addr = cpu.fetch_at_pc() as u16;
      Operand { src: Addr(zero_addr), val: cpu.mem_fetch(zero_addr) }
    }
    ZeroPageX => {
      let zero_addr = (cpu.fetch_at_pc().wrapping_add(cpu.x)) as u16;
      Operand { src: Addr(zero_addr), val: cpu.mem_fetch(zero_addr) }
    }
    IndirectX => {
      let zero_addr = (cpu.fetch_at_pc().wrapping_add(cpu.x)) as u16;
      let addr_effective = cpu.mem_fetch16(zero_addr);
      Operand { src: Addr(addr_effective), val: cpu.mem_fetch(addr_effective) }
    }
    ZeroPageY => {
      let zero_addr = (cpu.fetch_at_pc().wrapping_add(cpu.y)) as u16;
      Operand { src: Addr(zero_addr), val: cpu.mem_fetch(zero_addr) }
    }
    IndirectY => {
      let zero_addr = cpu.fetch_at_pc() as u16;
      let addr_effective = cpu.mem_fetch16(zero_addr)
      .wrapping_add(cpu.y as u16).wrapping_add(cpu.carry() as u16);
      Operand { src: Addr(addr_effective), val: cpu.mem_fetch(addr_effective) }
    }
    Absolute => {
      let addr = cpu.fetch16_at_pc();
      Operand { src: Addr(addr), val: cpu.mem_fetch(addr) }
    }
    //TODO: should be done wrapping add?
    AbsoluteX => { 
      let addr = cpu.fetch16_at_pc() + cpu.x as u16 + cpu.carry() as u16;
      // page crossing check
      if addr & 0xFF00 != cpu.pc & 0xFF00 {
        cpu.cycles = cpu.cycles.wrapping_add(1);
      }

      Operand { src: Addr(addr), val: cpu.mem_fetch(addr) }
    }
    //TODO: should be done wrapping add?
    AbsoluteY => {
      let addr = cpu.fetch16_at_pc() + cpu.y as u16 + cpu.carry() as u16;
      // page crossing check
      if addr & 0xFF00 != cpu.pc & 0xFF00 {
        cpu.cycles = cpu.cycles.wrapping_add(1);
      }
      
      Operand { src: Addr(addr), val: cpu.mem_fetch(addr) }
    }
    Indirect => {
      let addr = cpu.fetch16_at_pc();
      let addr_effective = cpu.mem_fetch16(addr);
      Operand { src: Addr(addr_effective), val: 0 }
    }
  };

  res
}

pub fn set_instr_result(cpu: &mut Cpu, dst: InstrDst) {
  match dst {
    InstrDst::None => {}
    InstrDst::Acc(res) => cpu.a = res,
    InstrDst::X(res) => cpu.x = res,
    InstrDst::Y(res) => cpu.y = res,
    InstrDst::Mem(addr, res) => cpu.mem_set(addr, res),
  }
}

pub fn load (cpu: &mut Cpu, operand: &Operand, dst: InstrDst) {
  cpu.set_czn(operand.val as u16);
  set_instr_result(cpu, dst);
}

pub fn lda(cpu: &mut Cpu, operand: &Operand) {
  load(cpu, operand, InstrDst::Acc(operand.val))
}

pub fn ldx(cpu: &mut Cpu, operand: &Operand) {
  load(cpu, operand, InstrDst::X(operand.val))
}

pub fn ldy(cpu: &mut Cpu, operand: &Operand) {
  load(cpu, operand, InstrDst::Y(operand.val))
}

pub fn store(cpu: &mut Cpu, operand: &Operand, val: u8) {
  if let OperandSrc::Addr(src) = operand.src {
    set_instr_result(cpu, InstrDst::Mem(src, val))
  } else { unreachable!() }  
}

pub fn sta(cpu: &mut Cpu, operand: &Operand) {
  store(cpu, operand, cpu.a)
}

pub fn stx(cpu: &mut Cpu, operand: &Operand) {
  store(cpu, operand, cpu.x)
}

pub fn sty(cpu: &mut Cpu, operand: &Operand) {
  store(cpu, operand, cpu.y)
}

pub fn transfer(cpu: &mut Cpu, src: u8, dst: InstrDst) {
  cpu.set_czn(src as u16);
  set_instr_result(cpu, dst);
}

pub fn tax(cpu: &mut Cpu, _: &Operand) {
  transfer(cpu, cpu.a, InstrDst::X(cpu.a))
}

pub fn tay(cpu: &mut Cpu, _: &Operand) {
  transfer(cpu, cpu.a, InstrDst::Y(cpu.a))
}

pub fn tsx(cpu: &mut Cpu, _: &Operand) {
  let res = cpu.stack_pop(); 
  transfer(cpu, res, InstrDst::X(res))
}

pub fn txa(cpu: &mut Cpu, _: &Operand) {
  transfer(cpu, cpu.x, InstrDst::Acc(cpu.x))
}

pub fn txs(cpu: &mut Cpu, _: &Operand) {
  cpu.stack_push(cpu.x);
}

pub fn tya(cpu: &mut Cpu, _: &Operand) {
  transfer(cpu, cpu.y, InstrDst::Acc(cpu.y))
}

pub fn pha(cpu: &mut Cpu, _: &Operand) {
  cpu.stack_push(cpu.a);
}

pub fn php(cpu: &mut Cpu, _: &Operand) {
  cpu.stack_push(cpu.p.bits());
}

pub fn pla(cpu: &mut Cpu, _: &Operand) {
  let res = cpu.stack_pop();
  cpu.set_czn(res as u16);
  cpu.a = res;
}

pub fn plp(cpu: &mut Cpu, _: &Operand) {
  let res = cpu.stack_pop();
  cpu.p = Status::from_bits_retain(res)
}

pub fn logic(cpu: &mut Cpu, res: u8) {
  cpu.set_czn(res as u16);
}

pub fn and(cpu: &mut Cpu, operand: &Operand) {
  logic(cpu, cpu.a & operand.val)
}

pub fn eor(cpu: &mut Cpu, operand: &Operand) {
  logic(cpu, cpu.a ^ operand.val)
}

pub fn ora(cpu: &mut Cpu, operand: &Operand) {
  logic(cpu, cpu.a | operand.val)
}

pub fn bit(cpu: &mut Cpu, operand: &Operand) {
  let res = cpu.a & operand.val;
  if res == 0 { cpu.p.insert(Status::zero); }
  if res & 0b0100_0000 != 0 { cpu.p.insert(Status::overflow); }
  if res & 0b1000_0000 != 0 { cpu.p.insert(Status::negative); }
}

// TODO: check if correct
pub fn adc(cpu: &mut Cpu, operand: &Operand) {
  let res = cpu.a as u16 + operand.val as u16 + cpu.carry() as u16;
  cpu.set_overflow(cpu.a as u16, operand.val as u16, res);
  cpu.set_czn(res);
  cpu.a = res as u8;
}

pub fn sbc(cpu: &mut Cpu, operand: &Operand) {
  let res = cpu.a as u16 +
                !operand.val as u16 + 
                (1 - cpu.carry()) as u16;
  cpu.set_overflow(cpu.a as u16, !operand.val as u16, res);
  cpu.set_czn(res);
  cpu.a = res as u8;
}

pub fn compare(cpu: &mut Cpu, a: u8, b: u8) {
  let res = a.wrapping_sub(b);
  cpu.set_czn(res as u16);
  cpu.p.set(Status::carry, a >= b);
}

pub fn cmp(cpu: &mut Cpu, operand: &Operand) {
  compare(cpu, cpu.a, operand.val)
}

pub fn cpx(cpu: &mut Cpu, operand: &Operand) {
  compare(cpu, cpu.x, operand.val)
}

pub fn cpy(cpu: &mut Cpu, operand: &Operand) {
  compare(cpu, cpu.y, operand.val)
}

pub fn increase(cpu: &mut Cpu, val: u8, f: fn(u8, u8) -> u8) -> u8 {
  let res = f(val, 1);
  cpu.set_czn(res as u16);
  res
}

pub fn inc(cpu: &mut Cpu, operand: &Operand) {
  if let OperandSrc::Addr(src) = operand.src {
    let res = increase(cpu, operand.val, u8::wrapping_add);
    cpu.mem_set(src, res);
  } else { unreachable!() }
}

pub fn inx(cpu: &mut Cpu, _: &Operand) {
  cpu.x = increase(cpu, cpu.x, u8::wrapping_add);
}

pub fn iny(cpu: &mut Cpu, _: &Operand) {
  cpu.y = increase(cpu, cpu.y, u8::wrapping_add)
}

pub fn dec(cpu: &mut Cpu, operand: &Operand) {
  if let OperandSrc::Addr(src) = operand.src {
    let res = increase(cpu, operand.val, u8::wrapping_sub);
    cpu.mem_set(src, res);
  } else { unreachable!() }
}

pub fn dex(cpu: &mut Cpu, _: &Operand) {
  cpu.x = increase(cpu, cpu.x, u8::wrapping_sub);
}

pub fn dey(cpu: &mut Cpu, _: &Operand) {
  cpu.y = increase(cpu, cpu.y, u8::wrapping_sub);
}

//TODO: factor out shifts
pub fn asl(cpu: &mut Cpu, operand: &Operand) {
  let res = (operand.val as u16) << 1;
  cpu.set_czn(res);

  match operand.src {
    OperandSrc::Acc => cpu.a = res as u8,
    OperandSrc::Addr(src) => cpu.mem_set(src, res as u8),
    OperandSrc::None => { unreachable!() }
  }
}
pub fn lsr(cpu: &mut Cpu, operand: &Operand) {
  let first = operand.val & 1 != 0;
  let res = operand.val >> 1;
  cpu.p.set(Status::carry, first);
  cpu.p.set(Status::zero, res != 0);
  cpu.p.set(Status::negative, res & 0b1000_0000 != 0);

  match operand.src {
    OperandSrc::Acc => cpu.a = res as u8,
    OperandSrc::Addr(src) => cpu.mem_set(src, res as u8),
    OperandSrc::None => { unreachable!() }
  }
}

//TODO: factor our rotations
pub fn rol(cpu: &mut Cpu, operand: &Operand) {
  let carry = operand.val & 0b1000_0000 != 0;
  let res = operand.val.rotate_left(1) & cpu.carry();
  cpu.set_czn(res as u16);
  cpu.p.set(Status::carry, carry);

  match operand.src {
    OperandSrc::Acc => cpu.a = res as u8,
    OperandSrc::Addr(src) => cpu.mem_set(src, res as u8),
    OperandSrc::None => { unreachable!() }
  }
} 
pub fn ror(cpu: &mut Cpu, operand: &Operand) {
  let carry = operand.val & 1 != 0;
  let res = operand.val.rotate_left(1) & cpu.carry() << 7;
  cpu.set_czn(res as u16);
  cpu.p.set(Status::carry, carry);

  match operand.src {
    OperandSrc::Acc => cpu.a = res as u8,
    OperandSrc::Addr(src) => cpu.mem_set(src, res as u8),
    OperandSrc::None => { unreachable!() }
  }
}

pub fn jmp(cpu: &mut Cpu, operand: &Operand) {
  if let OperandSrc::Addr(src) = operand.src {
    cpu.pc = src;
  } else { unreachable!() }
}

pub fn jsr(cpu: &mut Cpu, operand: &Operand) {
  cpu.stack_push16(cpu.pc);
  jmp(cpu, operand);
}

pub fn rts(cpu: &mut Cpu, _: &Operand) {
  cpu.pc = cpu.stack_pop16();
}

pub fn branch(cpu: &mut Cpu, offset: u8, cond: bool) {
  if cond {
    let offset = offset as i8;
    let new_pc = cpu.pc.wrapping_add_signed(offset as i16);

    // page boundary cross check
    if cpu.pc & 0xFF00 != new_pc & 0xFF00 {
      cpu.cycles = cpu.cycles.wrapping_add(1);
    }

    cpu.pc = new_pc;
  }
}

pub fn bcc(cpu: &mut Cpu, operand: &Operand) {
  branch(cpu, operand.val, cpu.carry() == 0)
}

pub fn bcs(cpu: &mut Cpu, operand: &Operand) {
  branch(cpu, operand.val, cpu.carry() == 1)
}

pub fn beq(cpu: &mut Cpu, operand: &Operand) {
  branch(cpu, operand.val, cpu.p.contains(Status::zero))
}

pub fn bne(cpu: &mut Cpu, operand: &Operand) {
  branch(cpu, operand.val, !cpu.p.contains(Status::zero))
}

pub fn bpl(cpu: &mut Cpu, operand: &Operand) {
  branch(cpu, operand.val, !cpu.p.contains(Status::negative))
}

pub fn bvc(cpu: &mut Cpu, operand: &Operand) {
  branch(cpu, operand.val, !cpu.p.contains(Status::overflow))
}

pub fn bvs(cpu: &mut Cpu, operand: &Operand) {
  branch(cpu, operand.val, cpu.p.contains(Status::overflow))
}

pub fn clear_stat(cpu: &mut Cpu, s: Status) {
  cpu.p.remove(s);
}

pub fn clc(cpu: &mut Cpu, _: &Operand) {
  clear_stat(cpu, Status::carry)
}

pub fn cld(cpu: &mut Cpu, _: &Operand) {
  clear_stat(cpu, Status::decimal)
}

pub fn cli(cpu: &mut Cpu, _: &Operand) {
  clear_stat(cpu, Status::interrupt)
}

pub fn clv(cpu: &mut Cpu, _: &Operand) {
  clear_stat(cpu, Status::overflow)
}

pub fn set_stat(cpu: &mut Cpu, s: Status) {
  cpu.p.insert(s);
}

pub fn sec(cpu: &mut Cpu, _: &Operand) {
  set_stat(cpu, Status::carry)
}

pub fn sed(cpu: &mut Cpu, _: &Operand) {
  set_stat(cpu, Status::decimal)
}

pub fn sei(cpu: &mut Cpu, _: &Operand) {
  set_stat(cpu, Status::interrupt)
}

// TODO
pub fn brk(_cpu: &mut Cpu, _: &Operand) {
  todo!()
}

pub fn nop(_cpu: &mut Cpu, _: &Operand) {
  todo!()
}

pub fn rti(_cpu: &mut Cpu, _: &Operand) {
  todo!()
} 

#[cfg(test)]
mod tests {
use super::*;

  #[test]
  fn signed_test() {
    let unsigned = 130u8;
    let signed = unsigned as i8;

    assert_eq!(signed as i16, -126);
  }

  #[test]
  fn cpu_test() {
    let mut cpu = Cpu::new();
    let codes = vec![0x69, 0x01, 0x69, 0x05];
    cpu.write_data(0, &codes);

    interpret(&mut cpu);

    assert_eq!(cpu.a, 6);
  }
}