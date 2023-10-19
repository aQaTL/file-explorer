use crate::{window::BitmapData, Texture};

pub fn draw_background(bitmap_data: BitmapData, x_offset: usize, y_offset: usize) {
	let bitmap_memory = bitmap_data.into_slice();

	for y in 0..(bitmap_data.bitmap_height as usize) {
		for x in 0..(bitmap_data.bitmap_width as usize) {
			let pixel = &mut bitmap_memory[y * bitmap_data.bitmap_width as usize + x];
			//*pixel = 0xFF8000;
			//*pixel = (((x & 0xFF) << 0) | ((y & 0xFF) << 8)) as u32;
			*pixel = ((x + x_offset) & 0xFF | (((y + y_offset) & 0xFF) << 8)) as u32;
		}
	}
}

pub fn draw_rectangle(
	bitmap_data: BitmapData,
	(pos_x, pos_y): (usize, usize),
	(width, height): (usize, usize),
	color: u32,
) {
	let bitmap_memory = bitmap_data.into_slice();

	for y in pos_y..(pos_y + height).min(bitmap_data.bitmap_height as usize) {
		for x in pos_x..(pos_x + width).min(bitmap_data.bitmap_width as usize) {
			let pixel = &mut bitmap_memory[y * bitmap_data.bitmap_width as usize + x];
			*pixel = color;
		}
	}
}

pub fn draw_texture(bitmap_data: BitmapData, texture: &Texture, pos_x: usize, pos_y: usize) {
	let bitmap_memory = bitmap_data.into_slice();

	for (tex_y, y) in
		(pos_y..(pos_y + texture.height).min(bitmap_data.bitmap_height as usize)).enumerate()
	{
		for (tex_x, x) in
			(pos_x..(pos_x + texture.width).min(bitmap_data.bitmap_width as usize)).enumerate()
		{
			let pixel = &mut bitmap_memory[y * bitmap_data.bitmap_width as usize + x];
			let bitmap_pixel = texture.bitmap[tex_y * texture.width + tex_x];

			let alpha = (((bitmap_pixel >> 24) & 0xFF) as f32) / 255.0;

			let background_r = ((*pixel >> 16) & 0xFF) as f32;
			let background_g = ((*pixel >> 8) & 0xFF) as f32;
			let background_b = (*pixel & 0xFF) as f32;
			let foreground_r = ((bitmap_pixel >> 16) & 0xFF) as f32;
			let foreground_g = ((bitmap_pixel >> 8) & 0xFF) as f32;
			let foreground_b = (bitmap_pixel & 0xFF) as f32;

			let r = lerp(background_r, foreground_r, alpha) as u32;
			let g = lerp(background_g, foreground_g, alpha) as u32;
			let b = lerp(background_b, foreground_b, alpha) as u32;

			*pixel = (r << 16) | (g << 8) | b;
		}
	}
}

fn lerp(v0: f32, v1: f32, t: f32) -> f32 {
	v0 + t * (v1 - v0)
}

pub fn dither(bitmap_data: BitmapData, x: usize, y: usize, width: usize, height: usize) {
	let bitmap_memory = bitmap_data.into_slice();

	for y in y..(y + height).min(bitmap_data.bitmap_height as usize) {
		for x in x..(x + width).min(bitmap_data.bitmap_width as usize) {
			let old_pixel = bitmap_memory[y * bitmap_data.bitmap_width as usize + x];
			let new_pixel = find_closest_palette_color(old_pixel);
			bitmap_memory[y * bitmap_data.bitmap_width as usize + x] = new_pixel;
			let quant_error = old_pixel.saturating_sub(new_pixel);

			if x < (bitmap_data.bitmap_width as usize - 1) {
				let pixel_idx = y * (bitmap_data.bitmap_width as usize) + (x + 1);
				if let Some(pixel) = bitmap_memory.get_mut(pixel_idx) {
					*pixel += quant_error * 7 / 16;
				}
			}

			if x > 0 && y < (bitmap_data.bitmap_height as usize - 1) {
				let pixel_idx = (y + 1) * (bitmap_data.bitmap_width as usize) + (x - 1);
				if let Some(pixel) = bitmap_memory.get_mut(pixel_idx) {
					*pixel += quant_error * 3 / 16;
				}
			}

			if y < (bitmap_data.bitmap_height as usize - 1) {
				let pixel_idx = (y + 1) * (bitmap_data.bitmap_width as usize) + x;
				if let Some(pixel) = bitmap_memory.get_mut(pixel_idx) {
					*pixel += quant_error * 5 / 16;
				}
			}

			if x < (bitmap_data.bitmap_width as usize - 1)
				&& y < (bitmap_data.bitmap_height as usize - 1)
			{
				let pixel_idx = (y + 1) * (bitmap_data.bitmap_width as usize) + (x + 1);
				if let Some(pixel) = bitmap_memory.get_mut(pixel_idx) {
					*pixel += quant_error * 1 / 16;
				}
			}
		}
	}
}

fn find_closest_palette_color(pixel: u32) -> u32 {
	const RED: u32 = 0xff0000;
	const GREEN: u32 = 0x00ff00;
	const BLUE: u32 = 0x0000ff;
	let palette: [u32; 8] = [
		0x000000, 0xff0000, 0x00ff00, 0xffff00, 0x0000ff, 0xff00ff, 0x00ffff, 0xffffff,
	];
	let mut nearest_color = 0;
	let mut minimum_distance: u64 = 255 * 255 + 255 * 255 + 255 * 255 + 1;
	for palette_color in palette {
		let red_diff = ((pixel & RED).saturating_sub(palette_color & RED)) as u64;
		let green_diff = ((pixel & GREEN).saturating_sub(palette_color & GREEN)) as u64;
		let blue_diff = ((pixel & BLUE).saturating_sub(palette_color & BLUE)) as u64;
		let distance: u64 = red_diff*red_diff + green_diff*green_diff + blue_diff*blue_diff;
		if distance < minimum_distance {
			minimum_distance = distance;
			nearest_color = palette_color;
		}
	}
	nearest_color
}
