use super::memory::Memory;
use super::sound::APU;
use super::video::PPU;

pub trait MemSize: Default {
    fn byte_size() -> u8;

    fn read_le(buf: &[u8]) -> Self;
    fn write_le(buf: &mut [u8], v: Self);
}

impl MemSize for u8 {
    fn byte_size() -> u8 {
        1
    }

    fn read_le(buf: &[u8]) -> u8 {
        buf[0]
    }

    fn write_le(buf: &mut [u8], v: u8) {
        buf[0] = v;
    }
}

impl MemSize for i8 {
    fn byte_size() -> u8 {
        1
    }

    fn read_le(buf: &[u8]) -> i8 {
        buf[0] as i8
    }

    fn write_le(buf: &mut [u8], v: i8) {
        buf[0] = v as u8;
    }
}

impl MemSize for u16 {
    fn byte_size() -> u8 {
        2
    }

    fn read_le(buf: &[u8]) -> u16 {
        (u16::from(buf[1]) << 8) | u16::from(buf[0])
    }

    fn write_le(buf: &mut [u8], v: u16) {
        buf[0] = (v & 0xFF) as u8;
        buf[1] = (v >> 8) as u8;
    }
}

pub trait MemR<T: MemSize> {
    fn read(&self, addr: u16) -> T;
}

pub trait MemW<T: MemSize> {
    fn write(&mut self, addr: u16, val: T);
}

pub trait MemRW<T: MemSize>: MemR<T> + MemW<T> {}

pub struct Bus {
    pub rom_00: Memory,
    pub rom_nn: Memory,

    pub eram: Memory,
    pub hram: Memory,
    pub wram_00: Memory,
    pub wram_nn: Memory,

    pub apu: APU,
    pub ppu: PPU,
}

impl Bus {
    pub fn new(rom: &[u8]) -> Bus {
        let mut rom_00 = Memory::new(0x4000);
        let mut rom_nn = Memory::new(0x4000);

        let mut roms = rom.chunks(0x4000);

        if let Some(chunk) = roms.nth(0) {
            for (i, b) in chunk.iter().enumerate() {
                rom_00.write(i as u16, *b);
            }
        }
        for chunk in roms {
            for (i, b) in chunk.iter().enumerate() {
                rom_nn.write(i as u16, *b);
            }
        }

        Bus {
            rom_00: rom_00,
            rom_nn: rom_nn,

            eram: Memory::new(0x2000),
            hram: Memory::new(127),
            wram_00: Memory::new(0x1000),
            wram_nn: Memory::new(0x1000),

            apu: APU::new(),
            ppu: PPU::new(),
        }
    }
}

impl<T: MemSize> MemR<T> for Bus {
    fn read(&self, addr: u16) -> T {
        match addr {
            0x0000..=0x3FFF => self.rom_00.read(addr),
            0x4000..=0x7FFF => self.rom_nn.read(addr - 0x4000),
            0x8000..=0x9FFF => self.ppu.read(addr),
            0xA000..=0xBFFF => self.eram.read(addr - 0xA000),
            0xC000..=0xCFFF => self.wram_00.read(addr - 0xC000),
            0xD000..=0xDFFF => self.wram_nn.read(addr - 0xD000),
            0xE000..=0xEFFF => self.wram_00.read(addr - 0xE000),
            0xF000..=0xFDFF => self.wram_nn.read(addr - 0xF000),
            0xFE00..=0xFE9F => self.ppu.read(addr),
            0xFF10..=0xFF3F => self.apu.read(addr - 0xFF10),
            0xFF40..=0xFF6F => self.ppu.read(addr),
            0xFF80..=0xFFFE => self.hram.read(addr - 0xFF80),
            _ => panic!("invalid memory address: 0x{:04X}", addr),
        }
    }
}

impl<T: MemSize> MemW<T> for Bus {
    fn write(&mut self, addr: u16, val: T) {
        match addr {
            0x0000..=0x3FFF => self.rom_00.write(addr, val),
            0x4000..=0x7FFF => self.rom_nn.write(addr - 0x4000, val),
            0x8000..=0x9FFF => self.ppu.write(addr, val),
            0xA000..=0xBFFF => self.eram.write(addr - 0xA000, val),
            0xC000..=0xCFFF => self.wram_00.write(addr - 0xC000, val),
            0xD000..=0xDFFF => self.wram_nn.write(addr - 0xD000, val),
            0xE000..=0xEFFF => self.wram_00.write(addr - 0xE000, val),
            0xF000..=0xFDFF => self.wram_nn.write(addr - 0xF000, val),
            0xFE00..=0xFE9F => self.ppu.write(addr, val),
            0xFF10..=0xFF3F => self.apu.write(addr - 0xFF10, val),
            0xFF40..=0xFF6F => self.ppu.write(addr, val),
            0xFF80..=0xFFFE => self.hram.write(addr - 0xFF80, val),
            _ => panic!("invalid memory address: 0x{:04X}", addr),
        }
    }
}
