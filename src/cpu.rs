use core::{cell::OnceCell, fmt, ops::{BitAnd, BitOr, BitXor, Not, Shl, Shr}};

use bitflags::bitflags;
use log::{debug, info, trace};

use crate::{bus::Bus, cart::Cart, instr::{AddressingMode, Instruction, INSTRUCTIONS, RMW_INSTRS}, mem::{Memory, Ram64Kb}};

bitflags! {
  #[derive(Debug, Clone, Copy)]
  pub struct CpuFlags: u8 {
    const carry     = 0b0000_0001;
    const zero      = 0b0000_0010;
    const irq_off   = 0b0000_0100;
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
const SP_RESET: u8 = 0xFD;
const PC_RESET: u16 = RESET_ISR;
const _P_RESET_U8: u8 = 0x24;
const P_RESET: CpuFlags = CpuFlags::irq_off.union(CpuFlags::brkpush);

const NMI_ISR: u16 = 0xFFFA;
const RESET_ISR: u16 = 0xFFFC;
const IRQ_ISR: u16 = 0xFFFE;

pub struct Cpu<M: Memory> {
  pub pc: u16,
  pub sp: u8,
  pub p: CpuFlags,
  pub a: u8,
  pub x: u8,
  pub y: u8,
  pub cycles: usize,
  pub jammed: bool,
  pub bus: M,
}

impl<M: Memory> Memory for Cpu<M> {
  fn read(&mut self, addr: u16) -> u8 {
    let res = self.bus.read(addr);
    self.tick();
    res
  }

  fn write(&mut self, addr: u16, val: u8) {
    self.bus.write(addr, val);
    self.tick();
  }
  
  fn tick(&mut self) {
    self.cycles += 1;
    self.bus.tick();
  }
}

impl<M: Memory> fmt::Debug for Cpu<M> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cpu").field("pc", &self.pc).field("sp", &self.sp).field("sr", &self.p).field("a", &self.a).field("x", &self.x).field("y", &self.y).field("cycles", &self.cycles).finish()
    }
}


impl Cpu<Ram64Kb> {
  pub fn with_ram64kb() -> Self {
    Self {
      pc: PC_RESET,
      sp: SP_RESET,
      a: 0, x: 0, y: 0,
      // At boot, only interrupt flag is enabled
      p: P_RESET,
      cycles: 0,
      jammed: false,
      bus: Ram64Kb { mem: [0; 64 * 1024] },
    }
  }
}

impl Cpu<Bus> {
  pub fn with_cart(cart: Cart) -> Self {
    let mut cpu = Self {
      pc: PC_RESET,
      sp: SP_RESET,
      a: 0, x: 0, y: 0,
      // At boot, only interrupt flag is enabled
      p: P_RESET,
      cycles: 0,
      jammed: false,
      bus: Bus::new(cart),
    };

    // cpu should start by executing the reset subroutine
    cpu.pc = cpu.read16(PC_RESET);
    cpu
  }

  pub fn load_cart(&mut self, cart: Cart) {
    self.bus = Bus::new(cart);
    self.reset();
  }
}

impl<M: Memory> Cpu<M> {
  pub fn reset(&mut self) {
    self.pc = self.read16(PC_RESET);
    self.sp = self.sp.wrapping_sub(3);
    self.p = self.p.clone().union(CpuFlags::irq_off);
  }

  fn set_carry(&mut self, res: u16) {
    self.p.set(CpuFlags::carry, res > u8::MAX as u16);
  }

  fn set_zero(&mut self, res: u8) {
    self.p.set(CpuFlags::zero, res == 0);
  }

  fn set_neg(&mut self, res: u8) {
    self.p.set(CpuFlags::negative, res & 0b1000_0000 != 0);
  }

  fn set_zn(&mut self, res: u8) {
    self.set_zero(res);
    self.set_neg(res);
  }
  
  fn set_czn(&mut self, res: u16) {
    self.set_carry(res);
    self.set_zn(res as u8);
  }

  // https://forums.nesdev.org/viewtopic.php?t=6331
  fn set_overflow(&mut self, a: u16, v: u16, s: u16) {
    let overflow = (a ^ s) & (v ^ s) & 0b1000_0000 != 0;
    self.p.set(CpuFlags::overflow, overflow);
  }

  fn carry(&self) -> u8 {
    self.p.contains(CpuFlags::carry).into()
  }

  pub fn read16(&mut self, addr: u16) -> u16 {
    let low = self.read(addr);
    let high = self.read(addr.wrapping_add(1));
    u16::from_le_bytes([low, high])
  }

  fn wrapping_read16(&mut self, addr: u16) -> u16 {
    if addr & 0x00FF == 0x00FF {
      let page = addr & 0xFF00;
      let low = self.read(page | 0xFF);
      let high = self.read(page | 0x00);
      u16::from_le_bytes([low, high])
    } else { self.read16(addr) }
  }

  fn pc_fetch(&mut self) -> u8 {
    let res = self.read(self.pc);
    self.pc = self.pc.wrapping_add(1);
    res
  }

  fn pc_fetch16(&mut self) -> u16 {
    let res = self.read16(self.pc);
    self.pc = self.pc.wrapping_add(2);
    res
  }

  fn sp_addr(&self) -> u16 {
    // Stack grows downward
    STACK_START.wrapping_add(self.sp as usize) as u16
  }

  fn stack_push(&mut self, val: u8) {
    debug!("-> Pushing ${:02X} to stack at cycle {}", val, self.cycles);
    self.write(self.sp_addr(), val);
    debug!("\t{}", self.stack_trace());
    self.sp = self.sp.wrapping_sub(1);
  }

  fn stack_push16(&mut self, val: u16) {
    let [low, high] = val.to_le_bytes();
    self.stack_push(high);
    self.stack_push(low);
  }

  fn stack_pull(&mut self) -> u8 {
    self.sp = self.sp.wrapping_add(1);
    debug!("<- Pulling ${:02X} from stack at cycle {}", self.read(self.sp_addr()), self.cycles);
    debug!("\t{}", self.stack_trace());
    self.read(self.sp_addr())
  }

  fn stack_pull16(&mut self) -> u16 {
    let low = self.stack_pull();
    let high = self.stack_pull();

    u16::from_le_bytes([low, high])
  }

  pub fn stack_trace(&mut self) -> String {
    let mut trace = String::new();
    const RANGE: i16 = 5;
    for i in -RANGE..=0 {
      let addr = self.sp_addr().wrapping_add_signed(i);
      trace.push_str(&format!("${:02X}, ", self.read(addr)));
    }

    trace
  }
}

#[derive(Debug)]
pub enum Operand { Acc, Imm(u8), Addr(u16, OnceCell<u8>) }
impl Operand {
  pub fn fetchable(addr: u16) -> Self {
    Operand::Addr(addr, OnceCell::new())
  }
}
enum InstrDst {
  Acc, X, Y, Mem(u16)
}

impl<M: Memory> Cpu<M> {
  pub fn step(&mut self) {
    if self.bus.is_dma_transfering() {
      self.bus.handle_dma();
      return;
    }
    self.interrupts_poll();
    
    let opcode = self.pc_fetch();
    let instr = &INSTRUCTIONS[opcode as usize];
    
    let mut op = self.get_operand_with_addressing(instr);
    debug!("{:?} with op {:?} at cycle {}", instr, op, self.cycles);
    
    self.execute(opcode, &mut op);
    // self.cycles += instr.cycles;
  }

  fn interrupts_poll(&mut self) {
    if self.bus.nmi_poll() {
      info!("NMI HANDLING");
      self.handle_interrupt(NMI_ISR);
    } else if self.bus.irq_poll() && !self.p.contains(CpuFlags::irq_off) {
      info!("IRQ HANDLING");
      self.handle_interrupt(IRQ_ISR);
    }
  }
  
  fn handle_interrupt(&mut self, isr_addr: u16) {
    // https://www.nesdev.org/wiki/CPU_interrupts
    self.tick();
    self.tick();

    self.stack_push16(self.pc);
    let pushable = self.p.clone().union(CpuFlags::brkpush);
    self.stack_push(pushable.bits());
    self.p.insert(CpuFlags::irq_off);
    self.pc = self.read16(isr_addr);
  }

  fn get_zeropage_operand(&mut self, offset: u8, instr: &Instruction) -> Operand {
    let zero_addr = self.pc_fetch();

    if instr.addressing != AddressingMode::ZeroPage {
      self.read(zero_addr as u16);
    }

    Operand::fetchable(zero_addr.wrapping_add(offset) as u16)
  }

  // fn get_absolute_operand(&mut self, offset: u8, instr: &Instruction) -> Operand {
  //   let addr_base = self.pc_fetch16();
  //   let addr_effective = addr_base.wrapping_add(offset as u16);

  //   // page crossing check
  //   if instr.page_boundary_cycle && addr_effective & 0xFF00 != addr_base & 0xFF00 {
  //     // dummy read: should read the previous page at effective low address
  //     self.read((addr_base & 0xFF00) | (addr_effective & 0x00FF));
  //     self.cycles += 1;
  //   } else if instr.addressing == AddressingMode::AbsoluteX
  //   && (instr.opcode ==  62 || instr.opcode == 157) {
  //     // STA abs,x and ROL abs,x dummy read
  //     self.read((addr_base & 0xFF00) | (addr_effective & 0x00FF));
  //   }

  //   Operand::fetchable(addr_effective)
  // }

  fn get_absolute_operand(&mut self, offset: u8, instr: &Instruction) -> Operand {
    let addr_base = self.pc_fetch16();
    let addr_effective = addr_base.wrapping_add(offset as u16);
    
    // page crossing check
    if instr.addressing != AddressingMode::Absolute 
    && (addr_effective & 0xFF00 != addr_base & 0xFF00 || RMW_INSTRS.contains(&instr.name)) {
      // TODO: can we do a little better? Perhaps, pass the intermediate address to the instr function...
      // dummy read: should read the previous page at effective low address
      self.read((addr_base & 0xFF00) | (addr_effective & 0x00FF)); 
    }

    Operand::fetchable(addr_effective)
  }

  fn get_operand_with_addressing(&mut self, instr: &Instruction) -> Operand {
    let mode = instr.addressing;
    use AddressingMode::*;
    
    match mode {
      Implicit => {
        // dummy read
        self.read(self.pc + 1);
        Operand::Imm(0)
      },
      Accumulator => {
        // dummy read
        self.read(self.pc + 1);
        Operand::Acc
      },
      Immediate | Relative => Operand::Imm(self.pc_fetch()),
      ZeroPage => self.get_zeropage_operand(0, instr),
      ZeroPageX => self.get_zeropage_operand(self.x, instr),
      ZeroPageY => self.get_zeropage_operand(self.y, instr),
      Absolute => self.get_absolute_operand(0, instr),
      AbsoluteX => self.get_absolute_operand(self.x, instr),
      AbsoluteY => self.get_absolute_operand(self.y, instr),
      Indirect => {
        let addr = self.pc_fetch16();
        let addr_effective = self.wrapping_read16(addr);
        Operand::fetchable(addr_effective)
      }
      IndirectX => {
        // important to keep it as u8
        let zero_addr = self.pc_fetch();
        self.read(zero_addr as u16);
        let addr_base = zero_addr.wrapping_add(self.x) as u16;
        let addr_effective = self.wrapping_read16(addr_base);

        // trace!("[IndirectX] ZeroAddr: {zero_addr:02X}, Effective: {addr_effective:04X}");
        Operand::fetchable(addr_effective)
      }
      IndirectY => {
        let zero_addr = self.pc_fetch() as u16;
        let addr_base = self.wrapping_read16(zero_addr);
        let addr_effective = addr_base.wrapping_add(self.y as u16);
        
        // trace!("[IndirectY] ZeroAddr: {zero_addr:04X}, BaseAddr: {addr_base:04X}, Effective: {addr_effective:04X}");
        // trace!(" | Has crossed boundaries? {}", addr_effective & 0xFF00 != addr_base & 0xFF00);
        
        // page crossing check
        if addr_effective & 0xFF00 != addr_base & 0xFF00 {
          // trace!(" | Boundary crossed at cycle {}", self.cycles);

          // dummy read: Should read the previous page at effective low address
          self.read((addr_base & 0xFF00) | (addr_effective & 0x00FF));
        } else if instr.opcode == 145 {
          // TODO: can we do a little better? Perhaps, pass the intermediate address to the instr function...
          // STA (zp),y dummy read
          self.read((addr_base & 0xFF00) | (addr_effective & 0x00FF));
        }

        Operand::fetchable(addr_effective)
      }
    }
  }

  fn set_instr_result(&mut self, dst: InstrDst, res: u8) {
    match dst {
      InstrDst::Acc => self.a = res,
      InstrDst::X => self.x = res,
      InstrDst::Y => self.y = res,
      InstrDst::Mem(addr) => self.write(addr, res),
    }
  }

  fn get_operand_value(&mut self, op: &mut Operand) -> u8 {
    match op {
      Operand::Acc => self.a,
      Operand::Imm(val) => *val,
      Operand::Addr(addr, val) => *val.get_or_init(|| self.read(*addr)),
    }
  }

  fn load (&mut self, op: &mut Operand, dst: InstrDst) {
    trace!("[LOAD] {op:?} at cycle {}", self.cycles);

    let val = self.get_operand_value(op);
    self.set_zn(val);
    self.set_instr_result(dst, val);
  }

  fn lda(&mut self, op: &mut Operand) {
    self.load(op, InstrDst::Acc)
  }
  fn ldx(&mut self, op: &mut Operand) {
    self.load(op, InstrDst::X)
  }
  fn ldy(&mut self, op: &mut Operand) {
    self.load(op, InstrDst::Y)
  }

  fn store(&mut self, op: &mut Operand, val: u8) {
    if let Operand::Addr(addr, _) = op {
      self.set_instr_result(InstrDst::Mem(*addr), val)
    } else { unreachable!("store operations should always have an address destination, got {op:?}") }
  }

  fn sta(&mut self, op: &mut Operand) {
    self.store(op, self.a)
  }
  fn stx(&mut self, op: &mut Operand) {
    self.store(op, self.x)
  }
  fn sty(&mut self, op: &mut Operand) {
    self.store(op, self.y)
  }

  fn transfer(&mut self, src: u8, dst: InstrDst) {
    self.set_zn(src);
    self.set_instr_result(dst, src);
  }

  fn tax(&mut self, _: &mut Operand) {
    self.transfer(self.a, InstrDst::X)
  }
  fn tay(&mut self, _: &mut Operand) {
    self.transfer(self.a, InstrDst::Y)
  }
  fn tsx(&mut self, _: &mut Operand) {
    self.transfer(self.sp, InstrDst::X)
  }
  fn txa(&mut self, _: &mut Operand) {
    self.transfer(self.x, InstrDst::Acc)
  }
  fn txs(&mut self, _: &mut Operand) {
    // debug!("SP changed from ${:02X} to ${:02X}", self.sp, self.x);
    self.sp = self.x;
  }
  fn tya(&mut self, _: &mut Operand) {
    self.transfer(self.y, InstrDst::Acc)
  }

  fn pha(&mut self, _: &mut Operand) {
    // trace!("[PHA] Pushing ${:02X} to stack at cycle {}", self.a, self.cycles);
    self.stack_push(self.a);
  }
  fn pla(&mut self, _: &mut Operand) {
    // pulling takes 2 cycles, 1 to increment sp, 1 to read
    self.tick();
    let res = self.stack_pull();
    self.set_zn(res);
    self.a = res;
    // trace!("[PLA] Pulled ${:02X} from stack at cycle {}", self.a, self.cycles);
  }
  fn php(&mut self, _: &mut Operand) {
    // Brk is always 1 on pushes
    let pushable = self.p.clone().union(CpuFlags::brkpush);
    // trace!("[PHP] Pushing {pushable:?} (${:02X}) to stack at cycle {}", pushable.bits(), self.cycles);
    self.stack_push(pushable.bits());
  }
  fn plp(&mut self, _: &mut Operand) {
    // pulling takes 2 cycles, 1 to increment sp, 1 to read
    self.tick();
    let res = self.stack_pull();
    // Brk is always 0 on pulls, but unused is always 1
    self.p = CpuFlags::from_bits_retain(res)
      .difference(CpuFlags::brk)
      .union(CpuFlags::unused);
    // trace!("[PLP] Pulled {:?} (${:02X}) from stack at cycle {}", self.p, self.p.bits(), self.cycles);
  }

  fn logical(&mut self, op: &mut Operand, bitop: fn(u8, u8) -> u8) {
    let val = self.get_operand_value(op);
    let res = bitop(self.a, val);
    self.set_zn(res);
    self.a = res;
  }
  fn and(&mut self, op: &mut Operand) {
    self.logical(op, u8::bitand)
  }
  fn eor(&mut self, op: &mut Operand) {
    self.logical(op, u8::bitxor)
  }
  fn ora(&mut self, op: &mut Operand) {
    self.logical(op, u8::bitor)
  }
  fn bit(&mut self, op: &mut Operand) {
    let val = self.get_operand_value(op);
    let res = self.a & val;
    self.set_zero(res);
    self.p.set(CpuFlags::overflow, val & 0b0100_0000 != 0);
    self.p.set(CpuFlags::negative, val & 0b1000_0000 != 0);
  }

  fn addition(&mut self, val: u8) {
    let res = self.a as u16 + val as u16 + self.carry() as u16;
    self.set_overflow(self.a as u16, val as u16, res);
    self.set_czn(res);
    self.a = res as u8;
  }

  fn adc(&mut self, op: &mut Operand) {
    let val = self.get_operand_value(op);
    self.addition(val);
  }
  fn sbc(&mut self, op: &mut Operand) {
    let val = self.get_operand_value(op);
    // self.addition((val as i8).wrapping_neg().wrapping_sub(1) as u8);
    self.addition(val.not());
  }

  fn compare(&mut self, reg: u8, op: &mut Operand) {
    let val = self.get_operand_value(op);
    let res = reg.wrapping_sub(val);
    self.set_zn(res);
    self.p.set(CpuFlags::carry, reg >= val);
  }

  fn cmp(&mut self, op: &mut Operand) {
    self.compare(self.a, op)
  }
  fn cpx(&mut self, op: &mut Operand) {
    self.compare(self.x, op)
  }
  fn cpy(&mut self, op: &mut Operand) {
    self.compare(self.y, op)
  }

  fn increase(&mut self, val: u8, f: fn(u8, u8) -> u8) -> u8 {
    let res = f(val, 1);
    self.set_zn(res);
    res
  }
  fn inc(&mut self, op: &mut Operand) {
    let val = self.get_operand_value(op);
    let res = self.increase(val, u8::wrapping_add);
    if let Operand::Addr(dst, _) = op {
      // dummy write
      self.write(*dst, val);
      self.write(*dst, res);
      *op = Operand::Addr(*dst, OnceCell::from(res));
    } else { unreachable!("inc should always have an address destination, got {op:?}") }
  }
  fn inx(&mut self, _: &mut Operand) {
    self.x = self.increase(self.x, u8::wrapping_add);
  }
  fn iny(&mut self, _: &mut Operand) {
    self.y = self.increase(self.y, u8::wrapping_add)
  }
  fn dec(&mut self, op: &mut Operand) {
    let val = self.get_operand_value(op);
    let res = self.increase(val, u8::wrapping_sub);
    if let Operand::Addr(dst, _) = op {
      // dummy write
      self.write(*dst, val);
      self.write(*dst, res);
      *op = Operand::Addr(*dst, OnceCell::from(res));
    } else { unreachable!("dec should always have an address destination, got {op:?}") }
  }
  fn dex(&mut self, _: &mut Operand) {
    self.x = self.increase(self.x, u8::wrapping_sub);
  }
  fn dey(&mut self, _: &mut Operand) {
    self.y = self.increase(self.y, u8::wrapping_sub);
  }

  fn shift<F: Fn(u8) -> u8>(&mut self, op: &mut Operand, carry_bit: u8, shiftop: F) {
    let val = self.get_operand_value(op);
    self.p.set(CpuFlags::carry, val & carry_bit != 0);
    let res = shiftop(val);
    self.set_zn(res);

    match op {
      Operand::Acc | Operand::Imm(_) => self.a = res,
      Operand::Addr(dst, _) => {
        // dummy write
        self.write(*dst, val);
        self.write(*dst, res);
        *op = Operand::Addr(*dst, OnceCell::from(res))
      }
    }
  }

  fn asl(&mut self, op: &mut Operand) {
    self.shift(op, 0b1000_0000, |v| v.shl(1));
  }
  fn lsr(&mut self, op: &mut Operand) {
    self.shift(op, 1, |v| v.shr(1));
  }
  fn rol(&mut self, op: &mut Operand) {
    let old_carry = self.carry();
    self.shift(op, 0b1000_0000, |v| v.shl(1) | old_carry);
  }
  fn ror(&mut self, op: &mut Operand) {
    let old_carry = self.carry();
    self.shift(op, 1, |v| v.shr(1) | (old_carry << 7));
  }

  fn jmp(&mut self, op: &mut Operand) {
    if let Operand::Addr(src, _) = op {
      self.pc = *src;
    } else { unreachable!("jmp should always have an address destination, got {op:?}") } 
  }
  fn jsr(&mut self, op: &mut Operand) {
    self.stack_push16(self.pc - 1);
    self.jmp(op);
    self.tick();
  }
  fn rts(&mut self, _: &mut Operand) {
    // pulling takes 2 cycles, 1 to increment sp, 1 to read
    self.tick();
    self.pc = self.stack_pull16() + 1;
    // pc increments takes 1 cycle
    self.tick();
  }

  fn branch(&mut self, op: &mut Operand, cond: bool) {
    if cond {
      let offset = self.get_operand_value(op) as i8;
      let new_pc = self.pc.wrapping_add_signed(offset as i16);

      // page boundary cross check
      if self.pc & 0xFF00 != new_pc & 0xFF00 {
        // page cross branch costs 2
        self.tick();
      }

      // same page branch costs 1
      self.tick();

      self.pc = new_pc;
    }
  }
  fn bcc(&mut self, op: &mut Operand) {
    self.branch(op, self.carry() == 0)
  }
  fn bcs(&mut self, op: &mut Operand) {
    self.branch(op, self.carry() == 1)
  }
  fn beq(&mut self, op: &mut Operand) {
    self.branch(op, self.p.contains(CpuFlags::zero))
  }
  fn bne(&mut self, op: &mut Operand) {
    self.branch(op, !self.p.contains(CpuFlags::zero))
  }
  fn bmi(&mut self, op: &mut Operand) {
    self.branch(op, self.p.contains(CpuFlags::negative))
  }
  fn bpl(&mut self, op: &mut Operand) {
    self.branch(op, !self.p.contains(CpuFlags::negative))
  }
  fn bvc(&mut self, op: &mut Operand) {
    self.branch(op, !self.p.contains(CpuFlags::overflow))
  }
  fn bvs(&mut self, op: &mut Operand) {
    self.branch(op, self.p.contains(CpuFlags::overflow))
  }

  fn clear_stat(&mut self, s: CpuFlags) {
    self.p.remove(s);
  }
  fn clc(&mut self, _: &mut Operand) {
    self.clear_stat(CpuFlags::carry)
  }
  fn cld(&mut self, _: &mut Operand) {
    self.clear_stat(CpuFlags::decimal)
  }
  fn cli(&mut self, _: &mut Operand) {
    self.clear_stat(CpuFlags::irq_off)
  }
  fn clv(&mut self, _: &mut Operand) {
    self.clear_stat(CpuFlags::overflow)
  }
  fn set_stat(&mut self, s: CpuFlags) {
    self.p.insert(s);
  }
  fn sec(&mut self, _: &mut Operand) {
    self.set_stat(CpuFlags::carry)
  }
  fn sed(&mut self, _: &mut Operand) {
    self.set_stat(CpuFlags::decimal)
  }
  fn sei(&mut self, _: &mut Operand) {
    self.set_stat(CpuFlags::irq_off)
  }

  fn brk(&mut self, op: &mut Operand) {
    self.stack_push16(self.pc.wrapping_add(1));
    self.php(op);
    self.p.insert(CpuFlags::irq_off);
    self.pc = self.read16(IRQ_ISR);
  }

  fn rti(&mut self, op: &mut Operand) {
    self.plp(op);
    self.pc = self.stack_pull16();
  }
  fn nop(&mut self, op: &mut Operand) {
    // if it is an undocumented nop, it reads the operand and discards it
    self.get_operand_value(op);
  }
}

impl<M: Memory> Cpu<M> {
  fn usbc(&mut self, op: &mut Operand) {
    self.sbc(op);
  }

  fn alr(&mut self, op: &mut Operand) {
    self.and(op);
    self.lsr(&mut Operand::Acc);
  }

  fn slo(&mut self, op: &mut Operand) {
    self.asl(op);
    self.ora(op);
  }

  fn sre(&mut self, op: &mut Operand) {
    self.lsr(op);
    self.eor(op);
  }

  fn rla(&mut self, op: &mut Operand) {
    self.rol(op);
    self.and(op);
  }

  fn rra(&mut self, op: &mut Operand) {
    self.ror(op);
    self.adc(op);
  }

  fn anc(&mut self, op: &mut Operand) {
    self.and(op);
    self.p.set(CpuFlags::carry, self.p.contains(CpuFlags::negative));
  }

  fn arr(&mut self, op: &mut Operand) {
    self.and(op);
    self.ror(&mut Operand::Acc);
    let res = self.a;
    let bit6 = res & 0b0100_0000 != 0;
    let bit5 = res & 0b0010_0000 != 0;
    self.p.set(CpuFlags::carry, bit6);
    self.p.set(CpuFlags::overflow, bit6 ^ bit5);
  }

  fn dcp(&mut self, op: &mut Operand) {
    self.dec(op);
    self.cmp(op);
  }

  fn isc(&mut self, op: &mut Operand) {
    self.inc(op);
    self.sbc(op); 
  }

  fn las(&mut self, op: &mut Operand) {
    let val = self.get_operand_value(op);
    let res = val & self.sp;
    self.a = res;
    self.x = res;
    self.sp = res;
    self.set_zn(res);
  }

  fn lax(&mut self, op: &mut Operand) {
    self.lda(op);
    self.ldx(op);
  }

  // also called AXS, SAX
  fn sbx(&mut self, op: &mut Operand) {
    let val = self.get_operand_value(op);
    self.compare(self.a & self.x, op);
    let res = (self.a & self.x).wrapping_sub(val);
    self.x = res;
  }

  // also called AXS, AAX
  fn sax(&mut self, op: &mut Operand) {
    if let Operand::Addr(dst, _) = op {
      let res = self.a & self.x;
      self.set_instr_result(InstrDst::Mem(*dst), res);
    }
  }

  fn high_addr_bitand(&mut self, op: &mut Operand, val: u8) {
    if let Operand::Addr(dst, _) = op {
      let addr_hi = (*dst >> 8) as u8;
      let addr_lo = (*dst & 0xFF) as u8; 
      let res = val & addr_hi.wrapping_add(1);
      let dst = (((val & (addr_hi.wrapping_add(1))) as u16) << 8) | addr_lo as u16;
      self.set_instr_result(InstrDst::Mem(dst), res);
    }
  }

  // also called XAS, SHS
  fn tas(&mut self, op: &mut Operand) {
    let res = self.a & self.x;
    self.sp = res;
    self.high_addr_bitand(op, res);
  }

  // also called SXA, XAS
  fn shx(&mut self, op: &mut Operand) {
    self.high_addr_bitand(op, self.x);
  }

  // also called A11m SYA, SAY
  fn shy(&mut self, op: &mut Operand) {
    self.high_addr_bitand(op, self.y);
  }

  // also called AHX, AXA
  fn sha(&mut self, op: &mut Operand) {
    self.high_addr_bitand(op, self.a & self.x);
  }

  // also called XAA
  fn ane(&mut self, op: &mut Operand) {
    self.txa(op);
    self.and(op);
  }

  // also called LAXI
  fn lxa(&mut self, op: &mut Operand) {
    let val = self.get_operand_value(op);
    self.set_zn(val);
    self.a = val;
    self.x = val;
  }

  // also called KIL, HLT
  fn jam(&mut self, _: &mut Operand) {
    self.jammed = true;
  }
}

impl<M: Memory> Cpu<M> {
  fn execute(&mut self, code: u8, op: &mut Operand) {
    match code {
      0 => self.brk(op),
      1 | 5 | 9 | 13 | 17 | 21 | 25 | 29 => self.ora(op),
      2 | 18 | 34 | 50 | 66 | 82 | 98 | 114 | 146 | 178 | 210 | 242 => self.jam(op),
      3 | 7 | 15 | 19 | 23 | 27 | 31 => self.slo(op),
      4 | 12 | 20 | 26 | 28 | 52 | 58 | 60 | 68 | 84 | 90 | 92 
      | 100 | 116 | 122 | 124 | 128 | 130 | 137 | 194
      | 212 | 218 | 220 | 226 | 234 | 244 | 250 | 252 => self.nop(op),
      6 | 10 | 14 | 22 | 30 => self.asl(op),
      8 => self.php(op),
      11 | 43 => self.anc(op),
      16 => self.bpl(op),
      24 => self.clc(op),
      32 => self.jsr(op),
      33 | 37 | 41 | 45 | 49 | 53 | 57 | 61 => self.and(op),
      35 | 39 | 47 | 51 | 55 | 59 | 63 => self.rla(op),
      36 | 44 => self.bit(op),
      38 | 42 | 46 | 54 | 62 => self.rol(op),
      40 => self.plp(op),
      48 => self.bmi(op),
      56 => self.sec(op),
      64 => self.rti(op),
      65 | 69 | 73 | 77 | 81 | 85 | 89 | 93 => self.eor(op),
      67 | 71 | 79 | 83 | 87 | 91 | 95 => self.sre(op),
      70 | 74 | 78 | 86 | 94 => self.lsr(op),
      72 => self.pha(op),
      75 => self.alr(op),
      76 | 108 => self.jmp(op),
      80 => self.bvc(op),
      88 => self.cli(op),
      96 => self.rts(op),
      97 | 101 | 105 | 109 | 113 | 117 | 121 | 125 => self.adc(op),
      99 | 103 | 111 | 115 | 119 | 123 | 127 => self.rra(op),
      102 | 106 | 110 | 118 | 126 => self.ror(op),
      104 => self.pla(op),
      107 => self.arr(op),
      112 => self.bvs(op),
      120 => self.sei(op),
      129 | 133 | 141 | 145 | 149 | 153 | 157 => self.sta(op),
      131 | 135 | 143 | 151 => self.sax(op),
      132 | 140 | 148 => self.sty(op),
      134 | 142 | 150 => self.stx(op),
      136 => self.dey(op),
      138 => self.txa(op),
      139 => self.ane(op),
      144 => self.bcc(op),
      147 | 159 => self.sha(op),
      152 => self.tya(op),
      154 => self.txs(op),
      155 => self.tas(op),
      156 => self.shy(op),
      158 => self.shx(op),
      160 | 164 | 172 | 180 | 188 => self.ldy(op),
      161 | 165 | 169 | 173 | 177 | 181 | 185 | 189 => self.lda(op),
      162 | 166 | 174 | 182 | 190 => self.ldx(op),
      163 | 167 | 175 | 179 | 183 | 191 => self.lax(op),
      168 => self.tay(op),
      170 => self.tax(op),
      171 => self.lxa(op),
      176 => self.bcs(op),
      184 => self.clv(op),
      186 => self.tsx(op),
      187 => self.las(op),
      192 | 196 | 204 => self.cpy(op),
      193 | 197 | 201 | 205 | 209 | 213 | 217 | 221 => self.cmp(op),
      195 | 199 | 207 | 211 | 215 | 219 | 223 => self.dcp(op),
      198 | 206 | 214 | 222 => self.dec(op),
      200 => self.iny(op),
      202 => self.dex(op),
      203 => self.sbx(op),
      208 => self.bne(op),
      216 => self.cld(op),
      224 | 228 | 236 => self.cpx(op),
      225 | 229 | 233 | 237 | 241 | 245 | 249 | 253 => self.sbc(op),
      227 | 231 | 239 | 243 | 247 | 251 | 255 => self.isc(op),
      230 | 238 | 246 | 254 => self.inc(op),
      232 => self.inx(op),
      235 => self.usbc(op),
      240 => self.beq(op),
      248 => self.sed(op),
    }
  }
}