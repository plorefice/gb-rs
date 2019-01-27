use super::bus::{MemR, MemRW, MemSize, MemW};

#[derive(Default, Copy, Clone)]
struct Tile([u8; 16]);

impl Tile {
    fn data(&self) -> &[u8] {
        &self.0[..]
    }

    fn data_mut(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }

    pub fn pixel(&self, x: u8, y: u8) -> u8 {
        let bl = self.0[usize::from(y) * 2];
        let bh = self.0[usize::from(y) * 2 + 1];
        (((bh >> (7 - x)) & 0x1) << 1) | ((bl >> (7 - x)) & 0x1)
    }
}

#[derive(Default, Copy, Clone)]
struct Sprite([u8; 4]);

impl Sprite {
    fn data(&self) -> &[u8] {
        &self.0[..]
    }

    fn data_mut(&mut self) -> &mut [u8] {
        &mut self.0[..]
    }
}

impl<'a> MemR for &'a [Sprite] {
    fn read<T: MemSize>(&self, addr: u16) -> T {
        let s = &self[usize::from(addr >> 2)];
        T::read_le(&s.data()[usize::from(addr % 2)..])
    }
}

impl<'a> MemR for &'a mut [Sprite] {
    fn read<T: MemSize>(&self, addr: u16) -> T {
        (&*self as &[Sprite]).read(addr)
    }
}

impl<'a> MemW for &'a mut [Sprite] {
    fn write<T: MemSize>(&mut self, addr: u16, val: T) {
        let s = &mut self[usize::from(addr >> 2)];
        T::write_le(&mut s.data_mut()[usize::from(addr % 2)..], val);
    }
}

impl<'a> MemRW for &'a mut [Sprite] {}

pub struct PPU {
    tdt: [Tile; 384],  // Tile Data Table
    oam: [Sprite; 40], // Object Attribute Memory
    bgtm0: [u8; 1024], // Background Tile Map #0
    bgtm1: [u8; 1024], // Background Tile Map #1

    regs: [u8; 48],
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            tdt: [Tile::default(); 384],
            oam: [Sprite::default(); 40],
            bgtm0: [0; 1024],
            bgtm1: [0; 1024],
            regs: [0; 48],
        }
    }

    pub fn rasterize(&self, vbuf: &mut [u8]) {
        if (self.lcdc() & 0x80) == 0 {
            for b in vbuf.iter_mut() {
                *b = 0xFF;
            }
        } else {
            for py in 0usize..144 {
                for px in 0usize..160 {
                    let y = (py + usize::from(self.scroll_y())) % 256;
                    let x = (px + usize::from(self.scroll_x())) % 256;

                    let pid = (py * (160 * 3)) + (px * 3);

                    let t = self.bg_tile(((y >> 3) << 5) + (x >> 3));
                    let px = t.pixel((x & 0x07) as u8, (y & 0x7) as u8);
                    let shade = self.shade(px);

                    vbuf[pid] = shade;
                    vbuf[pid + 1] = shade;
                    vbuf[pid + 2] = shade;
                }
            }
        }
    }

    pub fn hsync(&mut self) {
        self.regs[0x04] = (self.regs[0x04] + 1) % 154;
    }

    fn io_read<T: MemSize>(&self, idx: u16) -> T {
        T::read_le(&self.regs[usize::from(idx)..])
    }

    fn io_write<T: MemSize>(&mut self, idx: u16, v: T) {
        T::write_le(&mut self.regs[usize::from(idx)..], v);
    }

    fn lcdc(&self) -> u8 {
        self.regs[0x00]
    }

    fn bgp(&self) -> u8 {
        self.regs[0x07]
    }

    fn scroll_x(&self) -> u8 {
        self.regs[0x03]
    }

    fn scroll_y(&self) -> u8 {
        self.regs[0x02]
    }

    fn shade(&self, color: u8) -> u8 {
        match (self.bgp() >> (color * 2)) & 0x3 {
            0 => 0xFF,
            1 => 0xAA,
            2 => 0x55,
            3 => 0x00,
            _ => unreachable!(),
        }
    }

    fn bg_tile(&self, id: usize) -> &Tile {
        let tile_id = if (self.lcdc() & 0x08) == 0 {
            self.bgtm0[id]
        } else {
            self.bgtm1[id]
        };

        if (self.lcdc() & 0x10) == 0 {
            &self.tdt[(128 + i32::from(tile_id as i8)) as usize]
        } else {
            &self.tdt[usize::from(tile_id)]
        }
    }
}

impl MemR for PPU {
    fn read<T: MemSize>(&self, addr: u16) -> T {
        match addr {
            0x8000..=0x97FF => {
                let addr = addr - 0x8000;
                let tid = usize::from(addr >> 4);
                let bid = usize::from(addr & 0xF);
                T::read_le(&self.tdt[tid].data()[bid..])
            }
            0x9800..=0x9BFF => T::read_le(&self.bgtm0[usize::from(addr - 0x9800)..]),
            0x9C00..=0x9FFF => T::read_le(&self.bgtm1[usize::from(addr - 0x9C00)..]),
            0xFE00..=0xFE9F => (&self.oam[..]).read(addr - 0xFE00),
            0xFF40..=0xFF6F => self.io_read(addr - 0xFF40),
            _ => unreachable!(),
        }
    }
}

impl MemW for PPU {
    fn write<T: MemSize>(&mut self, addr: u16, val: T) {
        match addr {
            0x8000..=0x97FF => {
                let addr = addr - 0x8000;
                let tid = usize::from(addr >> 4);
                let bid = usize::from(addr & 0xF);
                T::write_le(&mut self.tdt[tid].data_mut()[bid..], val);
            }
            0x9800..=0x9BFF => T::write_le(&mut self.bgtm0[usize::from(addr - 0x9800)..], val),
            0x9C00..=0x9FFF => T::write_le(&mut self.bgtm1[usize::from(addr - 0x9C00)..], val),
            0xFE00..=0xFE9F => (&mut self.oam[..]).write(addr - 0xFE00, val),
            0xFF40..=0xFF6F => self.io_write(addr - 0xFF40, val),
            _ => unreachable!(),
        }
    }
}
