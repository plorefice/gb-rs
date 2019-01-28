use super::bus::Bus;
use super::cpu::CPU;

const CPU_CLOCK: u64 = 4_194_304; // Hz
const HSYNC_CLOCK: u64 = 9_198; // Hz

const CYCLES_PER_HSYNC: u64 = CPU_CLOCK / HSYNC_CLOCK;

pub struct GameBoy {
    cpu: CPU,
    bus: Bus,
}

impl GameBoy {
    pub fn with_cartridge(rom: &[u8]) -> GameBoy {
        GameBoy {
            cpu: CPU::new(),
            bus: Bus::new(rom),
        }
    }

    pub fn single_step(&mut self) {
        if !self.cpu.halted {
            self.cpu.exec(&mut self.bus);
        }
    }

    pub fn run_to_vblank(&mut self) {
        for _ in 0..154 {
            let until_clk = self.cpu.clk + u128::from(CYCLES_PER_HSYNC);

            while self.cpu.clk < until_clk && !self.cpu.halted {
                self.cpu.exec(&mut self.bus);
            }
            self.bus.ppu.hsync();
        }
    }

    pub fn rasterize(&self, vbuf: &mut [u8]) {
        self.bus.ppu.rasterize(vbuf);
    }

    pub fn cpu(&self) -> &CPU {
        &self.cpu
    }

    pub fn bus(&self) -> &Bus {
        &self.bus
    }
}
