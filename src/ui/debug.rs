use super::utils;
use super::EmuState;

use imgui::{ImGuiCol, ImGuiCond, ImString, ImVec2, Ui};

pub struct DebuggerWindow;

impl DebuggerWindow {
    pub fn new() -> DebuggerWindow {
        DebuggerWindow {}
    }

    pub fn draw(&self, ui: &Ui, state: &mut EmuState) {
        ui.window(im_str!("Debugger"))
            .size((450.0, 140.0), ImGuiCond::FirstUseEver)
            .position((320.0, 10.0), ImGuiCond::FirstUseEver)
            .build(|| {
                let cpu = state.gb.cpu();

                state.step_into = ui.button(im_str!("Step"), (50.0, 20.0)) && !state.running;
                ui.same_line(70.0);
                ui.checkbox(im_str!("Run"), &mut state.running);

                ui.separator();

                DebuggerWindow::print_reg(ui, (10.0, 60.0), "AF", cpu.af);
                DebuggerWindow::print_reg(ui, (10.0, 75.0), "BC", cpu.bc);
                DebuggerWindow::print_reg(ui, (10.0, 90.0), "DE", cpu.de);
                DebuggerWindow::print_reg(ui, (10.0, 105.0), "HL", cpu.hl);

                DebuggerWindow::print_reg(ui, (100.0, 60.0), "SP", cpu.sp);
                DebuggerWindow::print_reg(ui, (100.0, 75.0), "PC", cpu.pc);
                DebuggerWindow::print_flags(ui, state);

                ui.set_cursor_pos((0.0, 130.0));
                ui.separator();
            });
    }

    fn print_reg<P: Into<ImVec2>>(ui: &Ui, pos: P, s: &str, val: u16) {
        ui.set_cursor_pos(pos);
        ui.text(format!("{}: 0x{:04X}", s, val));
    }

    fn print_flags(ui: &Ui, state: &EmuState) {
        let cpu = state.gb.cpu();
        let bg_col = utils::text_bg_color(ui);

        for (i, (n, f)) in [
            ("Z", cpu.zf()),
            ("N", cpu.sf()),
            ("H", cpu.hc()),
            ("C", cpu.cy()),
        ]
        .iter()
        .enumerate()
        {
            let x = 100.0 + (i as f32 * 20.0);

            utils::text_with_bg(
                ui,
                (x, 105.0),
                ImString::new(*n),
                if *f { Some(bg_col) } else { None },
            );
        }
    }
}
