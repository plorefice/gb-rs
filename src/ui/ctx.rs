use conrod_core::{widget, Colorable, Positionable, Ui, UiBuilder, UiCell, Widget};
use conrod_glium::Renderer;

use glium::glutin;
use glium::glutin::{
    ElementState::Pressed, Event, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent::*,
};
use glium::{Display, Surface};

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub struct DisplayWinitWrapper(pub Display);

impl conrod_winit::WinitWindow for DisplayWinitWrapper {
    fn get_inner_size(&self) -> Option<(u32, u32)> {
        self.0.gl_window().get_inner_size().map(Into::into)
    }
    fn hidpi_factor(&self) -> f32 {
        self.0.gl_window().get_hidpi_factor() as _
    }
}

pub struct UiContext {
    pub ui: Ui,
    pub display: DisplayWinitWrapper,
    pub renderer: Renderer,

    pub events_loop: Rc<RefCell<glutin::EventsLoop>>,
    pub event_loop: EventLoop,

    should_quit: bool,
}

impl UiContext {
    pub fn new() -> UiContext {
        let events_loop = glutin::EventsLoop::new();

        let window = glutin::WindowBuilder::new()
            .with_title("gb-rs")
            .with_dimensions((1024, 768).into());

        let context = glutin::ContextBuilder::new()
            .with_vsync(true)
            .with_multisampling(4);

        let display = Display::new(window, context, &events_loop).unwrap();
        let display = DisplayWinitWrapper(display);

        let mut ui = UiBuilder::new([1024.0, 768.0]).build();

        // Add fonts

        let mut renderer = Renderer::new(&display.0).unwrap();

        UiContext {
            ui,
            display,
            renderer,
            events_loop: Rc::new(RefCell::from(events_loop)),
            event_loop: EventLoop::new(),

            should_quit: false,
        }
    }

    pub fn widget_ids_generator(&mut self) -> widget::id::Generator {
        self.ui.widget_id_generator()
    }

    pub fn handle_events(&mut self) {
        let events_loop = self.events_loop.clone();
        let mut events_loop = events_loop.borrow_mut();

        for event in self.event_loop.next(&mut events_loop) {
            // Use the `winit` backend feature to convert the winit event to a conrod one.
            if let Some(event) = conrod_winit::convert_event(event.clone(), &self.display) {
                self.ui.handle_event(event);
                self.event_loop.needs_update();
            }

            match event {
                glium::glutin::Event::WindowEvent { event, .. } => match event {
                    // Break from the loop upon `Escape`.
                    glium::glutin::WindowEvent::CloseRequested
                    | glium::glutin::WindowEvent::KeyboardInput {
                        input:
                            glium::glutin::KeyboardInput {
                                virtual_keycode: Some(glium::glutin::VirtualKeyCode::Escape),
                                ..
                            },
                        ..
                    } => self.should_quit = true,
                    _ => (),
                },
                _ => (),
            }
        }
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn render<F>(&mut self, mut f: F)
    where
        F: FnMut(&mut UiCell),
    {
        // Instantiate all GUI components
        {
            let ui = &mut self.ui.set_widgets();

            f(ui);
        }

        let image_map = conrod_core::image::Map::<glium::texture::Texture2d>::new();

        if let Some(primitives) = self.ui.draw_if_changed() {
            self.renderer.fill(&self.display.0, primitives, &image_map);

            let mut target = self.display.0.draw();
            target.clear_color(0.0, 0.0, 0.0, 1.0);
            self.renderer
                .draw(&self.display.0, &mut target, &image_map)
                .unwrap();
            target.finish().unwrap();
        }
    }
}

pub struct EventLoop {
    ui_needs_update: bool,
    last_update: Instant,
}

impl EventLoop {
    pub fn new() -> Self {
        EventLoop {
            last_update: Instant::now(),
            ui_needs_update: true,
        }
    }

    /// Produce an iterator yielding all available events.
    pub fn next(&mut self, events_loop: &mut glutin::EventsLoop) -> Vec<glutin::Event> {
        // We don't want to loop any faster than 60 FPS, so wait until it has been at least 16ms
        // since the last yield.
        let last_update = self.last_update;
        let sixteen_ms = Duration::from_millis(16);
        let duration_since_last_update = Instant::now().duration_since(last_update);
        if duration_since_last_update < sixteen_ms {
            std::thread::sleep(sixteen_ms - duration_since_last_update);
        }

        // Collect all pending events.
        let mut events = Vec::new();
        events_loop.poll_events(|event| events.push(event));

        // If there are no events and the `Ui` does not need updating, wait for the next event.
        if events.is_empty() && !self.ui_needs_update {
            events_loop.run_forever(|event| {
                events.push(event);
                glutin::ControlFlow::Break
            });
        }

        self.ui_needs_update = false;
        self.last_update = Instant::now();

        events
    }

    /// Notifies the event loop that the `Ui` requires another update whether or not there are any
    /// pending events.
    pub fn needs_update(&mut self) {
        self.ui_needs_update = true;
    }
}
