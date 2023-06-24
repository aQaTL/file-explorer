#![cfg_attr(feature = "windows_subsystem", windows_subsystem = "windows")]

use std::{io, ops::ControlFlow, time::Instant};

use log::error;

use crate::window::Window;

mod string;
mod window;

fn main() {
	aqa_logger::init();
	if let Err(err) = main_() {
		error!("error: {err}");
		std::process::exit(1);
	}
}

fn main_() -> Result<(), io::Error> {
	let mut window = Window::open()?;

	let (mut x_offset, y_offset) = (0, 0);

	//let mut start = Instant::now();

	while let ControlFlow::Continue(_) = window.process_messages() {
		window.render(x_offset, y_offset);
		x_offset += 1;
		//let elapsed = start.elapsed();
		//let fps = 1000.0 / (elapsed.as_millis() as f64);
		//info!("{elapsed:?}\tFPS {fps:.0}");
		//start = Instant::now();
	}

	Ok(())
}
