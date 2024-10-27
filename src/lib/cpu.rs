use std::{collections::HashMap, sync::LazyLock};

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

#[derive(Debug, Clone, Copy)]
pub struct Cpu {
  ip: u16,
  sp: u8,
  sr: StatusReg,
  a: u8,
  x: u8,
  y: u8,
  cycles: usize,
  mem: [u8; 0xFFFF],
}
impl Cpu {
  pub fn new() -> Self {
    Self {
      ip: 0,
      sp: 0x00FD,
      a: 0, x: 0, y: 0, sr: StatusReg::default(),
      cycles: 0,
      mem: [0; 0xFFFF],
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
    self.mem[addr as usize]
  }

  pub fn mem_fetch16(&self, addr: u16) -> u16 {
    u16::from_le_bytes([self.mem_fetch(addr), self.mem_fetch(addr+1)])
  }

  pub fn mem_set(&mut self, addr: u16, val: u8) {
    self.mem[addr as usize] = val;
  }

  pub fn mem_set16(&mut self, addr: u16, val: u16) {
    let [first, second] = val.to_le_bytes();
    self.mem[addr as usize] = first;
    self.mem[(addr + 1) as usize] = second;
  }

  pub fn fetch_at_ip(&self) -> u8 {
    self.mem_fetch(self.ip)
  }

  pub fn fetch16_at_ip(&self) -> u16 {
    self.mem_fetch16(self.ip)
  }

  pub fn stack_push(&mut self, val: u8) {
    todo!();
  }

  pub fn stack_pop(&self) -> u8 {
    todo!();
  }
}

type InstructionFn = fn(Cpu, u8) -> Cpu;
static OPCODES_MAP: LazyLock<HashMap<&'static str, InstructionFn>> = LazyLock::new(|| {
  let mut map: HashMap<&'static str, InstructionFn> = HashMap::new();

  map.insert("ADC", adc);
  map.insert("NOP", nop);

  map
});

pub fn interpret(cpu: &mut Cpu) {
  loop {
    let opcode = cpu.fetch_at_ip();
    cpu.ip+=1;

    let inst = &INSTRUCTIONS[opcode as usize];
    let operand = get_operand(cpu, &inst);

    if operand == 0 { break; }

    let opname = inst.names[0].as_str();
    let (_, inst_fn) = OPCODES_MAP
      .get_key_value(opname).expect("Op should be in map");
    *cpu = inst_fn(*cpu, operand);

    cpu.ip += get_operand_len(&inst);
    cpu.cycles += inst.cycles;
  }
}


pub fn get_operand(cpu: &Cpu, inst: &Instruction) -> u8 {
  let mode = inst.addressing;
  use AddressingMode::*;
  match mode {
    Implicit => 0,
    Accumulator => cpu.a,
    Immediate | Relative => cpu.fetch_at_ip(),
    ZeroPage => {
      let zero_addr = cpu.fetch_at_ip() as u16;
      cpu.mem_fetch(zero_addr)
    }
    ZeroPageX => {
      let zero_addr = (cpu.fetch_at_ip().wrapping_add(cpu.x)) as u16;
      cpu.mem_fetch(zero_addr)
    }
    IndirectX => {
      let zero_addr = (cpu.fetch_at_ip().wrapping_add(cpu.x)) as u16;
      let lookup = cpu.mem_fetch16(zero_addr);
      cpu.mem_fetch(lookup)
    }
    ZeroPageY => {
      let zero_addr = (cpu.fetch_at_ip().wrapping_add(cpu.y)) as u16;
      cpu.mem_fetch(zero_addr)
    }
    IndirectY => {
      let zero_addr = cpu.fetch_at_ip() as u16;
      let lookup = cpu.mem_fetch16(zero_addr).wrapping_add(cpu.y as u16);
      cpu.mem_fetch(lookup)
    }
    Absolute => {
      let addr = cpu.fetch16_at_ip();
      cpu.mem_fetch(addr)
    }
    //TODO: should be done wrapping add?
    //TODO: check for page boudary crossing
    AbsoluteX => { 
      let addr = cpu.fetch16_at_ip() + cpu.x as u16;
      cpu.mem_fetch(addr)
    }
    //TODO: should be done wrapping add?
    //TODO: check for page boudary crossing
    AbsoluteY => {
      let addr = cpu.fetch16_at_ip() + cpu.y as u16;
      cpu.mem_fetch(addr)
    }
    Indirect => {
      let addr = cpu.fetch16_at_ip();
      cpu.mem_fetch(addr)
    }
  }
}

//TODO: This should be constant in the json
pub fn get_operand_len(inst: &Instruction) -> u16 {
  let mode = inst.addressing;
  use AddressingMode::*;
  match mode {
    Implicit | Accumulator => 0,

    ZeroPage | ZeroPageX | ZeroPageY |
    IndirectX | IndirectY |
    Immediate | Relative => 1,

    Absolute | AbsoluteX | AbsoluteY | 
    Indirect => 2,
  }
}

pub fn lda(mut cpu: Cpu, operand: u8) -> Cpu {
  cpu.set_czn(operand as u16);
  cpu.a = operand;
  cpu
}

pub fn ldx(mut cpu: Cpu, operand: u8) -> Cpu {
  cpu.set_czn(operand as u16);
  cpu.x = operand;
  cpu
}

pub fn ldy(mut cpu: Cpu, operand: u8) -> Cpu {
  cpu.set_czn(operand as u16);
  cpu.y = operand;
  cpu
}

pub fn sta(mut cpu: Cpu, operand: u8) -> Cpu {
  todo!("needs refactoring");
}

pub fn stx(mut cpu: Cpu, operand: u8) -> Cpu {
  todo!("needs refactoring");
}

pub fn sty(mut cpu: Cpu, operand: u8) -> Cpu {
  todo!("needs refactoring");
}

pub fn tax(mut cpu: Cpu, _: u8) -> Cpu {
  cpu.set_czn(cpu.a as u16);
  cpu.x = cpu.a;
  cpu
}

pub fn tay(mut cpu: Cpu, _: u8) -> Cpu {
  cpu.set_czn(cpu.a as u16);
  cpu.y = cpu.a;
  cpu
}

pub fn tsx(mut cpu: Cpu, _: u8) -> Cpu {
  let res = cpu.stack_pop();
  cpu.set_czn(res as u16);
  cpu.x = res;
  cpu
}

pub fn txa(mut cpu: Cpu, _: u8) -> Cpu {
  cpu.set_czn(cpu.x as u16);
  cpu.a = cpu.x;
  cpu
}

pub fn txs(mut cpu: Cpu, _: u8) -> Cpu {
  cpu.stack_push(cpu.x);
  cpu
}

pub fn tya(mut cpu: Cpu, _: u8) -> Cpu {
  cpu.set_czn(cpu.y as u16);
  cpu.a = cpu.y;
  cpu
}

pub fn pha(mut cpu: Cpu, _: u8) -> Cpu {
  cpu.stack_push(cpu.a);
  cpu
}

pub fn php(mut cpu: Cpu, _: u8) -> Cpu {
  cpu.stack_push(cpu.sr.bits());
  cpu
}

pub fn pla(mut cpu: Cpu, _: u8) -> Cpu {
  let res = cpu.stack_pop();
  cpu.set_czn(res as u16);

  cpu.a = res;
  cpu
}

pub fn plp(mut cpu: Cpu, _: u8) -> Cpu {
  let res = cpu.stack_pop();
  
  cpu.sr = StatusReg::from_bits(res).expect("No unused bits should be set");
  cpu
}

pub fn and(mut cpu: Cpu, operand: u8) -> Cpu {
  let res = cpu.a & operand;
  cpu.set_czn(res as u16);

  cpu.a = res;
  cpu
}

pub fn eor(mut cpu: Cpu, operand: u8) -> Cpu {
  let res = cpu.a ^ operand;
  cpu.set_czn(res as u16);

  cpu.a = res;
  cpu
}

pub fn ora(mut cpu: Cpu, operand: u8) -> Cpu {
  let res = cpu.a | operand;
  cpu.set_czn(res as u16);

  cpu.a = res;
  cpu
}

pub fn bit(mut cpu: Cpu, operand: u8) -> Cpu {
  let res = cpu.a & operand;
  if res == 0 { cpu.sr.insert(StatusReg::zero); }
  if res & 0b0100_0000 != 0 { cpu.sr.insert(StatusReg::overflow); }
  if res & 0b1000_0000 != 0 { cpu.sr.insert(StatusReg::negative); }

  cpu
}

pub fn adc(mut cpu: Cpu, operand: u8) -> Cpu {
  let res = cpu.a as u16 + operand as u16 + cpu.carry() as u16;
  cpu.set_overflow(cpu.a as u16, operand as u16, res);
  cpu.set_czn(res);

  cpu.a = res as u8;
  cpu
}

pub fn nop(cpu: Cpu, _: u8) -> Cpu {
  cpu
}



#[cfg(test)]
mod tests {
  use super::*;

  fn write_codes_to_ram(cpu: &mut Cpu, codes: &Vec<u8>) {
    let (first, _) = cpu.mem.split_at_mut(codes.len());
    first.copy_from_slice(codes.as_slice());
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