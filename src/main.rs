use std::{io, ops::ControlFlow};

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

	loop {
		match window.get_message() {
			ControlFlow::Continue(_) => (),
			ControlFlow::Break(result) => return result,
		}
	}
}
