#[macro_use]
extern crate imgui;
extern crate imgui_sys;

mod gb;
mod ui;

use gb::GameBoy;

use std::env;
use std::fs;
use std::process;

fn main() {
    let fname = match env::args().nth(1) {
        Some(fname) => fname,
        None => {
            println!("USAGE: chip8-sdl ROM-FILE");
            process::exit(1);
        }
    };

    let rom = match fs::read(&fname) {
        Ok(b) => b,
        Err(e) => {
            println!("could not open {}: {}", &fname, e);
            process::exit(1);
        }
    };

    ui::EmuUi::new(GameBoy::with_cartridge(&rom[..])).run();
}
