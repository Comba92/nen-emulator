#![allow(dead_code)]
use std::{cell::RefCell, collections::HashMap, rc::Rc, sync::LazyLock};

use bitflags::bitflags;
use super::instructions::{AddressingMode, Instruction, INSTRUCTIONS};

bitflags! {
  #[derive(Default, Debug, Clone, Copy)]
  struct StatusReg: u8 {
    const carry     = 0b0000_0001;
    const zero      = 0b0000_0010;
    const interrupt = 0b0000_0100;
    const decimal   = 0b0000_1000;
    const overflow  = 0b0100_0000;
    const negative  = 0b1000_0000;
  }
}

const STACK_START: usize = 0x0100;

#[derive(Debug, Clone)]
pub struct Cpu {
  ip: u16,
  sp: u8,
  sr: StatusReg,
  a: u8,
  x: u8,
  y: u8,
  cycles: usize,
  mem: Rc<RefCell<[u8; 0xFFFF]>>,
}
impl Cpu {
  pub fn new() -> Self {
    Self {
      ip: 0,
      sp: STACK_START as u8,
      a: 0, x: 0, y: 0, sr: StatusReg::default(),
      cycles: 0,
      mem: Rc::new(RefCell::new([0; 0xFFFF])),
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
    self.mem_set(self.sp as u16, val);
    self.sp -= 1;
  }

  pub fn stack_push16(&mut self, val: u16) {
    self.mem_set16(self.sp as u16, val);
    self.sp -= 2;
  }

  pub fn stack_pop(&mut self) -> u8 {
    self.sp -= 1;
    self.mem_fetch(self.sp as u16)
  }

  pub fn stack_pop16(&mut self) -> u16 {
    self.sp -= 2;
    self.mem_fetch16(self.sp as u16)
  }
}

type InstructionFn = fn(Cpu, Operand) -> Cpu;
static OPCODES_MAP: LazyLock<HashMap<&'static str, InstructionFn>> = LazyLock::new(|| {
  let mut map: HashMap<&'static str, InstructionFn> = HashMap::new();
  
  map.insert("BRK", brk);
  map.insert("ORA", ora);
  map.insert("NOP", nop);
  map.insert("ASL", asl);
  map.insert("PHP", php);
  map.insert("BPL", bpl);
  map.insert("CLC", clc);
  map.insert("JSR", jsr);
  map.insert("AND", and);
  map.insert("BIT", bit);
  map.insert("ROL", rol);
  map.insert("PLP", plp);
  map.insert("SEC", sec);
  map.insert("RTI", rti);
  map.insert("EOR", eor);
  map.insert("LSR", lsr);
  map.insert("PHA", pha);
  map.insert("JMP", jmp);
  map.insert("BVC", bvc);
  map.insert("CLI", cli);
  map.insert("RTS", rts);
  map.insert("ADC", adc);
  map.insert("ROR", ror);
  map.insert("PLA", pla);
  map.insert("BVS", bvs);
  map.insert("SEI", sei);
  map.insert("STA", sta);
  map.insert("STY", sty);
  map.insert("STX", stx);
  map.insert("DEY", dey);
  map.insert("TXA", txa);
  map.insert("BCC", bcc);
  map.insert("TYA", tya);
  map.insert("TXS", txs);
  map.insert("LDY", ldy);
  map.insert("LDA", lda);
  map.insert("LDX", ldx);
  map.insert("TAY", tay);
  map.insert("TAX", tax);
  map.insert("BCS", bcs);
  map.insert("CLV", clv);
  map.insert("TSX", tsx);
  map.insert("CPY", cpy);
  map.insert("CMP", cmp);
  map.insert("DEC", dec);
  map.insert("INY", iny);
  map.insert("DEX", dex);
  map.insert("BNE", bne);
  map.insert("CLD", cld);
  map.insert("CPX", cpx);
  map.insert("SBC", sbc);
  map.insert("INC", inc);
  map.insert("INX", inx);
  map.insert("BEQ", beq);
  map.insert("SED", sed);

  map
});

pub fn interpret(cpu: &mut Cpu) {
  loop {
    let opcode = cpu.fetch_at_ip();
    cpu.ip += 1;

    let inst = &INSTRUCTIONS[opcode as usize];
    let operand = get_operand_with_addressing(&cpu, &inst);

    if opcode == 0 { break; }

    let opname = inst.name.as_str();
    let (_, inst_fn) = OPCODES_MAP
      .get_key_value(opname).expect("Op should be in map");
    
    cpu.ip += inst.bytes as u16 - 1;
    *cpu = inst_fn(cpu.clone(), operand);

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

pub fn get_operand_with_addressing(cpu: &Cpu, inst: &Instruction) -> Operand {
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

pub fn lda(mut cpu: Cpu, operand: Operand) -> Cpu {
  cpu.set_czn(operand.val as u16);
  cpu.a = operand.val;
  cpu
}

pub fn ldx(mut cpu: Cpu, operand: Operand) -> Cpu {
  cpu.set_czn(operand.val as u16);
  cpu.x = operand.val;
  cpu
}

pub fn ldy(mut cpu: Cpu, operand: Operand) -> Cpu {
  cpu.set_czn(operand.val as u16);
  cpu.y = operand.val;
  cpu
}

pub fn sta(mut cpu: Cpu, operand: Operand) -> Cpu {
  if let OperandSrc::Addr(src) = operand.src {
    cpu.mem_set(src, cpu.a);
    cpu
  } else { unreachable!() }
}

pub fn stx(mut cpu: Cpu, operand: Operand) -> Cpu {
  if let OperandSrc::Addr(src) = operand.src {
    cpu.mem_set(src, cpu.x);
    cpu
  } else { unreachable!() }
}

pub fn sty(mut cpu: Cpu, operand: Operand) -> Cpu {
  if let OperandSrc::Addr(src) = operand.src {
    cpu.mem_set(src, cpu.y);
    cpu
  } else { unreachable!() }
}

pub fn tax(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.set_czn(cpu.a as u16);
  cpu.x = cpu.a;
  cpu
}

pub fn tay(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.set_czn(cpu.a as u16);
  cpu.y = cpu.a;
  cpu
}

pub fn tsx(mut cpu: Cpu, _: Operand) -> Cpu {
  let res = cpu.stack_pop();
  cpu.set_czn(res as u16);
  cpu.x = res;
  cpu
}

pub fn txa(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.set_czn(cpu.x as u16);
  cpu.a = cpu.x;
  cpu
}

pub fn txs(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.stack_push(cpu.x);
  cpu
}

pub fn tya(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.set_czn(cpu.y as u16);
  cpu.a = cpu.y;
  cpu
}

pub fn pha(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.stack_push(cpu.a);
  cpu
}

pub fn php(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.stack_push(cpu.sr.bits());
  cpu
}

pub fn pla(mut cpu: Cpu, _: Operand) -> Cpu {
  let res = cpu.stack_pop();
  cpu.set_czn(res as u16);

  cpu.a = res;
  cpu
}

pub fn plp(mut cpu: Cpu, _: Operand) -> Cpu {
  let res = cpu.stack_pop();
  
  cpu.sr = StatusReg::from_bits(res).expect("No unused bits should be set");
  cpu
}

pub fn and(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.a & operand.val;
  cpu.set_czn(res as u16);

  cpu.a = res;
  cpu
}

pub fn eor(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.a ^ operand.val;
  cpu.set_czn(res as u16);

  cpu.a = res;
  cpu
}

pub fn ora(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.a | operand.val;
  cpu.set_czn(res as u16);

  cpu.a = res;
  cpu
}

pub fn bit(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.a & operand.val;
  if res == 0 { cpu.sr.insert(StatusReg::zero); }
  if res & 0b0100_0000 != 0 { cpu.sr.insert(StatusReg::overflow); }
  if res & 0b1000_0000 != 0 { cpu.sr.insert(StatusReg::negative); }

  cpu
}

// TODO: check if correct
pub fn adc(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.a as u16 + operand.val as u16 + cpu.carry() as u16;
  cpu.set_overflow(cpu.a as u16, operand.val as u16, res);
  cpu.set_czn(res);

  cpu.a = res as u8;
  cpu
}

pub fn sbc(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.a as u16 +
                !operand.val as u16 + 
                (1 - cpu.carry()) as u16;
  cpu.set_overflow(cpu.a as u16, !operand.val as u16, res);
  cpu.set_czn(res);

  cpu.a = res as u8;
  cpu
}

pub fn cmp(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.a.wrapping_sub(operand.val);
  cpu.set_czn(res as u16);
  cpu.sr.set(StatusReg::carry, cpu.a >= operand.val);

  cpu
}

pub fn cpx(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.x.wrapping_sub(operand.val);
  cpu.set_czn(res as u16);
  cpu.sr.set(StatusReg::carry, cpu.x >= operand.val);

  cpu
}

pub fn cpy(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.y.wrapping_sub(operand.val);
  cpu.set_czn(res as u16);
  cpu.sr.set(StatusReg::carry, cpu.y >= operand.val);

  cpu
}

pub fn inc(mut cpu: Cpu, operand: Operand) -> Cpu {
  if let OperandSrc::Addr(src) = operand.src {
    let res = cpu.mem_fetch(src).wrapping_add(1);
    cpu.set_czn(res as u16);

    cpu.mem_set(src, res);
    cpu
  } else { unreachable!() }
}

pub fn inx(mut cpu: Cpu, _: Operand) -> Cpu {
  let res = cpu.x.wrapping_add(1);
  cpu.set_czn(res as u16);
  cpu
}

pub fn iny(mut cpu: Cpu, _: Operand) -> Cpu {
  let res = cpu.y.wrapping_add(1);
  cpu.set_czn(res as u16);
  cpu
}

pub fn dec(mut cpu: Cpu, operand: Operand) -> Cpu {
  if let OperandSrc::Addr(src) = operand.src {
    let res = cpu.mem_fetch(src).wrapping_sub(1);
    cpu.set_czn(res as u16);

    cpu.mem_set(src, res);
    cpu
  } else { unreachable!() }
}

pub fn dex(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.x.wrapping_sub(1);
  cpu.set_czn(res as u16);
  cpu
}

pub fn dey(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = cpu.y.wrapping_sub(1);
  cpu.set_czn(res as u16);
  cpu
}

pub fn asl(mut cpu: Cpu, operand: Operand) -> Cpu {
  let res = (operand.val as u16) << 1;
  cpu.set_czn(res);

  match operand.src {
    OperandSrc::Acc => cpu.a = res as u8,
    OperandSrc::Addr(src) => cpu.mem_set(src, res as u8),
    OperandSrc::None => { unreachable!() }
  };

  cpu
}
pub fn lsr(mut cpu: Cpu, operand: Operand) -> Cpu {
  let first = operand.val & 1 != 0;
  let res = operand.val >> 1;
  cpu.sr.set(StatusReg::carry, first);
  cpu.sr.set(StatusReg::zero, res != 0);
  cpu.sr.set(StatusReg::negative, res & 0b1000_0000 != 0);

  match operand.src {
    OperandSrc::Acc => cpu.a = res as u8,
    OperandSrc::Addr(src) => cpu.mem_set(src, res as u8),
    OperandSrc::None => { unreachable!() }
  };

  cpu
}

pub fn rol(mut cpu: Cpu, operand: Operand) -> Cpu {
  let carry = operand.val & 0b1000_0000 != 0;
  let res = operand.val.rotate_left(1) & cpu.carry();
  cpu.set_czn(res as u16);
  cpu.sr.set(StatusReg::carry, carry);

  match operand.src {
    OperandSrc::Acc => cpu.a = res as u8,
    OperandSrc::Addr(src) => cpu.mem_set(src, res as u8),
    OperandSrc::None => { unreachable!() }
  };

  cpu
} 
pub fn ror(mut cpu: Cpu, operand: Operand) -> Cpu {
  let carry = operand.val & 1 != 0;
  let res = operand.val.rotate_left(1) & cpu.carry() << 7;
  cpu.set_czn(res as u16);
  cpu.sr.set(StatusReg::carry, carry);

  match operand.src {
    OperandSrc::Acc => cpu.a = res as u8,
    OperandSrc::Addr(src) => cpu.mem_set(src, res as u8),
    OperandSrc::None => { unreachable!() }
  };

  cpu
}

pub fn jmp(mut cpu: Cpu, operand: Operand) -> Cpu {
  if let OperandSrc::Addr(src) = operand.src {
    cpu.ip = src;
    cpu
  } else { unreachable!() }
} 

pub fn jsr(mut cpu: Cpu, operand: Operand) -> Cpu {
  cpu.stack_push16(cpu.ip);
  jmp(cpu, operand)
}

pub fn rts(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.ip = cpu.stack_pop16();
  cpu
}

pub fn bcc(mut cpu: Cpu, operand: Operand) -> Cpu {
  if cpu.carry() == 0 {
    let offset = operand.val as i8;
    cpu.ip = cpu.ip.wrapping_add_signed(offset as i16);
  }

  cpu
}

pub fn bcs(mut cpu: Cpu, operand: Operand) -> Cpu {
  if cpu.carry() == 1 {
    let offset = operand.val as i8;
    cpu.ip = cpu.ip.wrapping_add_signed(offset as i16);
  }

  cpu
}

pub fn beq(mut cpu: Cpu, operand: Operand) -> Cpu {
  if cpu.sr.contains(StatusReg::zero) {
    let offset = operand.val as i8;
    cpu.ip = cpu.ip.wrapping_add_signed(offset as i16);
  }

  cpu
}

pub fn bne(mut cpu: Cpu, operand: Operand) -> Cpu {
  if !cpu.sr.contains(StatusReg::zero) {
    let offset = operand.val as i8;
    cpu.ip = cpu.ip.wrapping_add_signed(offset as i16);
  }

  cpu
}

pub fn bpl(mut cpu: Cpu, operand: Operand) -> Cpu {
  if !cpu.sr.contains(StatusReg::negative) {
    let offset = operand.val as i8;
    cpu.ip = cpu.ip.wrapping_add_signed(offset as i16);
  }

  cpu
}

pub fn bvc(mut cpu: Cpu, operand: Operand) -> Cpu {
  if !cpu.sr.contains(StatusReg::overflow) {
    let offset = operand.val as i8;
    cpu.ip = cpu.ip.wrapping_add_signed(offset as i16);
  }

  cpu
}

pub fn bvs(mut cpu: Cpu, operand: Operand) -> Cpu {
  if cpu.sr.contains(StatusReg::overflow) {
    let offset = operand.val as i8;
    cpu.ip = cpu.ip.wrapping_add_signed(offset as i16);
  }

  cpu
}

pub fn clc(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.sr.remove(StatusReg::carry);
  cpu
}

pub fn cld(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.sr.remove(StatusReg::decimal);
  cpu
}

pub fn cli(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.sr.remove(StatusReg::interrupt);
  cpu
}

pub fn clv(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.sr.remove(StatusReg::overflow);
  cpu
}

pub fn sec(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.sr.insert(StatusReg::carry);
  cpu
}

pub fn sed(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.sr.insert(StatusReg::decimal);
  cpu
}

pub fn sei(mut cpu: Cpu, _: Operand) -> Cpu {
  cpu.sr.insert(StatusReg::interrupt);
  cpu
}

pub fn brk(mut cpu: Cpu, _: Operand) -> Cpu {
  todo!()
} 

pub fn nop(cpu: Cpu, _: Operand) -> Cpu {
  cpu
}

pub fn rti(mut cpu: Cpu, _: Operand) -> Cpu {
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
    write_codes_to_ram(&mut cpu, &codes);

    interpret(&mut cpu);

    assert_eq!(cpu.a, 6);
  }
}