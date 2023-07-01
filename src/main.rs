#![cfg_attr(feature = "windows_subsystem", windows_subsystem = "windows")]

use std::sync::atomic::{AtomicU32, Ordering};
use std::{io, ops::ControlFlow};

use log::error;

use crate::draw::{draw_background, draw_rectangle};
use crate::key::Key;
use crate::window::Window;

mod draw;
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

	let mut state = Box::new(State {
		background: BackgroundState {
			x_offset: 0,
			y_offset: 0,
		},
		player: PlayerState {
			x: 0,
			y: 0,
			width: 50,
			height: 500,
		},
	});

	let state_ptr = state.as_ref() as *const State;
	window.on_key_press(Key::F3, move |_window, _keyboard| {
		// SAFETY: state lives for the duration for the program
		let state = unsafe { &*state_ptr };
		println!("{state:?}");
	});

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
		update(&mut window, &mut state);
		render(&mut window, &mut state);

		window.render();
		state.background.x_offset += 1;
		state.background.y_offset += 1;

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

#[derive(Debug)]
pub struct State {
	pub background: BackgroundState,
	pub player: PlayerState,
}

#[derive(Debug)]
pub struct BackgroundState {
	pub x_offset: usize,
	pub y_offset: usize,
}

#[derive(Debug)]
pub struct PlayerState {
	pub x: usize,
	pub y: usize,
	pub width: usize,
	pub height: usize,
}

fn update(window: &mut Window, state: &mut State) {
	let keyboard = &window.window_data.keyboard;
	let bitmap_data = &mut window.window_data.bitmap_data;

	if keyboard.is_pressed(Key::Up) && state.player.y > 0 {
		state.player.y -= 5;
	}
	if keyboard.is_pressed(Key::Down)
		&& (state.player.y as i32) < bitmap_data.bitmap_height - state.player.height as i32
	{
		state.player.y += 5;
	}
	if keyboard.is_pressed(Key::Left) && state.player.x > 0 {
		state.player.x -= 5;
	}
	if keyboard.is_pressed(Key::Right)
		&& (state.player.x as i32) < bitmap_data.bitmap_width - state.player.width as i32
	{
		state.player.x += 5;
	}
}

fn render(window: &mut Window, state: &mut State) {
	let bitmap_data = window.window_data.bitmap_data;

	draw_background(
		bitmap_data,
		state.background.x_offset,
		state.background.y_offset,
	);
	draw_rectangle(
		bitmap_data,
		(state.player.x, state.player.y),
		(state.player.width, state.player.height),
		0xd3869b,
	);
}
