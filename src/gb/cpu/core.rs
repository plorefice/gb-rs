use super::bus::{MemRW, MemSize};

pub struct CPU {
    pub af: u16,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub sp: u16,

    pub prev_pc: u16,
    pub pc: u16,

    pub intr_enabled: bool,
    pub halted: bool,
    pub clk: u128,
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            af: 0x00,
            bc: 0x00,
            de: 0x00,
            hl: 0x00,
            sp: 0x00,

            prev_pc: 0x00,
            pc: 0x00,

            intr_enabled: false,
            halted: false,
            clk: 0,
        }
    }

    pub fn exec(&mut self, bus: &mut impl MemRW) {
        self.prev_pc = self.pc;

        let opc = self.fetch_pc(bus);
        self.op(bus, opc);
    }

    pub fn fetch_pc<T: MemSize>(&mut self, bus: &mut impl MemRW) -> T {
        let v = self.fetch(bus, self.pc);
        self.pc += u16::from(T::byte_size());
        v
    }

    pub fn fetch<T: MemSize>(&mut self, bus: &mut impl MemRW, addr: u16) -> T {
        let v = bus.read::<T>(addr);
        self.clk += u128::from(T::byte_size() * 4);
        v
    }

    pub fn store<T: MemSize>(&mut self, bus: &mut impl MemRW, addr: u16, val: T) {
        bus.write::<T>(addr, val);
        self.clk += u128::from(T::byte_size() * 4);
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
