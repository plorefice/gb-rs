use super::gb::GameBoy;

mod ctx;
mod debug;
mod disasm;

use ctx::UiContext;
use debug::DebuggerWindow;
use disasm::DisasmWindow;

use glium::{
    backend::Facade,
    texture::{ClientFormat, RawImage2d},
    Texture2d,
};
use imgui::{ImGuiCond, Ui};

use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

const EMU_X_RES: usize = 160;
const EMU_Y_RES: usize = 144;

pub struct EmuState {
    gb: GameBoy,

    running: bool,
    step_into: bool,
}

impl EmuState {
    fn with(gb: GameBoy) -> EmuState {
        EmuState {
            gb,

            running: false,
            step_into: false,
        }
    }
}

pub struct EmuUi {
    ui_ctx: Rc<RefCell<UiContext>>,

    state: EmuState,
    vpu_texture: Option<imgui::ImTexture>,

    disasm: DisasmWindow,
    debugger: DebuggerWindow,
}

impl EmuUi {
    pub fn new(emu: GameBoy) -> EmuUi {
        let state = EmuState::with(emu);

        let disasm = DisasmWindow::new(&state);
        let debugger = DebuggerWindow::new();

        EmuUi {
            ui_ctx: Rc::from(RefCell::new(UiContext::new())),

            state,
            vpu_texture: None,

            disasm,
            debugger,
        }
    }

    pub fn run(&mut self) {
        let mut last_frame = Instant::now();
        let mut vbuf = vec![0; EMU_X_RES * EMU_Y_RES * 4];

        loop {
            let ui_ctx = self.ui_ctx.clone();
            let mut ui_ctx = ui_ctx.borrow_mut();

            ui_ctx.poll_events();
            if ui_ctx.should_quit() {
                break;
            }

            let now = Instant::now();
            let delta = now - last_frame;
            let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
            last_frame = now;

            if self.state.step_into {
                self.state.gb.single_step();
            } else if self.state.running {
                self.state.gb.run_to_vblank();
            }

            self.state.gb.rasterize(&mut vbuf[..]);

            let new_screen = Texture2d::new(
                ui_ctx.display.get_context(),
                RawImage2d {
                    data: Cow::Borrowed(&vbuf[..]),
                    width: EMU_X_RES as u32,
                    height: EMU_Y_RES as u32,
                    format: ClientFormat::U8U8U8U8,
                },
            )
            .unwrap();

            if let Some(texture) = self.vpu_texture {
                ui_ctx.renderer.textures().replace(texture, new_screen);
            } else {
                self.vpu_texture = Some(ui_ctx.renderer.textures().insert(new_screen));
            }

            if !ui_ctx.render(delta_s, |ui| self.draw(ui)) {
                break;
            }
        }
    }

    fn draw(&mut self, ui: &Ui) -> bool {
        self.draw_screen(ui);
        self.disasm.draw(ui, &mut self.state);
        self.debugger.draw(ui, &mut self.state);
        true
    }

    fn draw_screen(&mut self, ui: &Ui) {
        ui.window(im_str!("Screen"))
            .size(
                (EMU_X_RES as f32 + 15.0, EMU_Y_RES as f32 + 40.0),
                ImGuiCond::FirstUseEver,
            )
            .position((780.0, 10.0), ImGuiCond::FirstUseEver)
            .resizable(false)
            .build(|| {
                if let Some(texture) = self.vpu_texture {
                    ui.image(texture, (EMU_X_RES as f32, EMU_Y_RES as f32))
                        .build();
                }
            });
    }
}
