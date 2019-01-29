use super::{EmuState, Instruction};

use std::collections::BTreeMap;

use imgui::{ImGuiCond, Ui};

pub struct DisasmWindow {
    disasm: BTreeMap<u16, Instruction>,
}

impl DisasmWindow {
    pub fn new(state: &EmuState) -> DisasmWindow {
        let mut dw = DisasmWindow {
            disasm: BTreeMap::new(),
        };

        dw.realign_disasm(state, 0);
        dw
    }

    // If there is alread an instruction decoded at address `from`,
    // do nothing. Otherwise, fetch the instruction at from, invalidate
    // all the overlapping decoded instructions and update the view.
    fn realign_disasm(&mut self, state: &EmuState, mut from: u16) {
        let cpu = state.gb.cpu();
        let bus = state.gb.bus();

        while from < bus.rom_size() {
            let instr = cpu.disasm(bus, from);
            let next = from + u16::from(instr.size);

            if self.disasm.get(&from).is_some() {
                break;
            }
            for addr in from..next {
                self.disasm.remove(&addr);
            }

            self.disasm.insert(from, instr);
            from = next;
        }
    }

    pub fn draw(&mut self, ui: &Ui, state: &mut EmuState) {
        self.realign_disasm(state, state.gb.cpu().pc);

        ui.window(im_str!("ROM00 disassembly"))
            .size((300.0, 740.0), ImGuiCond::FirstUseEver)
            .position((10.0, 10.0), ImGuiCond::FirstUseEver)
            .build(|| {
                for (addr, instr) in self.disasm.iter() {
                    ui.text(format!("{:04X}", addr));
                    ui.same_line(70.0);
                    ui.text(format!("{:02X}", instr.opcode));
                    if let Some(imm) = instr.imm {
                        ui.same_line(100.0);
                        ui.text(format!("{:04X}", imm));
                    }
                    ui.same_line(160.0);
                    ui.text(instr.mnemonic);
                }
            });
    }
}
