use core::{fmt, ops::{BitAnd, BitOr, BitXor, Not, Shl, Shr}};

use bitflags::bitflags;

use crate::{bus::Bus, cart::Cart, addr::{AddressingMode, MODES_TABLE}, mem::{Memory, Ram64Kb}};

bitflags! {
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
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

const NMI_ISR: u16   = 0xFFFA;
const RESET_ISR: u16 = 0xFFFC;
const IRQ_ISR: u16   = 0xFFFE;


#[derive(serde::Serialize, serde::Deserialize)]
pub struct Cpu<M: Memory> {
  pub pc: u16,
  pub sp: u8,
  pub p: CpuFlags,
  pub a: u8,
  pub x: u8,
  pub y: u8,
  pub cycles: usize,
  pub jammed: bool,

  #[serde(skip)]
  instr_addr: u16,
  #[serde(skip)]
  instr_val:  u8,
  #[serde(skip)]
  instr_dummy_addr: u16,
  #[serde(skip)]
  instr_dummy_readed: bool,
  #[serde(skip)]
  instr_mode: AddressingMode,

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

      instr_addr: 0,
      instr_val: 0,
      instr_dummy_addr: 0,
      instr_dummy_readed: false,
      instr_mode: Default::default(),
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

      instr_addr: 0,
      instr_val: 0,
      instr_dummy_addr: 0,
      instr_dummy_readed: false,
      instr_mode: Default::default(),
    };

    // boot only if cart contains prg
    if !cpu.bus.cart.as_mut().prg.is_empty() {
      // cpu should start by executing the reset subroutine
      cpu.pc = cpu.read16(PC_RESET);
    }
    cpu
  }
}

impl<M: Memory> Cpu<M> {
  pub fn reset(&mut self) {
    self.pc = self.read16(PC_RESET);
    self.sp = self.sp.wrapping_sub(3);
    self.p = self.p | CpuFlags::irq_off;
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
    self.write(self.sp_addr(), val);
    self.sp = self.sp.wrapping_sub(1);
  }

  fn stack_push16(&mut self, val: u16) {
    let [low, high] = val.to_le_bytes();
    self.stack_push(high);
    self.stack_push(low);
  }

  fn stack_pull(&mut self) -> u8 {
    self.sp = self.sp.wrapping_add(1);
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


impl<M: Memory> Cpu<M> {
  pub fn step(&mut self) {
    self.interrupts_poll();
    
    let opcode = self.pc_fetch();
    // let instr = &INSTRUCTIONS[opcode as usize];
    let mode = MODES_TABLE[opcode as usize];
    self.fetch_operand(mode);
    
    self.execute(opcode);
  }

  fn interrupts_poll(&mut self) {
    if self.bus.nmi_poll() {
      self.handle_interrupt(NMI_ISR);
    } else if self.bus.irq_poll() && !self.p.contains(CpuFlags::irq_off) {
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

  fn fetch_zeropage_operand(&mut self, offset: u8) {
    let zero_addr = self.pc_fetch();
    // dummy read
    self.read(zero_addr as u16);
    self.instr_addr = zero_addr.wrapping_add(offset) as u16;
  }

  fn fetch_absolute_operand(&mut self, offset: u8) {
    let addr_base = self.pc_fetch16();
    let addr_effective = addr_base.wrapping_add(offset as u16);
    self.instr_dummy_addr = (addr_base & 0xFF00) | (addr_effective & 0x00FF);

    // page crossing check
    if addr_effective & 0xFF00 != addr_base & 0xFF00 {
      // dummy read: should read the previous page at effective low address
      self.read(self.instr_dummy_addr);
      self.instr_dummy_readed = true;
    }

    self.instr_addr = addr_effective;
  }

  fn fetch_operand(&mut self, mode: AddressingMode) {
    self.instr_mode = mode;
    self.instr_dummy_readed = false;
    self.instr_val = 0;

    use AddressingMode::*;
    match mode {
      Implied => {
        // dummy read
        self.read(self.pc + 1);
      },
      Accumulator => {
        // dummy read
        self.read(self.pc + 1);
        self.instr_val = self.a;
      },
      Immediate | Relative => self.instr_val = self.pc_fetch(),
      ZeroPage  => self.instr_addr = self.pc_fetch() as u16,
      ZeroPageX => self.fetch_zeropage_operand(self.x),
      ZeroPageY => self.fetch_zeropage_operand(self.y),
      Absolute  => self.instr_addr = self.pc_fetch16(),
      AbsoluteX => self.fetch_absolute_operand(self.x),
      AbsoluteY => self.fetch_absolute_operand(self.y),
      Indirect => {
        let addr = self.pc_fetch16();
        self.instr_addr = self.wrapping_read16(addr);
      }
      IndirectX => {
        // important to keep it as u8
        let zero_addr = self.pc_fetch();
        self.read(zero_addr as u16);
        let addr_base = zero_addr.wrapping_add(self.x) as u16;
        let addr_effective = self.wrapping_read16(addr_base);
        self.instr_addr = addr_effective;
      }
      IndirectY => {
        let zero_addr = self.pc_fetch() as u16;
        let addr_base = self.wrapping_read16(zero_addr);
        let addr_effective = addr_base.wrapping_add(self.y as u16);
        self.instr_dummy_addr = (addr_base & 0xFF00) | (addr_effective & 0x00FF);

        // page crossing check
        if addr_effective & 0xFF00 != addr_base & 0xFF00 {
          // dummy read: Should read the previous page at effective low address
          self.read(self.instr_dummy_addr);
          self.instr_dummy_readed = true;
        }

        self.instr_addr = addr_effective;
      }
    }
  }

  fn absolute_dummy_read(&mut self) {
    let should_dummy_read = !self.instr_dummy_readed 
    && (self.instr_mode == AddressingMode::AbsoluteX || self.instr_mode == AddressingMode::AbsoluteY);

    if should_dummy_read {
      self.read(self.instr_dummy_addr);
      self.instr_dummy_readed = true;
    }
  }

  fn fetch_operand_value(&mut self) -> u8 {
    use AddressingMode::*;
    match self.instr_mode {
      Implied | Immediate | Accumulator | Relative => self.instr_val,
      _ => self.read(self.instr_addr)
    }
  }
}


enum RegTarget { A, X, Y }

impl<M: Memory> Cpu<M> {
  fn load (&mut self) -> u8 {
    let val = self.fetch_operand_value();
    self.set_zn(val);
    val
  }
  fn lda(&mut self) { self.a = self.load() }
  fn ldx(&mut self) { self.x = self.load() }
  fn ldy(&mut self) { self.y = self.load() }

  fn store(&mut self, val: u8) {
    self.absolute_dummy_read();
    self.write(self.instr_addr, val);
  }
  fn sta(&mut self) {
    // this instruction always does the dummy read in indirectY mode
    if !self.instr_dummy_readed && self.instr_mode == AddressingMode::IndirectY {
      self.read(self.instr_dummy_addr);
      self.instr_dummy_readed = true;
    }

    self.store(self.a)
  }
  fn stx(&mut self) { self.store(self.x) }
  fn sty(&mut self) { self.store(self.y) }

  fn tax(&mut self) {
    self.set_zn(self.a);
    self.x = self.a;
  }
  fn tay(&mut self) { 
    self.set_zn(self.a);
    self.y = self.a;
  }
  fn tsx(&mut self) {
    self.set_zn(self.sp);
    self.x = self.sp;
  }
  fn txa(&mut self) {
    self.set_zn(self.x);
    self.a = self.x; 
  }
  fn txs(&mut self) { self.sp = self.x; }
  fn tya(&mut self) { 
    self.set_zn(self.y);
    self.a = self.y;
  }

  fn pha(&mut self) {
    self.stack_push(self.a);
  }
  fn pla(&mut self) {
    // pulling takes 2 cycles, 1 to increment sp, 1 to read
    self.tick();
    let res = self.stack_pull();
    self.set_zn(res);
    self.a = res;
  }
  fn php(&mut self) {
    // Brk is always 1 on pushes
    let pushable = self.p.clone().union(CpuFlags::brkpush);
    self.stack_push(pushable.bits());
  }
  fn plp(&mut self) {
    // pulling takes 2 cycles, 1 to increment sp, 1 to read
    self.tick();
    let res = self.stack_pull();
    // Brk is always 0 on pulls, but unused is always 1
    self.p = CpuFlags::from_bits_retain(res)
      .difference(CpuFlags::brk)
      .union(CpuFlags::unused);
  }

  fn logical(&mut self, bitop: fn(u8, u8) -> u8) {
    let val = self.fetch_operand_value();
    let res = bitop(self.a, val);
    self.set_zn(res);
    self.a = res;
    self.instr_val = res;
  }
  fn and(&mut self) { self.logical(u8::bitand) }
  fn eor(&mut self) { self.logical(u8::bitxor) }
  fn ora(&mut self) { self.logical(u8::bitor) }

  fn bit(&mut self) {
    let val = self.fetch_operand_value();
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

  fn adc(&mut self) {
    let val = self.fetch_operand_value();
    self.addition(val);
  }
  fn sbc(&mut self) {
    let val = self.fetch_operand_value();
    self.addition(val.not());
  }

  fn compare(&mut self, reg: u8) {
    let val = self.fetch_operand_value();
    let res = reg.wrapping_sub(val);
    self.set_zn(res);
    self.p.set(CpuFlags::carry, reg >= val);
  }
  fn cmp(&mut self) { self.compare(self.a) }
  fn cpx(&mut self) { self.compare(self.x) }
  fn cpy(&mut self) { self.compare(self.y) }

  fn increase(&mut self, val: u8, f: fn(u8, u8) -> u8) -> u8 {
    let res = f(val, 1);
    self.set_zn(res);
    res
  }
  fn inc(&mut self) {
    self.absolute_dummy_read();

    let val = self.fetch_operand_value();
    let res = self.increase(val, u8::wrapping_add);

    // dummy write
    self.write(self.instr_addr, val);
    self.write(self.instr_addr, res);
    self.instr_val = res;
  }
  fn inx(&mut self) {
    self.x = self.increase(self.x, u8::wrapping_add);
  }
  fn iny(&mut self) {
    self.y = self.increase(self.y, u8::wrapping_add)
  }
  fn dec(&mut self) {
    self.absolute_dummy_read();

    let val = self.fetch_operand_value();
    let res = self.increase(val, u8::wrapping_sub);

    // dummy write
    self.write(self.instr_addr, val);
    self.write(self.instr_addr, res);
    self.instr_val = res;
  }
  fn dex(&mut self) {
    self.x = self.increase(self.x, u8::wrapping_sub);
  }
  fn dey(&mut self) {
    self.y = self.increase(self.y, u8::wrapping_sub);
  }

  fn shift<F: Fn(u8) -> u8>(&mut self, carry_bit: u8, shiftop: F) {
    self.absolute_dummy_read();

    let val = self.fetch_operand_value();
    self.p.set(CpuFlags::carry, val & carry_bit != 0);
    let res = shiftop(val);
    self.set_zn(res);

    use AddressingMode::*;
    match self.instr_mode {
      Implied | Immediate | Accumulator | Relative => self.a = res,
      _ => {
        // dummy write
        self.write(self.instr_addr, val);
        self.write(self.instr_addr, res);
      }
    }

    self.instr_val = res;
  }

  fn asl(&mut self) {
    self.shift(0b1000_0000, |v| v.shl(1));
  }
  fn lsr(&mut self) {
    self.shift(1, |v| v.shr(1));
  }
  fn rol(&mut self) {
    let old_carry = self.carry();
    self.shift(0b1000_0000, |v| v.shl(1) | old_carry);
  }
  fn ror(&mut self) {
    let old_carry = self.carry();
    self.shift(1, |v| v.shr(1) | (old_carry << 7));
  }

  fn jmp(&mut self) {
    self.pc = self.instr_addr;
  }
  fn jsr(&mut self) {
    self.stack_push16(self.pc - 1);
    self.jmp();
    self.tick();
  }
  fn rts(&mut self) {
    // pulling takes 2 cycles, 1 to increment sp, 1 to read
    self.tick();
    self.pc = self.stack_pull16() + 1;
    // pc increments takes 1 cycle
    self.tick();
  }

  fn branch(&mut self, cond: bool) {
    if cond {
      let offset = self.fetch_operand_value() as i8;
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
  fn bcc(&mut self) { self.branch(self.carry() == 0) }
  fn bcs(&mut self) { self.branch(self.carry() == 1) }
  fn beq(&mut self) { self.branch(self.p.contains(CpuFlags::zero)) }
  fn bne(&mut self) { self.branch(!self.p.contains(CpuFlags::zero)) }
  fn bmi(&mut self) { self.branch(self.p.contains(CpuFlags::negative)) }
  fn bpl(&mut self) { self.branch(!self.p.contains(CpuFlags::negative)) }
  fn bvc(&mut self) { self.branch(!self.p.contains(CpuFlags::overflow)) }
  fn bvs(&mut self) { self.branch(self.p.contains(CpuFlags::overflow)) }

  fn clear_stat(&mut self, s: CpuFlags) { self.p.remove(s); }
  fn clc(&mut self) { self.clear_stat(CpuFlags::carry) }
  fn cld(&mut self) { self.clear_stat(CpuFlags::decimal) }
  fn cli(&mut self) { self.clear_stat(CpuFlags::irq_off) }
  fn clv(&mut self) { self.clear_stat(CpuFlags::overflow) }

  fn set_stat(&mut self, s: CpuFlags) { self.p.insert(s); }
  fn sec(&mut self) { self.set_stat(CpuFlags::carry) }
  fn sed(&mut self) { self.set_stat(CpuFlags::decimal) }
  fn sei(&mut self) { self.set_stat(CpuFlags::irq_off) }

  fn brk(&mut self) {
    self.stack_push16(self.pc.wrapping_add(1));
    self.php();
    self.p.insert(CpuFlags::irq_off);
    self.pc = self.read16(IRQ_ISR);
  }

  fn rti(&mut self) {
    self.plp();
    self.pc = self.stack_pull16();
  }

  fn nop(&mut self) {
    // if it is an undocumented nop it reads the operand and discards it
    self.fetch_operand_value();
  }
}

impl<M: Memory> Cpu<M> {
  fn usbc(&mut self) { self.sbc(); }

  fn alr(&mut self) {
    self.and();
    self.instr_mode = AddressingMode::Accumulator;
    self.lsr();
  }

  fn slo(&mut self) {
    self.asl();
    self.ora();
  }

  fn sre(&mut self) {
    self.lsr();
    self.eor();
  }

  fn rla(&mut self) {
    self.rol();
    self.and();
  }

  fn rra(&mut self) {
    self.ror();
    self.adc();
  }

  fn anc(&mut self) {
    self.and();
    self.p.set(CpuFlags::carry, self.p.contains(CpuFlags::negative));
  }

  fn arr(&mut self) {
    self.and();
    self.instr_mode = AddressingMode::Accumulator;
    self.ror();
    let res = self.a;
    let bit6 = res & 0b0100_0000 != 0;
    let bit5 = res & 0b0010_0000 != 0;
    self.p.set(CpuFlags::carry, bit6);
    self.p.set(CpuFlags::overflow, bit6 ^ bit5);
  }

  fn dcp(&mut self) {
    self.dec();
    self.cmp();
  }

  // also called ISB, INS
  fn isc(&mut self) {
    self.inc();
    self.sbc(); 
  }

  fn las(&mut self) {
    let val = self.fetch_operand_value();
    let res = val & self.sp;
    self.a = res;
    self.x = res;
    self.sp = res;
    self.set_zn(res);
  }

  fn lax(&mut self) {
    self.lda();
    self.ldx();
  }

  // also called AXS, SAX
  fn sbx(&mut self) {
    let val = self.fetch_operand_value();
    self.compare(self.a & self.x);
    let res = (self.a & self.x).wrapping_sub(val);
    self.x = res;
  }

  // also called AXS, AAX
  fn sax(&mut self) {
    let res = self.a & self.x;
    self.write(self.instr_addr, res);
  }

  fn high_addr_bitand(&mut self, val: u8) {
    let addr_hi = (self.instr_addr >> 8) as u8;
    let addr_lo = (self.instr_addr & 0xFF) as u8; 
    let res = val & addr_hi.wrapping_add(1);
    let dst = (((val & (addr_hi.wrapping_add(1))) as u16) << 8) | addr_lo as u16;
    self.write(dst, res);
  }

  // also called XAS, SHS
  fn tas(&mut self) {
    let res = self.a & self.x;
    self.sp = res;
    self.high_addr_bitand(res);
  }

  // also called SXA, XAS
  fn shx(&mut self) {
    self.absolute_dummy_read();
    self.high_addr_bitand(self.x);
  }

  // also called A11m SYA, SAY
  fn shy(&mut self) {
    self.absolute_dummy_read();
    self.high_addr_bitand(self.y);
  }

  // also called AHX, AXA
  fn sha(&mut self) {
    self.absolute_dummy_read();
    self.high_addr_bitand(self.a & self.x);
  }

  // also called XAA
  fn ane(&mut self) {
    self.txa();
    self.and();
  }

  // also called LAXI
  fn lxa(&mut self) {
    let val = self.fetch_operand_value();
    self.set_zn(val);
    self.a = val;
    self.x = val;
  }

  // also called KIL, HLT
  fn jam(&mut self) {
    self.jammed = true;
    panic!("System jammed! (reached JAM instruction)")
  }
}

impl<M: Memory> Cpu<M> {
  fn execute(&mut self, code: u8) {
    match code {
      0 => self.brk(),
      1 | 5 | 9 | 13 | 17 | 21 | 25 | 29 => self.ora(),
      2 | 18 | 34 | 50 | 66 | 82 | 98 | 114 | 146 | 178 | 210 | 242 => self.jam(),
      3 | 7 | 15 | 19 | 23 | 27 | 31 => self.slo(),
      4 | 12 | 20 | 26 | 28 | 52 | 58 | 60 | 68 | 84 | 90 | 92 
      | 100 | 116 | 122 | 124 | 128 | 130 | 137 | 194
      | 212 | 218 | 220 | 226 | 234 | 244 | 250 | 252 => self.nop(),
      6 | 10 | 14 | 22 | 30 => self.asl(),
      8 => self.php(),
      11 | 43 => self.anc(),
      16 => self.bpl(),
      24 => self.clc(),
      32 => self.jsr(),
      33 | 37 | 41 | 45 | 49 | 53 | 57 | 61 => self.and(),
      35 | 39 | 47 | 51 | 55 | 59 | 63 => self.rla(),
      36 | 44 => self.bit(),
      38 | 42 | 46 | 54 | 62 => self.rol(),
      40 => self.plp(),
      48 => self.bmi(),
      56 => self.sec(),
      64 => self.rti(),
      65 | 69 | 73 | 77 | 81 | 85 | 89 | 93 => self.eor(),
      67 | 71 | 79 | 83 | 87 | 91 | 95 => self.sre(),
      70 | 74 | 78 | 86 | 94 => self.lsr(),
      72 => self.pha(),
      75 => self.alr(),
      76 | 108 => self.jmp(),
      80 => self.bvc(),
      88 => self.cli(),
      96 => self.rts(),
      97 | 101 | 105 | 109 | 113 | 117 | 121 | 125 => self.adc(),
      99 | 103 | 111 | 115 | 119 | 123 | 127 => self.rra(),
      102 | 106 | 110 | 118 | 126 => self.ror(),
      104 => self.pla(),
      107 => self.arr(),
      112 => self.bvs(),
      120 => self.sei(),
      129 | 133 | 141 | 145 | 149 | 153 | 157 => self.sta(),
      131 | 135 | 143 | 151 => self.sax(),
      132 | 140 | 148 => self.sty(),
      134 | 142 | 150 => self.stx(),
      136 => self.dey(),
      138 => self.txa(),
      139 => self.ane(),
      144 => self.bcc(),
      147 | 159 => self.sha(),
      152 => self.tya(),
      154 => self.txs(),
      155 => self.tas(),
      156 => self.shy(),
      158 => self.shx(),
      160 | 164 | 172 | 180 | 188 => self.ldy(),
      161 | 165 | 169 | 173 | 177 | 181 | 185 | 189 => self.lda(),
      162 | 166 | 174 | 182 | 190 => self.ldx(),
      163 | 167 | 175 | 179 | 183 | 191 => self.lax(),
      168 => self.tay(),
      170 => self.tax(),
      171 => self.lxa(),
      176 => self.bcs(),
      184 => self.clv(),
      186 => self.tsx(),
      187 => self.las(),
      192 | 196 | 204 => self.cpy(),
      193 | 197 | 201 | 205 | 209 | 213 | 217 | 221 => self.cmp(),
      195 | 199 | 207 | 211 | 215 | 219 | 223 => self.dcp(),
      198 | 206 | 214 | 222 => self.dec(),
      200 => self.iny(),
      202 => self.dex(),
      203 => self.sbx(),
      208 => self.bne(),
      216 => self.cld(),
      224 | 228 | 236 => self.cpx(),
      225 | 229 | 233 | 237 | 241 | 245 | 249 | 253 => self.sbc(),
      227 | 231 | 239 | 243 | 247 | 251 | 255 => self.isc(),
      230 | 238 | 246 | 254 => self.inc(),
      232 => self.inx(),
      235 => self.usbc(),
      240 => self.beq(),
      248 => self.sed(),
    }
  }
}