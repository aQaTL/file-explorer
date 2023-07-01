#![cfg_attr(feature = "windows_subsystem", windows_subsystem = "windows")]

use std::sync::atomic::{AtomicU32, Ordering};
use std::{io, ops::ControlFlow};

use log::error;

use crate::key::Key;
use crate::window::Window;

mod key;
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

	let mut state = State {
		x_offset: 0,
		y_offset: 0,
	};

	let mut start = std::time::Instant::now();

	static FPS: AtomicU32 = AtomicU32::new(0);

	#[cfg(feature = "fps")]
	{
		std::thread::spawn(|| loop {
			let fps = FPS.load(Ordering::Relaxed);
			log::info!("FPS: {fps}");
			std::thread::sleep(std::time::Duration::from_millis(100));
		});
	}

	while let ControlFlow::Continue(_) = window.process_messages() {
		update(&mut window);

		window.render(&state);
		state.x_offset += 1;

		{
			let elapsed = start.elapsed();
			let fps = (1000.0 / (elapsed.as_millis() as f64)) as u32;
			FPS.store(fps, Ordering::Relaxed);
			//log::debug!("{elapsed:?}\tFPS {fps:.0}");
			start = std::time::Instant::now();
		}
	}

	Ok(())
}

pub struct State {
	pub x_offset: usize,
	pub y_offset: usize,
}

fn update(window: &mut Window) {
	let keyboard = &window.window_data.keyboard;
	let bitmap_data = &mut window.window_data.bitmap_data;

	if keyboard.is_pressed(Key::Up) && bitmap_data.player_y > 0 {
		bitmap_data.player_y -= 5;
	}
	if keyboard.is_pressed(Key::Down)
		&& (bitmap_data.player_y as i32)
			< bitmap_data.bitmap_height - bitmap_data.player_height as i32
	{
		bitmap_data.player_y += 5;
	}
	if keyboard.is_pressed(Key::Left) && bitmap_data.player_x > 0 {
		bitmap_data.player_x -= 5;
	}
	if keyboard.is_pressed(Key::Right)
		&& (bitmap_data.player_x as i32)
			< bitmap_data.bitmap_width - bitmap_data.player_width as i32
	{
		bitmap_data.player_x += 5;
	}
}
