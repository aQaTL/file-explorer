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

// TODO handle alpha channel
pub fn draw_texture(bitmap_data: BitmapData, texture: &Texture, pos_x: usize, pos_y: usize) {
	let bitmap_memory = bitmap_data.into_slice();

	for (tex_y, y) in
		(pos_y..(pos_y + texture.height).min(bitmap_data.bitmap_height as usize)).enumerate()
	{
		for (tex_x, x) in
			(pos_x..(pos_x + texture.width).min(bitmap_data.bitmap_width as usize)).enumerate()
		{
			let pixel = &mut bitmap_memory[y * bitmap_data.bitmap_width as usize + x];
			*pixel = texture.bitmap[tex_y * texture.width + tex_x];
		}
	}
}
