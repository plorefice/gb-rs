use super::gb::GameBoy;

mod ctx;
mod disasm;

use ctx::UiContext;
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

pub struct EmuUi {
    emu: GameBoy,
    ui_ctx: Rc<RefCell<UiContext>>,
    vpu_texture: Option<imgui::ImTexture>,

    disasm: DisasmWindow,

    paused: bool,
}

impl EmuUi {
    pub fn new(emu: GameBoy) -> EmuUi {
        let disasm = DisasmWindow::new(&emu);

        EmuUi {
            emu,
            ui_ctx: Rc::from(RefCell::new(UiContext::new())),
            vpu_texture: None,

            disasm,

            paused: true,
        }
    }

    pub fn run(&mut self) {
        let mut last_frame = Instant::now();
        let mut vbuf = vec![0; EMU_X_RES * EMU_Y_RES * 4];

        loop {
            let ui_ctx_c = self.ui_ctx.clone();
            let mut ui_ctx = ui_ctx_c.borrow_mut();

            ui_ctx.poll_events();
            if ui_ctx.should_quit() {
                break;
            }

            let now = Instant::now();
            let delta = now - last_frame;
            let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
            last_frame = now;

            if !self.paused {
                self.emu.run_to_vblank();
                self.emu.rasterize(&mut vbuf[..]);

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
            }

            if !ui_ctx.render(delta_s, |ui| self.draw(ui)) {
                break;
            }
        }
    }

    fn draw(&mut self, ui: &Ui) -> bool {
        self.paused = !ui.button(
            if self.paused {
                im_str!("Run")
            } else {
                im_str!("Pause")
            },
            (50.0, 20.0),
        );

        ui.window(im_str!("Screen"))
            .size(
                (EMU_X_RES as f32 + 15.0, EMU_Y_RES as f32 + 40.0),
                ImGuiCond::FirstUseEver,
            )
            .resizable(false)
            .build(|| {
                if let Some(texture) = self.vpu_texture {
                    ui.image(texture, (EMU_X_RES as f32, EMU_Y_RES as f32))
                        .build();
                }
            });

        self.disasm.draw(ui)
    }
}
