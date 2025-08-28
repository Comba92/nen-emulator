use std::ops::{Shl, Shr};
use crate::emu::Emu;

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
  pub fn cpu_reset(&mut self) {
    self.cpu.pc = self.cpu_read16(RST_VECTOR);
    self.cpu.p |= Status::IrqDisable;
    self.cpu.sp -= 3;
  }

  #[cfg(not(feature = "ram64kb"))]
  fn cpu_read8(&mut self, addr: u16) -> u8 {
    let res = self.cpu_dispatch_read(addr);
    self.cpu_tick();
    res
  }

  #[cfg(not(feature = "ram64kb"))]
  fn cpu_write8(&mut self, addr: u16, val: u8) {
    self.cpu_dispatch_write(addr, val);
    self.cpu_tick();
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

  pub fn cpu_step(&mut self) {
    // TODO: handle dma better
    if self.handle_dma() { return; }
    self.poll_interrupts();
    let opcode = self.pc_fetch8();

    self.fetch_operand(opcode);
    self.decode_n_exec(opcode);
  }

  fn handle_dma(&mut self) -> bool {
    if self.apu.dmc.buffer.is_none() && self.apu.dmc.dma.remaining > 0 {
      // TODO: do not do halting cycles here
      // // halting cycle
      // self.cpu_tick();

      // dummy cycle
      self.cpu_tick();
      // TODO: +1 cycle on odd cpu cyles

      self.cpu_tick();
      let byte = self.cpu_dispatch_read(self.apu.dmc.dma.addr);
      self.read_dmc_sample(byte);
    }
    
    if self.ppu.dma.remaining > 0 {
      // https://www.nesdev.org/wiki/PPU_registers#OAMDMA_-_Sprite_DMA_($4014_write)
    
      // TODO: do not do halting cycles here
      // // halting cycle
      // self.cpu_tick();
      // TODO: +1 cycle on odd cpu cyles

      self.cpu_tick();
      let byte = self.cpu_dispatch_read(self.ppu.dma.addr);
      self.ppu.dma.addr += 1;
      self.ppu.dma.remaining -= 1;

      self.cpu_tick();
      self.ppu.oam_write(byte);
    }

    self.ppu.dma.remaining > 0 || (self.apu.dmc.buffer.is_none() && self.apu.dmc.dma.remaining > 0)
  }

  fn poll_interrupts(&mut self) {
    // https://www.nesdev.org/wiki/CPU_interrupts#IRQ_and_NMI_tick-by-tick_execution
    if self.mem.nmi {
      self.mem.nmi = false;
      self.handle_interrupt(NMI_VECTOR);
    } else if !self.mem.irq.is_empty()
      && !self.cpu.p.contains(Status::IrqDisable)
    {
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
    // 3   address   R  read from address, add index register to it
    self.cpu_read8(zero_addr as u16);
    self.cpu.op_addr = zero_addr.wrapping_add(offset) as u16;
  }

  // (ASL, LSR, ROL, ROR, INC, DEC, SLO, SRE, RLA, RRA, ISB, DCP)
  // (STA, STX, STY)
  // const RMW: &[u8] = &[0x1e, 0xde, 0xfe, 0x5e, 0x3e, 0x7e, 0x9d, 0x99, 0x1f, 0x1b, 0x5f, 0x5b, 0x3f, 0x3b, 0x7f, 0x7b, 0xff, 0xfb, 0xdf, 0xdb];
  
  // TODO: temporary solution, definetely too over engineered for this
  fn is_rmw_op(opcode: u8) -> bool {
    // we are interested in b, e, f
    // precondition: we will never get any opcode with a here
    // a 1010
    // b 1011
    // c 1100
    // d 1101
    // e 1110
    // f 1111

    // the mask is: 1010 (this excludes c, d, we are ok with a)

    // lo nibble 0x0b, 0x0e, 0x0f
    let is_rmw = 
      // (opcode & 0x0f >= 0x0e || opcode & 0x0f == 0x0b) 
      opcode & 0b1010 == 0b1010
      // all odds hi nipple
      && opcode & 0x10 != 0 
      // except 0xbe (LDX)
      && opcode != 0xbe;

    // STA, hi nipple == 0x9
    let is_w = opcode & 0xf0 == 0x90;
    is_rmw || is_w
  }

  fn fetch_absolute_op(&mut self, offset: u8, opcode: u8) {
    let base_addr = self.pc_fetch16();
    self.cpu.op_addr = base_addr.wrapping_add(offset as u16);

    // page crossing check
    if (base_addr & 0xff00 != self.cpu.op_addr & 0xff00)
      // Read-Modify-Instructions ALWAYS do this dummy read.
      || Self::is_rmw_op(opcode)
    {
      self.cpu_read8(base_addr & 0xff00 | self.cpu.op_addr & 0xff);
    }
  }

  fn fetch_operand(&mut self, opcode: u8) {
    let mode = &MODES_TABLE[opcode as usize];
    self.cpu.op_val = None;

    // https://www.nesdev.org/6502_cpu.txt
    match mode {
      // 2    PC     R  read next instruction byte (and throw it away) 
      Implied | Accumulator => {
        self.cpu_tick();
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
        // 3    pointer    R  read from the address, add X to it
        self.cpu_read8(zero_addr as u16);

        let addr = zero_addr.wrapping_add(self.cpu.x) as u16;
        self.cpu.op_addr = self.wrapping_cpu_read16(addr);
      },
      IndirectY => {
        let zero_addr = self.pc_fetch8() as u16;
        let base_addr = self.wrapping_cpu_read16(zero_addr);
        self.cpu.op_addr = base_addr.wrapping_add(self.cpu.y as u16);

        // page crossing check
        // STA is the only exception, it ALWAYS does this dummy read.
        if (base_addr & 0xff00 != self.cpu.op_addr & 0xff00) || opcode == 0x91 {
          self.cpu_read8(base_addr & 0xff00 | self.cpu.op_addr & 0xff);
        }
      }
    }
  }

  fn set_zn(&mut self, res: u8) {
    self.cpu.p.set(Status::Zero, res == 0);
    self.cpu.p.set(Status::Negative, res & 0x80 != 0);
  }

  fn get_op_val(&mut self) -> u8 {
    match self.cpu.op_val {
      Some(val) => val,
      None => self.cpu_read8(self.cpu.op_addr)
    }
  }

  fn set_op_res(&mut self, old: u8, res: u8) {
    match self.cpu.op_val {
      Some(_) => self.cpu.a = res,
      None => {
        // Read-modify-write instructions perform a dummy write during the "modify" stage and thus take 1 extra cycle.
        self.cpu_write8(self.cpu.op_addr, old);
        self.cpu_write8(self.cpu.op_addr, res);
      }
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
    self.stack_push8((self.cpu.p | Status::Brk).bits());
  }
  fn pla(&mut self) {
    // Instructions that pop data from the stack take 2 extra cycles, since they also need to pre-increment the stack pointer.
    self.cpu_tick();
    
    let res = self.stack_pop8();
    self.set_zn(res);
    self.cpu.a = res;
  }
  fn plp(&mut self) {
    // Instructions that pop data from the stack take 2 extra cycles, since they also need to pre-increment the stack pointer.
    self.cpu_tick();
    
    // https://www.nesdev.org/wiki/Instruction_reference#PLP
    // TODO: The effect of changing IrqDisable flag is delayed 1 instruction. 
    let res = self.stack_pop8();
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
    self.cpu.p.set(Status::Zero, res == 0);
    self.cpu.p.set(Status::Overflow, val & 0x40 != 0);
    self.cpu.p.set(Status::Negative, val & 0x80 != 0);
  }

  fn addition(&mut self, val: u8) {
    let res = self.cpu.a as u16 
      + val as u16
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
    let val = self.get_op_val();
    let res = val.wrapping_add(1);
    self.set_zn(res);
    self.set_op_res(val, res);
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
    let val = self.get_op_val();
    let res = val.wrapping_sub(1);
    self.set_zn(res);
    self.set_op_res(val, res);
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

  fn shift<F: FnOnce(u8) -> u8>(&mut self, val: u8, op: F, carry_bit: u8) -> u8 {
    let res = op(val);
    self.cpu.p.set(Status::Carry, val.shr(carry_bit) & 1 == 1);
    self.set_zn(res);
    self.set_op_res(val, res);
    res
  }

  fn asl_base(&mut self, val: u8) -> u8 {
    self.shift(val, |x| x.shl(1), 7)
  }
  fn lsr_base(&mut self, val: u8) -> u8 {
    self.shift(val, |x| x.shr(1), 0)
  }
  fn rol_base(&mut self, val: u8) -> u8 {
    let carry = self.cpu.p.contains(Status::Carry) as u8;
    self.shift(val, |x| x.shl(1) | carry, 7)
  }
  fn ror_base(&mut self, val: u8) -> u8 {
    let carry = self.cpu.p.contains(Status::Carry) as u8;
    self.shift(val, |x| x.shr(1) | (carry << 7), 0)
  }

  fn asl(&mut self) {
    let val = self.get_op_val();
    self.asl_base(val);
  }
  fn lsr(&mut self) {
    let val = self.get_op_val();
    self.lsr_base(val);
  }
  fn rol(&mut self) {
    let val = self.get_op_val();
    self.rol_base(val);
  }
  fn ror(&mut self) {
    let val = self.get_op_val();
    self.ror_base(val);
  }

  fn jmp(&mut self) {
    self.cpu.pc = self.cpu.op_addr;
  }
  fn jsr(&mut self) {
    // this has an extra cycle for internal operation (done for last here)
    // 3  $0100,S  R  internal operation (predecrement S?)

    self.stack_push16(self.cpu.pc.wrapping_sub(1));
    self.jmp();

    self.cpu_tick();
  }
  fn rts(&mut self) {
    // Instructions that pop data from the stack take 2 extra cycles, since they also need to pre-increment the stack pointer.
    self.cpu_tick();
    
    self.cpu.pc = self.stack_pop16().wrapping_add(1);

    // https://www.nesdev.org/wiki/Cycle_counting#Instruction_timings
    // plus 1 cycle to post-increment the program counter (to compensate for the off-by-1 address pushed by JSR).
    self.cpu_tick();
  }

  fn branch(&mut self, cond: bool) {
    if cond {
      // if branch is taken, costs 1 cycle more
      self.cpu_tick();

      // this is always Some
      let val = self.cpu.op_val.unwrap();
      let res = self.cpu.pc
        .wrapping_add_signed((val as i8) as i16);

      // if branch occurs to different page, costs 1 cycle more
      if res & 0xff00 != self.cpu.pc & 0xff00 {
        self.cpu_tick();
      }

      self.cpu.pc = res;
    }
  }
  fn bcc(&mut self) {
    self.branch(!self.cpu.p.contains(Status::Carry));
  }
  fn bcs(&mut self) {
    self.branch(self.cpu.p.contains(Status::Carry));
  }
  fn beq(&mut self) {
    self.branch(self.cpu.p.contains(Status::Zero));
  }
  fn bmi(&mut self) {
    self.branch(self.cpu.p.contains(Status::Negative));
  }
  fn bne(&mut self) {
    self.branch(!self.cpu.p.contains(Status::Zero));
  }
  fn bpl(&mut self) {
    self.branch(!self.cpu.p.contains(Status::Negative));
  }
  fn bvc(&mut self) {
    self.branch(!self.cpu.p.contains(Status::Overflow));
  }
  fn bvs(&mut self) {
    self.branch(self.cpu.p.contains(Status::Overflow));
  }

  fn clc(&mut self) {
    self.cpu.p.remove(Status::Carry);
  }
  fn cld(&mut self) {
    self.cpu.p.remove(Status::Decimal);
  }
  fn cli(&mut self) {
    // https://www.nesdev.org/wiki/Instruction_reference#CLI
    // TODO: The effect of changing this flag is delayed 1 instruction. 
    self.cpu.p.remove(Status::IrqDisable);
  }
  fn clv(&mut self) {
    self.cpu.p.remove(Status::Overflow);
  }
  fn sec(&mut self) {
    self.cpu.p.insert(Status::Carry);
  }
  fn sed(&mut self) {
    self.cpu.p.insert(Status::Decimal);
  }
  fn sei(&mut self) {
    // https://www.nesdev.org/wiki/Instruction_reference#SEI
    // TODO: The effect of changing this flag is delayed 1 instruction. 
    self.cpu.p.insert(Status::IrqDisable);
  }

  fn brk(&mut self) {
    self.stack_push16(self.cpu.pc.wrapping_add(1));
    self.php();
    self.cpu.p.insert(Status::IrqDisable);
    self.cpu.pc = self.cpu_read16(IRQ_VECTOR);
  }
  fn rti(&mut self) {
    // extra pull cycle is done in plp
    self.plp();
    self.cpu.pc = self.stack_pop16();
  }

  fn nop(&mut self) {
    // NOP reads from effective address with zeropage, absolute and indexed addressing
    self.get_op_val();
  }

  
  fn lax(&mut self) {
    // self.lda();
    // self.ldx();

    // A <- M, X <- M
    let res = self.get_op_val();
    self.set_zn(res);
    self.cpu.a = res;
    self.cpu.x = res;
  }

  fn sax(&mut self) {
    // M <- A AND X
    let res = self.cpu.a & self.cpu.x;
    self.cpu_write8(self.cpu.op_addr, res);
  }

  fn dcp(&mut self) {
    // self.dec();
    // self.cmp();

    // M <- M - 1, then A - M
    let val = self.get_op_val();
    let res = val.wrapping_sub(1);
    self.set_op_res(val, res);

    let res = self.cpu.a.wrapping_sub(res);

    // self.cpu.p = bit_change(self.cpu.p, flags::Carry, a >= b);
    self.cpu.p.set(Status::Carry, self.cpu.a >= res);
    self.set_zn(res);
  }

  fn isb(&mut self) {
    // self.inc();
    // self.sbc();

    // M <- M + 1, then A <- A - M - C-
    let val = self.get_op_val();
    let res = val.wrapping_add(1);
    self.set_op_res(val, res);

    self.addition(!res);
  }

  fn slo(&mut self) {
    // self.asl();
    // self.ora();

    // M = C <- [76543210] <- 0, A OR M -> A
    let val = self.get_op_val();
    let res = self.asl_base(val);

    let res = self.cpu.a | res;
    self.set_zn(res);
    self.cpu.a = res;
  }

  fn rla(&mut self) {
    // self.rol();
    // self.and();

    // M = C <- [76543210] <- C, A AND M -> A
    let val = self.get_op_val();
    let res = self.rol_base(val);

    let res = self.cpu.a & res;
    self.set_zn(res);
    self.cpu.a = res;
  }

  fn sre(&mut self) {
    // self.lsr();
    // self.eor();

    // M = 0 -> [76543210] -> C, A EOR M -> A
    let val = self.get_op_val();
    let res = self.lsr_base(val);

    let res = self.cpu.a ^ res;
    self.set_zn(res);
    self.cpu.a = res;
  }

  fn rra(&mut self) {
    // self.ror();
    // self.adc();

    // M = C -> [76543210] -> C, A + M + C -> A, C
    let val = self.get_op_val();
    let res = self.ror_base(val);

    self.addition(res);
  }

  fn anc(&mut self) {
    let val = self.get_op_val();
    let res = self.cpu.a & val;
    self.cpu.p.set(Status::Carry, res >> 7 == 1);
    self.set_zn(res);
    self.cpu.a = res;
  }

  fn alr(&mut self) {
    let val = self.get_op_val();
    let res = self.lsr_base(self.cpu.a & val);
    self.cpu.a = res;
  }

  fn arr(&mut self) {
    let val = self.get_op_val();
    let res = self.ror_base(self.cpu.a & val);
    let bit6 = res & 0b0100_0000 != 0;
    let bit5 = res & 0b0010_0000 != 0;
    self.cpu.p.set(Status::Carry, bit6);
    self.cpu.p.set(Status::Overflow, bit6 ^ bit5);
    self.cpu.a = res;
  }

  fn las(&mut self) {
    // self.lda();
    // self.tsx();

    // M AND SP -> A, X, SP
    let val = self.get_op_val();
    let res = val & self.cpu.sp;
    self.set_zn(res);

    self.cpu.a = res;
    self.cpu.x = res;
    self.cpu.sp = res;
  }

  fn sbx(&mut self) {
    // self.cmp();
    // self.dex();

    // (A AND X) - oper -> X
    let b = self.get_op_val();
    let a = self.cpu.a & self.cpu.x;
    let res = a.wrapping_sub(b);

    self.cpu.p.set(Status::Carry, a >= b);
    self.set_zn(res);
    self.cpu.x = res;
  }

  fn jam(&mut self) {
    panic!("===[SYSTEM JAMMED]===\n{:?}", self.cpu);
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

      0x02 => self.jam(),
      0x03 => self.slo(),
      0x04 => self.nop(),
      0x07 => self.slo(),
      0x0b => self.anc(),
      0x0c => self.nop(),
      0x0f => self.slo(),
      0x12 => self.jam(),
      0x13 => self.slo(),
      0x14 => self.nop(),
      0x17 => self.slo(),
      0x1a => self.nop(),
      0x1b => self.slo(),
      0x1c => self.nop(),
      0x1f => self.slo(),
      0x22 => self.jam(),
      0x23 => self.rla(),
      0x27 => self.rla(),
      0x2b => self.anc(),
      0x2f => self.rla(),
      0x32 => self.jam(),
      0x33 => self.rla(),
      0x34 => self.nop(),
      0x37 => self.rla(),
      0x3a => self.nop(),
      0x3b => self.rla(),
      0x3c => self.nop(),
      0x3f => self.rla(),
      0x42 => self.jam(),
      0x43 => self.sre(),
      0x44 => self.nop(),
      0x47 => self.sre(),
      0x4b => self.alr(),
      0x4f => self.sre(),
      0x52 => self.jam(),
      0x53 => self.sre(),
      0x54 => self.nop(),
      0x57 => self.sre(),
      0x5a => self.nop(),
      0x5b => self.sre(),
      0x5c => self.nop(),
      0x5f => self.sre(),
      0x62 => self.jam(),
      0x63 => self.rra(),
      0x64 => self.nop(),
      0x67 => self.rra(),
      0x6b => self.arr(),
      0x6f => self.rra(),
      0x72 => self.jam(),
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
      0x92 => self.jam(),
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
      0xb2 => self.jam(),
      0xb3 => self.lax(),
      0xb7 => self.lax(),
      0xbb => self.las(),
      0xbf => self.lax(),
      0xc2 => self.nop(),
      0xc3 => self.dcp(),
      0xc7 => self.dcp(),
      0xcb => self.sbx(),
      0xcf => self.dcp(),
      0xd2 => self.jam(),
      0xd3 => self.dcp(),
      0xd4 => self.nop(),
      0xd7 => self.dcp(),
      0xda => self.nop(),
      0xdb => self.dcp(),
      0xdc => self.nop(),
      0xdf => self.dcp(),
      0xe2 => self.nop(),
      0xe3 => self.isb(),
      0xe7 => self.isb(),
      0xeb => self.sbc(),
      0xef => self.isb(),
      0xf2 => self.jam(),
      0xf3 => self.isb(),
      0xf4 => self.nop(),
      0xf7 => self.isb(),
      0xfa => self.nop(),
      0xfb => self.isb(),
      0xfc => self.nop(),
      0xff => self.isb(),
      _ => unreachable!("illegal opcode 0x{opcode:02X} at address 0x{:04X} reached, system jammed", self.cpu.pc)
    }
  }
}