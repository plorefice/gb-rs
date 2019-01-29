use super::{EmuState, Instruction};

use std::collections::BTreeMap;

use conrod_core::{color, widget, Colorable, Labelable, Positionable, Sizeable, UiCell, Widget};

pub struct DisasmWindow {
    window_id: widget::Id,

    disasm: BTreeMap<u16, Instruction>,
}

impl DisasmWindow {
    pub fn new(mut id_gen: widget::id::Generator, state: &EmuState) -> DisasmWindow {
        let mut dw = DisasmWindow {
            window_id: id_gen.next(),

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

    pub fn draw(&mut self, ui: &mut UiCell, state: &mut EmuState) {
        self.realign_disasm(state, state.gb.cpu().pc);

        let window = widget::Canvas::new().floating(true).w_h(300.0, 740.0);

        window
            .title_bar("ROM00 disassembly")
            .set(self.window_id, ui);

        // ui.window(im_str!("ROM00 disassembly"))
        //     .size((300.0, 740.0), ImGuiCond::FirstUseEver)
        //     .position((10.0, 10.0), ImGuiCond::FirstUseEver)
        //     .build(|| {
        //         for (addr, instr) in self.disasm.iter() {
        //             let text = format!(
        //                 "{:04X}\t{:02X} {:4}\t{}",
        //                 addr,
        //                 instr.opcode,
        //                 if let Some(imm) = instr.imm {
        //                     format!("{:04X}", imm)
        //                 } else {
        //                     "    ".to_string()
        //                 },
        //                 instr.mnemonic
        //             );

        //             if *addr == state.gb.cpu().pc {
        //                 let scroll_y = unsafe { imgui_sys::igGetScrollY() };
        //                 let (cx, cy) = ui.get_cursor_pos();

        //                 utils::text_with_bg(
        //                     ui,
        //                     (cx, cy - scroll_y),
        //                     ImString::new(text),
        //                     Some(utils::text_bg_color(ui)),
        //                 );
        //             } else {
        //                 ui.text(text);
        //             }
        //         }
        //     });
    }
}
