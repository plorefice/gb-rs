use super::EmuState;

use std::collections::BTreeMap;

use imgui::{ImGuiCond, StyleVar, Ui};

pub struct DisasmWindow {
    disasm: BTreeMap<u16, String>,
}

impl DisasmWindow {
    pub fn new(state: &EmuState) -> DisasmWindow {
        let mut disasm = BTreeMap::new();
        let mut addr = 0u16;

        while addr < 0x4000 {
            let (s, sz) = state.gb.cpu().disasm(state.gb.bus(), addr);
            disasm.insert(addr, s);

            addr += u16::from(sz);
        }

        DisasmWindow { disasm }
    }

    pub fn draw(&self, ui: &Ui, state: &mut EmuState) {
        ui.with_style_var(
            StyleVar::Alpha(if state.running { 0.65 } else { 1.0 }),
            || {
                ui.window(im_str!("ROM00 disassembly"))
                    .size((300.0, 740.0), ImGuiCond::FirstUseEver)
                    .position((10.0, 10.0), ImGuiCond::FirstUseEver)
                    .build(|| {
                        for (_, s) in self.disasm.iter() {
                            ui.text(s);
                        }
                    });
            },
        );
    }
}
