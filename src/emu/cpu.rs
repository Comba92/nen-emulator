#![allow(dead_code)]
use std::{cell::RefCell, fmt::Debug, rc::Rc};

use bitflags::bitflags;
use super::{cart::Cart, instr::{AddressingMode, Instruction, INSTRUCTIONS, INSTR_TO_FN}, mem::Mem};

bitflags! {
  #[derive(Default, Debug, Clone, Copy)]
  pub struct StatusReg: u8 {
    const carry     = 0b0000_0001;
    const zero      = 0b0000_0010;
    const interrupt = 0b0000_0100;
    const decimal   = 0b0000_1000;
    const overflow  = 0b0100_0000;
    const negative  = 0b1000_0000;
  }
}

const STACK_START: usize = 0x0100;
const STACK_RESET: usize = 0x24;
const IP_RESET: usize = 0xFFFC;
const MEM_SIZE: usize = 0x10000;

#[derive(Clone)]
pub struct Cpu {
  pub ip: u16,
  pub sp: u8,
  pub sr: StatusReg,
  pub a: u8,
  pub x: u8,
  pub y: u8,
  pub cycles: usize,
  pub mem: Rc<RefCell<[u8; MEM_SIZE]>>,
}

impl Debug for Cpu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cpu").field("ip", &self.ip).field("sp", &self.sp).field("sr", &self.sr).field("a", &self.a).field("x", &self.x).field("y", &self.y).field("cycles", &self.cycles).finish()
    }
}

impl Cpu {
  pub fn new() -> Self {    
    Self {
      ip: IP_RESET as u16,
      sp: STACK_RESET as u8,
      a: 0, x: 0, y: 0,
      //TODO: find starting value
      sr: StatusReg::default(),
      cycles: 7,
      mem: Rc::new(RefCell::new([0; MEM_SIZE])),
    }
  }

  pub fn set_czn(&mut self, res: u16) {
    if res > u8::MAX as u16 { self.sr.insert(StatusReg::carry); } 
    if res == 0 { self.sr.insert(StatusReg::zero); }
    if res & 0b1000_0000 == 1 { self.sr.insert(StatusReg::negative); }
  }

  // https://forums.nesdev.org/viewtopic.php?t=6331
  fn set_overflow(&mut self, a: u16, v: u16, s: u16) {
    let overflow = (a ^ s) & (v ^ s) & 0b1000_0000 != 0;
    if overflow { self.sr.insert(StatusReg::overflow); }
  }

  pub fn carry(&self) -> u8 {
    self.sr.contains(StatusReg::carry).into()
  }

  pub fn write_data(&mut self, addr: usize, data: &[u8]) {
    let mut mem = self.mem.borrow_mut();
    mem[addr..addr+data.len()].copy_from_slice(data);
  }

  pub fn mem_fetch(&self, addr: u16) -> u8 {
    self.mem.borrow()[addr as usize]
  }

  pub fn mem_fetch16(&self, addr: u16) -> u16 {
    u16::from_le_bytes([self.mem_fetch(addr), self.mem_fetch(addr+1)])
  }

  pub fn mem_set(&mut self, addr: u16, val: u8) {
    self.mem.borrow_mut()[addr as usize] = val;
  }

  pub fn mem_set16(&mut self, addr: u16, val: u16) {
    let [first, second] = val.to_le_bytes();
    self.mem.borrow_mut()[addr as usize] = first;
    self.mem.borrow_mut()[(addr + 1) as usize] = second;
  }

  pub fn fetch_at_ip(&self) -> u8 {
    self.mem_fetch(self.ip)
  }

  pub fn fetch16_at_ip(&self) -> u16 {
    self.mem_fetch16(self.ip)
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

    let opcode = cpu.fetch_at_ip();
    cpu.ip += 1;
    let old_ip = cpu.ip;

    let inst = &INSTRUCTIONS[opcode as usize];
    let operand = get_operand_with_addressing(&mut cpu, &inst);

    let opname = inst.name.as_str();
    let (_, inst_fn) = INSTR_TO_FN
      .get_key_value(opname).expect("Op should be in map");
    
    let dst = inst_fn(&mut cpu, &operand);
    set_instr_result(&mut cpu, dst);

    if cpu.ip == old_ip {
      cpu.ip += inst.bytes as u16 - 1;
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
enum InstrDst {
  None, Acc(u8), X(u8), Y(u8), Mem(u16, u8) 
}

pub type InstrFn = fn(&mut Cpu, &Operand) -> InstrDst;

pub fn get_operand_with_addressing(cpu: &mut Cpu, inst: &Instruction) -> Operand {
  let mode = inst.addressing;
  use AddressingMode::*;
  use OperandSrc::*;

  match mode {
    Implicit => Operand {src: None, val: 0},
    Accumulator => Operand {src: Acc, val: cpu.a},
    Immediate | Relative => Operand {src: None, val: cpu.fetch_at_ip()},
    ZeroPage => {
      let zero_addr = cpu.fetch_at_ip() as u16;
      Operand { src: Addr(zero_addr), val: cpu.mem_fetch(zero_addr) }
    }
    ZeroPageX => {
      let zero_addr = (cpu.fetch_at_ip().wrapping_add(cpu.x)) as u16;
      Operand { src: Addr(zero_addr), val: cpu.mem_fetch(zero_addr) }
    }
    IndirectX => {
      let zero_addr = (cpu.fetch_at_ip().wrapping_add(cpu.x)) as u16;
      let lookup = cpu.mem_fetch16(zero_addr);
      Operand { src: Addr(lookup), val: cpu.mem_fetch(lookup) }
    }
    ZeroPageY => {
      let zero_addr = (cpu.fetch_at_ip().wrapping_add(cpu.y)) as u16;
      Operand { src: Addr(zero_addr), val: cpu.mem_fetch(zero_addr) }
    }
    IndirectY => {
      let zero_addr = cpu.fetch_at_ip() as u16;
      let lookup = cpu.mem_fetch16(zero_addr).wrapping_add(cpu.y as u16);
      Operand { src: Addr(lookup), val: cpu.mem_fetch(lookup) }
    }
    Absolute => {
      let addr = cpu.fetch16_at_ip();
      Operand { src: Addr(addr), val: cpu.mem_fetch(addr) }
    }
    //TODO: should be done wrapping add?
    //TODO: check for page boudary crossing
    AbsoluteX => { 
      let addr = cpu.fetch16_at_ip() + cpu.x as u16;
      Operand { src: Addr(addr), val: cpu.mem_fetch(addr) }
    }
    //TODO: should be done wrapping add?
    //TODO: check for page boudary crossing
    AbsoluteY => {
      let addr = cpu.fetch16_at_ip() + cpu.y as u16;
      Operand { src: Addr(addr), val: cpu.mem_fetch(addr) }
    }
    Indirect => {
      let addr = cpu.fetch16_at_ip();
      let lookup = cpu.mem_fetch16(addr);
      Operand { src: Addr(lookup), val: 0 }
    }
  }
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

pub fn load (cpu: &mut Cpu, operand: &Operand, dst: InstrDst) -> InstrDst {
  cpu.set_czn(operand.val as u16);
  dst
}

pub fn lda(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  load(cpu, operand, InstrDst::Acc(operand.val))
}

pub fn ldx(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  load(cpu, operand, InstrDst::X(operand.val))
}

pub fn ldy(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  load(cpu, operand, InstrDst::Y(operand.val))
}

pub fn store(operand: &Operand, val: u8) -> InstrDst {
  if let OperandSrc::Addr(src) = operand.src {
    InstrDst::Mem(src, val)
  } else { unreachable!() }  
}

pub fn sta(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  store(operand, cpu.a)
}

pub fn stx(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  store(operand, cpu.x)
}

pub fn sty(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  store(operand, cpu.y)
}

pub fn transfer(cpu: &mut Cpu, src: u8, dst: InstrDst) -> InstrDst {
  cpu.set_czn(src as u16);
  dst
}

pub fn tax(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  transfer(cpu, cpu.a, InstrDst::X(cpu.a))
}

pub fn tay(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  transfer(cpu, cpu.a, InstrDst::Y(cpu.a))
}

pub fn tsx(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  let res = cpu.stack_pop(); 
  transfer(cpu, res, InstrDst::X(res))
}

pub fn txa(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  transfer(cpu, cpu.x, InstrDst::Acc(cpu.x))
}

pub fn txs(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  cpu.stack_push(cpu.x);
  InstrDst::None
}

pub fn tya(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  transfer(cpu, cpu.y, InstrDst::Acc(cpu.y))
}

pub fn pha(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  cpu.stack_push(cpu.a);
  InstrDst::None
}

pub fn php(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  cpu.stack_push(cpu.sr.bits());
  InstrDst::None
}

pub fn pla(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  let res = cpu.stack_pop();
  cpu.set_czn(res as u16);
  InstrDst::Acc(res)
}

pub fn plp(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  let res = cpu.stack_pop();
  cpu.sr = StatusReg::from_bits(res).expect("No unused bits should be set");
  InstrDst::None
}

pub fn logic(cpu: &mut Cpu, res: u8) -> InstrDst {
  cpu.set_czn(res as u16);
  InstrDst::Acc(res)
}

pub fn and(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  logic(cpu, cpu.a & operand.val)
}

pub fn eor(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  logic(cpu, cpu.a ^ operand.val)
}

pub fn ora(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  logic(cpu, cpu.a | operand.val)
}

pub fn bit(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  let res = cpu.a & operand.val;
  if res == 0 { cpu.sr.insert(StatusReg::zero); }
  if res & 0b0100_0000 != 0 { cpu.sr.insert(StatusReg::overflow); }
  if res & 0b1000_0000 != 0 { cpu.sr.insert(StatusReg::negative); }
  InstrDst::None
}

// TODO: check if correct
pub fn adc(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  let res = cpu.a as u16 + operand.val as u16 + cpu.carry() as u16;
  cpu.set_overflow(cpu.a as u16, operand.val as u16, res);
  cpu.set_czn(res);

  InstrDst::Acc(res as u8)
}

pub fn sbc(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  let res = cpu.a as u16 +
                !operand.val as u16 + 
                (1 - cpu.carry()) as u16;
  cpu.set_overflow(cpu.a as u16, !operand.val as u16, res);
  cpu.set_czn(res);

  InstrDst::Acc(res as u8)
}

pub fn compare(cpu: &mut Cpu, a: u8, b: u8) -> InstrDst {
  let res = a.wrapping_sub(b);
  cpu.set_czn(res as u16);
  cpu.sr.set(StatusReg::carry, a >= b);
  InstrDst::None
}

pub fn cmp(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  compare(cpu, cpu.a, operand.val)
}

pub fn cpx(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  compare(cpu, cpu.x, operand.val)
}

pub fn cpy(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  compare(cpu, cpu.y, operand.val)
}

pub fn increase(cpu: &mut Cpu, val: u8, f: fn(u8, u8) -> u8) -> u8 {
  let res = f(val, 1);
  cpu.set_czn(res as u16);
  res
}

pub fn inc(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  if let OperandSrc::Addr(src) = operand.src {
    InstrDst::Mem(src, increase(cpu, operand.val, u8::wrapping_add))
  } else { unreachable!() }
}

pub fn inx(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  InstrDst::X(increase(cpu, cpu.x, u8::wrapping_add))
}

pub fn iny(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  InstrDst::Y(increase(cpu, cpu.y, u8::wrapping_add))
}

pub fn dec(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  if let OperandSrc::Addr(src) = operand.src {
    InstrDst::Mem(src, increase(cpu, operand.val, u8::wrapping_sub))
  } else { unreachable!() }
}

pub fn dex(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  InstrDst::X(increase(cpu, cpu.x, u8::wrapping_sub))
}

pub fn dey(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  InstrDst::Y(increase(cpu, cpu.y, u8::wrapping_sub))
}

pub fn asl(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  let res = (operand.val as u16) << 1;
  cpu.set_czn(res);

  match operand.src {
    OperandSrc::Acc => InstrDst::Acc(res as u8),
    OperandSrc::Addr(src) => InstrDst::Mem(src, res as u8),
    OperandSrc::None => { unreachable!() }
  }
}
pub fn lsr(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  let first = operand.val & 1 != 0;
  let res = operand.val >> 1;
  cpu.sr.set(StatusReg::carry, first);
  cpu.sr.set(StatusReg::zero, res != 0);
  cpu.sr.set(StatusReg::negative, res & 0b1000_0000 != 0);

  match operand.src {
    OperandSrc::Acc => InstrDst::Acc(res as u8),
    OperandSrc::Addr(src) => InstrDst::Mem(src, res as u8),
    OperandSrc::None => { unreachable!() }
  }
}

pub fn rol(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  let carry = operand.val & 0b1000_0000 != 0;
  let res = operand.val.rotate_left(1) & cpu.carry();
  cpu.set_czn(res as u16);
  cpu.sr.set(StatusReg::carry, carry);

  match operand.src {
    OperandSrc::Acc => InstrDst::Acc(res as u8),
    OperandSrc::Addr(src) => InstrDst::Mem(src, res as u8),
    OperandSrc::None => { unreachable!() }
  }
} 
pub fn ror(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  let carry = operand.val & 1 != 0;
  let res = operand.val.rotate_left(1) & cpu.carry() << 7;
  cpu.set_czn(res as u16);
  cpu.sr.set(StatusReg::carry, carry);

  match operand.src {
    OperandSrc::Acc => InstrDst::Acc(res as u8),
    OperandSrc::Addr(src) => InstrDst::Mem(src, res as u8),
    OperandSrc::None => { unreachable!() }
  }
}

pub fn jmp(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  if let OperandSrc::Addr(src) = operand.src {
    cpu.ip = src;
    InstrDst::None
  } else { unreachable!() }
}

pub fn jsr(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  cpu.stack_push16(cpu.ip);
  jmp(cpu, operand)
}

pub fn rts(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  cpu.ip = cpu.stack_pop16();
  InstrDst::None
}

pub fn branch(cpu: &mut Cpu, offset: u8, cond: bool) -> InstrDst {
  if cond {
    let offset = offset as i8;
    cpu.ip = cpu.ip.wrapping_add_signed(offset as i16);
  }

  InstrDst::None
}

pub fn bcc(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  branch(cpu, operand.val, cpu.carry() == 0)
}

pub fn bcs(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  branch(cpu, operand.val, cpu.carry() == 1)
}

pub fn beq(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  branch(cpu, operand.val, cpu.sr.contains(StatusReg::zero))
}

pub fn bne(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  branch(cpu, operand.val, !cpu.sr.contains(StatusReg::zero))
}

pub fn bpl(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  branch(cpu, operand.val, !cpu.sr.contains(StatusReg::negative))
}

pub fn bvc(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  branch(cpu, operand.val, !cpu.sr.contains(StatusReg::overflow))
}

pub fn bvs(cpu: &mut Cpu, operand: &Operand) -> InstrDst {
  branch(cpu, operand.val, cpu.sr.contains(StatusReg::overflow))
}

pub fn clear_stat(cpu: &mut Cpu, s: StatusReg) -> InstrDst {
  cpu.sr.remove(s);
  InstrDst::None
}

pub fn clc(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  clear_stat(cpu, StatusReg::carry)
}

pub fn cld(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  clear_stat(cpu, StatusReg::decimal)
}

pub fn cli(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  clear_stat(cpu, StatusReg::interrupt)
}

pub fn clv(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  clear_stat(cpu, StatusReg::overflow)
}

pub fn set_stat(cpu: &mut Cpu, s: StatusReg) -> InstrDst {
  cpu.sr.insert(s);
  InstrDst::None
}

pub fn sec(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  set_stat(cpu, StatusReg::carry)
}

pub fn sed(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  set_stat(cpu, StatusReg::decimal)
}

pub fn sei(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  set_stat(cpu, StatusReg::interrupt)
}

// TODO
pub fn brk(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  todo!()
}

pub fn nop(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  todo!()
}

pub fn rti(cpu: &mut Cpu, _: &Operand) -> InstrDst {
  todo!()
} 

#[cfg(test)]
mod tests {
use super::*;

  fn write_codes_to_ram(cpu: &mut Cpu, codes: &Vec<u8>) {
    let mut mem = cpu.mem.borrow_mut();
    let (first, _) = mem.split_at_mut(codes.len());
    first.copy_from_slice(codes.as_slice());
  }

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