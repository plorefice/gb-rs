use super::dbg;
use super::mem::MemRW;
use super::opcodes::OPCODES;

use std::collections::HashSet;

#[derive(Debug, Clone, Copy)]
pub struct OpcodeInfo(
    pub &'static str,    // Mnemonic
    pub OperandLocation, // Destination
    pub OperandLocation, // Source
    pub u8,              // Size
    pub u8,              // Cycles if branch taken
    pub u8,              // Cycles if branch not taken
);

#[derive(Debug, Clone, Copy)]
pub enum MemoryAddressing {
    A16, // (a16)
    IO,  // ($ff00 + a8)
    C,   // ($ff00 + C)
    BC,  // (BC)
    DE,  // (DE)
    HL,  // (HL)
    SP,  // (SP)
}

#[derive(Debug, Clone, Copy)]
pub enum OperandLocation {
    Register,
    Immediate,
    Memory(MemoryAddressing),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CpuState {
    FetchOpcode,
    FetchByte0,
    FetchByte1,
    FetchMemory,
    Writeback,
    Delay(u8),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WritebackOp {
    Write8(u16, u8),
    Write16(u16, u16),
    Push(u16),
    Return,
}

#[derive(Clone)]
pub struct CPU {
    // Registers
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub sp: u16,
    pub pc: u16,

    // Misc
    pub halted: bool,
    pub halt_bug: bool,
    pub should_halt: bool,
    pub intr_enabled: bool,

    // Execution-related members
    pub state: CpuState,
    pub info: OpcodeInfo,
    pub opcode: u8,
    pub cb_mode: bool,
    pub operand: u16,
    pub write_op: Option<WritebackOp>,
    pub executing: bool,
    pub branch_taken: bool,
    pub remaining_cycles: u8,

    // Debug
    paused: bool,
    breakpoints: HashSet<u16>,

    // Hacks/workarounds
    ignore_next_halt: bool,
}

impl Default for CPU {
    fn default() -> CPU {
        CPU {
            af: 0,
            bc: 0,
            de: 0,
            hl: 0,
            sp: 0,
            pc: 0,

            halted: false,
            halt_bug: false,
            should_halt: false,
            intr_enabled: false,

            state: CpuState::FetchOpcode,
            info: OPCODES[0],
            opcode: 0,
            cb_mode: false,
            operand: 0,
            write_op: None,
            executing: false,
            branch_taken: false,
            remaining_cycles: 0,

            paused: false,
            breakpoints: HashSet::new(),

            ignore_next_halt: false,
        }
    }
}

impl CPU {
    pub fn new() -> CPU {
        CPU::default()
    }

    pub fn tick(&mut self, bus: &mut impl MemRW) -> Result<(), dbg::TraceEvent> {
        use CpuState::*;

        let saved_ctx = self.clone();

        self.remaining_cycles -= 4;

        let res = match self.state {
            FetchOpcode => self.fetch_opcode(bus),
            FetchByte0 => self.fetch_immediate(bus),
            FetchByte1 => self.fetch_immediate(bus),
            FetchMemory => self.fetch_memory(bus),
            Writeback => self.writeback(bus),
            Delay(0) => {
                self.state = CpuState::FetchOpcode;
                self.executing = false;
                Ok(())
            }
            Delay(n) => {
                self.state = CpuState::Delay(n - 1);
                Ok(())
            }
        };

        // // The HALT bug prevents PC from being incremented on the instruction
        // // following a HALT, under certain conditions.
        // if self.halt_bug {
        //     self.halt_bug = false;
        //     self.pc = saved_ctx.pc;
        // }

        match res {
            Err(dbg::TraceEvent::CgbSpeedSwitchReq) => {
                // A speed switch in CGB is followed by a STOP which should be ignored.
                // Some ROMs (eg. Blargg's test ROMs) might call this on DMG, in which
                // case it should be ignored.
                self.ignore_next_halt = true;
                Ok(())
            }
            Err(e) => {
                // Restore previous state on error. Note that this is for debugging purposes only,
                // the side effects of the instruction (eg. memory writes) are NOT rolled back.
                *self = saved_ctx;
                Err(e)
            }
            Ok(()) => {
                // See above for the CGB workaround
                if self.should_halt && self.ignore_next_halt {
                    self.ignore_next_halt = false;
                    self.should_halt = false;
                }
                Ok(())
            }
        }
    }

    fn fetch_opcode(&mut self, bus: &mut impl MemRW) -> Result<(), dbg::TraceEvent> {
        // Handle breakpoints at the current position
        if !self.paused() && self.breakpoints.contains(&self.pc) {
            self.pause();
            return Err(dbg::TraceEvent::Breakpoint(self.pc));
        } else {
            self.resume();
        }

        // Fetch opcode and reset internal state
        self.opcode = self.fetch_pc(bus)?;
        self.info = OPCODES[self.opcode as usize];
        self.operand = 0;
        self.cb_mode = self.opcode == 0xCB;
        self.write_op = None;
        self.executing = true;
        self.branch_taken = false;
        self.remaining_cycles = self.info.5 - 4;

        // Check if we need to fetch more bytes, otherwise execute directly
        if self.info.3 > 1 {
            self.state = CpuState::FetchByte0;
        } else if let OperandLocation::Memory(_) = self.info.2 {
            self.state = CpuState::FetchMemory;
        } else {
            return self.exec();
        }

        Ok(())
    }

    fn fetch_immediate(&mut self, bus: &mut impl MemRW) -> Result<(), dbg::TraceEvent> {
        use MemoryAddressing::*;
        use OperandLocation::*;

        let d8 = self.fetch_pc(bus)?;

        match self.state {
            CpuState::FetchByte0 => {
                self.operand |= u16::from(d8);

                // Handle CB opcodes that fetch from memory
                if self.cb_mode {
                    self.opcode = self.operand as u8;

                    if self.operand & 0x7 == 0x6 {
                        self.info.2 = Memory(HL);
                        self.remaining_cycles += 8;
                    }
                }

                // Check if we need to fetch more bytes, otherwise execute directly
                if self.info.3 > 2 {
                    self.state = CpuState::FetchByte1;
                } else if let Memory(_) = self.info.2 {
                    self.state = CpuState::FetchMemory;
                } else {
                    return self.exec();
                }
            }
            CpuState::FetchByte1 => {
                self.operand |= u16::from(d8) << 8;

                // Check if we need to fetch more, otherwise execute directly
                if let Memory(_) = self.info.2 {
                    self.state = CpuState::FetchMemory;
                } else {
                    return self.exec();
                }
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    fn fetch_memory(&mut self, bus: &mut impl MemRW) -> Result<(), dbg::TraceEvent> {
        use MemoryAddressing::*;
        use OperandLocation::*;

        // Operand location in memory is codified in the opcode.
        // This handles all possible memory addressings.
        self.operand = match self.info.2 {
            Memory(C) => bus.read::<u8>(0xFF00 + u16::from(self.c()))?.into(),
            Memory(IO) => bus.read::<u8>(0xFF00 + self.operand)?.into(),
            Memory(BC) => bus.read::<u8>(self.bc)?.into(),
            Memory(DE) => bus.read::<u8>(self.de)?.into(),
            Memory(HL) => bus.read::<u8>(self.hl)?.into(),
            Memory(A16) => bus.read::<u8>(self.operand)?.into(),
            Memory(SP) => {
                let r = bus.read::<u16>(self.sp)?;
                self.sp += 2;
                r
            }
            _ => unreachable!(),
        };

        // After a memory fetch, there is always the execution step
        self.exec()
    }

    fn exec(&mut self) -> Result<(), dbg::TraceEvent> {
        // Execute operation
        if !self.cb_mode {
            self.op()?;
        } else {
            self.op_cb()?;
        }

        // Adjust remaining cycles based on the branching information
        if self.branch_taken {
            self.remaining_cycles += self.info.4 - self.info.5;
        }

        // If nothing needs to be written to memory, we are done
        if self.write_op.is_some() {
            self.state = CpuState::Writeback;
        } else if self.remaining_cycles > 0 {
            self.state = CpuState::Delay((self.remaining_cycles - 1) / 4)
        } else {
            self.state = CpuState::FetchOpcode;
            self.executing = false;
        }

        Ok(())
    }

    fn writeback(&mut self, bus: &mut impl MemRW) -> Result<(), dbg::TraceEvent> {
        use WritebackOp::*;

        // After a writeback operation, reset state machine for the next instruction
        if self.remaining_cycles > 0 {
            self.state = CpuState::Delay((self.remaining_cycles - 1) / 4);
        } else {
            self.state = CpuState::FetchOpcode;
            self.executing = false;
        }

        match self.write_op {
            Some(Write8(dest, d8)) => bus.write::<u8>(dest, d8),
            Some(Write16(dest, d16)) => bus.write::<u16>(dest, d16),
            Some(Push(d16)) => {
                self.sp -= 2;
                bus.write::<u16>(self.sp, d16)
            }
            Some(Return) => {
                // This is basically a POP PC operation
                self.pc = bus.read::<u16>(self.sp)?;
                self.sp += 2;
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub fn jump_to_isr(&mut self, bus: &mut impl MemRW, addr: u16) -> Result<(), dbg::TraceEvent> {
        // Push PC onto the stack
        self.sp -= 2;
        bus.write::<u16>(self.sp, self.pc)?;

        // Jump to ISR
        self.pc = addr;
        Ok(())
    }

    pub fn fetch_pc(&mut self, bus: &mut impl MemRW) -> Result<u8, dbg::TraceEvent> {
        let v = bus.read::<u8>(self.pc)?;
        self.pc += 1;
        Ok(v)
    }

    fn resume(&mut self) {
        self.paused = false;
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn paused(&self) -> bool {
        self.paused
    }

    pub fn set_breakpoint(&mut self, addr: u16) {
        self.breakpoints.insert(addr);
    }

    pub fn clear_breakpoint(&mut self, addr: u16) {
        self.breakpoints.remove(&addr);
    }

    pub fn breakpoint_at(&self, addr: u16) -> bool {
        self.breakpoints.contains(&addr)
    }

    pub fn breakpoints(&self) -> &HashSet<u16> {
        &self.breakpoints
    }
}

#[rustfmt::skip]
impl CPU {
    pub fn c(&self) -> u8 { self.bc as u8 }
    pub fn e(&self) -> u8 { self.de as u8 }
    pub fn l(&self) -> u8 { self.hl as u8 }
    pub fn a(&self) -> u8 { (self.af >> 8) as u8 }
    pub fn b(&self) -> u8 { (self.bc >> 8) as u8 }
    pub fn d(&self) -> u8 { (self.de >> 8) as u8 }
    pub fn h(&self) -> u8 { (self.hl >> 8) as u8 }
    pub fn f(&self) -> u8 { (self.af & 0x00F0) as u8 }

    pub fn set_c(&mut self, v: u8) { self.bc = (self.bc & 0xFF00) | u16::from(v); }
    pub fn set_e(&mut self, v: u8) { self.de = (self.de & 0xFF00) | u16::from(v); }
    pub fn set_l(&mut self, v: u8) { self.hl = (self.hl & 0xFF00) | u16::from(v); }
    pub fn set_f(&mut self, v: u8) { self.af = (self.af & 0xFF00) | u16::from(v & 0xF0); }
    pub fn set_a(&mut self, v: u8) { self.af = (self.af & 0x00FF) | (u16::from(v) << 8); }
    pub fn set_b(&mut self, v: u8) { self.bc = (self.bc & 0x00FF) | (u16::from(v) << 8); }
    pub fn set_d(&mut self, v: u8) { self.de = (self.de & 0x00FF) | (u16::from(v) << 8); }
    pub fn set_h(&mut self, v: u8) { self.hl = (self.hl & 0x00FF) | (u16::from(v) << 8); }

    pub fn zf(&self) -> bool { (self.f() & 0x80) != 0 }
    pub fn sf(&self) -> bool { (self.f() & 0x40) != 0 }
    pub fn hc(&self) -> bool { (self.f() & 0x20) != 0 }
    pub fn cy(&self) -> bool { (self.f() & 0x10) != 0 }

    pub fn set_zf(&mut self, v: bool) { self.set_f((self.f() & (!0x80)) | (u8::from(v) << 7)); }
    pub fn set_sf(&mut self, v: bool) { self.set_f((self.f() & (!0x40)) | (u8::from(v) << 6)); }
    pub fn set_hc(&mut self, v: bool) { self.set_f((self.f() & (!0x20)) | (u8::from(v) << 5)); }
    pub fn set_cy(&mut self, v: bool) { self.set_f((self.f() & (!0x10)) | (u8::from(v) << 4)); }
}
