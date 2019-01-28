use glium::glutin;
use glium::glutin::{
    ElementState::Pressed, Event, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent::*,
};
use glium::{Display, Surface};
use imgui::{FontGlyphRange, FrameSize, ImFontConfig, ImGui, Ui};
use imgui_glium_renderer::Renderer;

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Copy, Clone, PartialEq, Debug, Default)]
struct MouseState {
    pos: (i32, i32),
    pressed: (bool, bool, bool),
    wheel: f32,
}

pub struct UiContext {
    pub imgui: ImGui,
    pub display: Display,
    pub renderer: Renderer,
    pub events_loop: Rc<RefCell<glutin::EventsLoop>>,

    mouse_state: MouseState,
    should_quit: bool,
}

impl UiContext {
    pub fn new() -> UiContext {
        let events_loop = glutin::EventsLoop::new();

        let context = glutin::ContextBuilder::new().with_vsync(true);
        let builder = glutin::WindowBuilder::new()
            .with_title("gb-rs")
            .with_dimensions(glutin::dpi::LogicalSize::new(1024.0, 764.0));

        let display = Display::new(builder, context, &events_loop).unwrap();

        let mut imgui = ImGui::init();
        imgui.set_ini_filename(None);

        let hidpi_factor = display.gl_window().get_hidpi_factor().round();

        UiContext::load_fonts(&mut imgui, hidpi_factor);

        let renderer = Renderer::init(&mut imgui, &display).expect("Failed to initialize renderer");

        UiContext {
            imgui,
            display,
            renderer,
            events_loop: Rc::new(RefCell::from(events_loop)),

            mouse_state: MouseState::default(),
            should_quit: false,
        }
    }

    pub fn poll_events(&mut self) {
        let hidpi_factor = self.window().get_hidpi_factor();
        let events_loop = self.events_loop.clone();

        events_loop.borrow_mut().poll_events(|event| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    CloseRequested => {
                        self.should_quit = true;
                    }
                    CursorMoved { position: pos, .. } => {
                        self.mouse_state.pos = pos
                            .to_physical(hidpi_factor)
                            .to_logical(hidpi_factor)
                            .into();
                    }
                    MouseInput { state, button, .. } => match button {
                        MouseButton::Left => self.mouse_state.pressed.0 = state == Pressed,
                        MouseButton::Right => self.mouse_state.pressed.1 = state == Pressed,
                        MouseButton::Middle => self.mouse_state.pressed.2 = state == Pressed,
                        _ => {}
                    },
                    MouseWheel {
                        delta: MouseScrollDelta::LineDelta(_, y),
                        phase: TouchPhase::Moved,
                        ..
                    } => self.mouse_state.wheel = y,
                    MouseWheel {
                        delta: MouseScrollDelta::PixelDelta(pos),
                        phase: TouchPhase::Moved,
                        ..
                    } => {
                        self.mouse_state.wheel =
                            pos.to_physical(hidpi_factor).to_logical(hidpi_factor).y as f32;
                    }
                    _ => (),
                }
            }
        });

        self.update_imgui_mouse();
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn render<F>(&mut self, delta_s: f32, mut f: F) -> bool
    where
        F: FnMut(&Ui) -> bool,
    {
        let window = self.display.gl_window();
        let hidpi_factor = window.get_hidpi_factor();

        let physical_size = window.get_inner_size().unwrap().to_physical(hidpi_factor);
        let logical_size = physical_size.to_logical(hidpi_factor);

        let frame_size = FrameSize {
            logical_size: logical_size.into(),
            hidpi_factor,
        };

        let ui = self.imgui.frame(frame_size, delta_s);
        if !f(&ui) {
            return false;
        }

        let mut target = self.display.draw();
        target.clear_color(0.4, 0.5, 0.6, 1.0);
        self.renderer
            .render(&mut target, ui)
            .expect("Rendering failed");
        target.finish().unwrap();

        true
    }

    fn window(&self) -> std::cell::Ref<'_, glutin::GlWindow> {
        self.display.gl_window()
    }

    fn load_fonts(imgui: &mut ImGui, hidpi_factor: f64) {
        let font_size = 13.0 * hidpi_factor as f32;

        imgui.fonts().add_font_with_config(
            include_bytes!("../../res/mplus-1p-regular.ttf"),
            ImFontConfig::new()
                .oversample_h(1)
                .pixel_snap_h(true)
                .size_pixels(font_size)
                .rasterizer_multiply(1.75),
            &FontGlyphRange::japanese(),
        );

        imgui.fonts().add_default_font_with_config(
            ImFontConfig::new()
                .merge_mode(true)
                .oversample_h(1)
                .pixel_snap_h(true)
                .size_pixels(font_size),
        );

        imgui.set_font_global_scale(1.0 / hidpi_factor as f32);
    }

    fn update_imgui_mouse(&mut self) {
        self.imgui
            .set_mouse_pos(self.mouse_state.pos.0 as f32, self.mouse_state.pos.1 as f32);
        self.imgui.set_mouse_down([
            self.mouse_state.pressed.0,
            self.mouse_state.pressed.1,
            self.mouse_state.pressed.2,
            false,
            false,
        ]);
        self.imgui.set_mouse_wheel(self.mouse_state.wheel);
        self.mouse_state.wheel = 0.0;
    }
}
