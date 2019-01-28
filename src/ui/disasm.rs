use super::EmuState;
use super::GameBoy;

use imgui::{ImGuiCond, StyleVar, Ui};

pub struct DisasmWindow {
    disasm: Vec<String>,
}

impl DisasmWindow {
    pub fn new(emu: &GameBoy) -> DisasmWindow {
        let mut disasm = Vec::with_capacity(0x4000);
        let mut addr = 0u16;

        while addr < 0x4000 {
            let (s, sz) = emu.cpu().disasm(emu.bus(), addr);
            addr += u16::from(sz);
            disasm.push(s);
        }

        DisasmWindow { disasm }
    }

    pub fn draw(&self, ui: &Ui, state: &EmuState) -> bool {
        ui.with_style_var(
            StyleVar::Alpha(if state.running { 0.65 } else { 1.0 }),
            || {
                ui.window(im_str!("ROM00 disassembly"))
                    .size((300.0, 740.0), ImGuiCond::FirstUseEver)
                    .position((10.0, 10.0), ImGuiCond::FirstUseEver)
                    .build(|| {
                        for s in self.disasm.iter() {
                            ui.text(s);
                        }
                    });
            },
        );

        true
    }
}
