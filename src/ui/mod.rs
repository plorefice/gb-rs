use super::gb::*;

mod ctx;
use ctx::UiContext;

use conrod_core::{widget, Labelable, Positionable, Sizeable, UiCell, Widget};

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

widget_ids!(struct Ids { canvas, button });

pub struct EmuUi {
    ui_ctx: Rc<RefCell<UiContext>>,
    canvas: widget::Id,
    button: widget::Id,

    state: EmuState,
}

impl EmuUi {
    pub fn new(emu: GameBoy) -> EmuUi {
        let state = EmuState::with(emu);

        let mut ctx = UiContext::new();

        let canvas = ctx.widget_ids_generator().next();
        let button = ctx.widget_ids_generator().next();

        EmuUi {
            ui_ctx: Rc::from(RefCell::from(ctx)),
            canvas,
            button,

            state,
        }
    }

    pub fn run(&mut self) {
        let mut last_frame = Instant::now();
        let mut vbuf = vec![0; EMU_X_RES * EMU_Y_RES * 4];

        loop {
            let ui_ctx = self.ui_ctx.clone();
            let mut ui_ctx = ui_ctx.borrow_mut();

            ui_ctx.handle_events();
            if ui_ctx.should_quit() {
                break;
            }

            let now = Instant::now();
            let delta = now - last_frame;
            let delta_s = delta.as_secs() as f32 + delta.subsec_nanos() as f32 / 1_000_000_000.0;
            last_frame = now;

            ui_ctx.render(|ui| self.draw(ui))
        }
    }

    fn draw(&mut self, ui: &mut UiCell) {
        // Create a background canvas upon which we'll place the button.
        widget::Canvas::new().pad(40.0).set(self.canvas, ui);

        // Draw the button and increment `count` if pressed.
        widget::Button::new()
            .middle_of(self.canvas)
            .w_h(80.0, 80.0)
            .label("Fuffa")
            .set(self.button, ui);
    }
}
