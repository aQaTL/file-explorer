use std::{
	collections::HashMap,
	ffi::c_void,
	io,
	mem::{self, MaybeUninit},
	ops::ControlFlow,
	slice, usize,
};

use log::{debug, error};
use windows::{
	core::PCWSTR,
	Win32::{
		Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM},
		Graphics::Gdi::{
			BeginPaint, EndPaint, GetDC, ReleaseDC, StretchDIBits, BITMAPINFO, BITMAPINFOHEADER,
			BI_RGB, DIB_RGB_COLORS, GDI_ERROR, HBRUSH, HDC, PAINTSTRUCT, RGBQUAD, SRCCOPY,
		},
		System::{
			LibraryLoader::GetModuleHandleW,
			Memory::{VirtualAlloc, VirtualFree, MEM_COMMIT, MEM_RELEASE, PAGE_READWRITE},
		},
		UI::WindowsAndMessaging::{
			CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, GetWindowLongPtrW,
			PeekMessageW, PostQuitMessage, RegisterClassW, SetWindowLongPtrW, TranslateMessage,
			CREATESTRUCTW, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, GWLP_USERDATA, HCURSOR, HICON,
			HMENU, MSG, PM_REMOVE, WINDOW_EX_STYLE, WM_ACTIVATEAPP, WM_CLOSE, WM_CREATE,
			WM_DESTROY, WM_KEYDOWN, WM_KEYUP, WM_PAINT, WM_QUIT, WM_SIZE, WNDCLASSW,
			WS_OVERLAPPEDWINDOW, WS_VISIBLE,
		},
	},
};

use crate::key::Key;
use crate::string::WindowsStrings;

pub struct Window {
	window: HWND,

	#[allow(dead_code)]
	classname: Vec<u16>,
	#[allow(dead_code)]
	window_title: Vec<u16>,

	pub window_data: Box<WindowData>,
}

#[derive(Default)]
pub struct WindowData {
	pub bitmap_data: BitmapData,
	pub keyboard: Keyboard,

	#[allow(clippy::type_complexity)]
	key_handlers: HashMap<Key, Box<dyn Fn(&mut BitmapData, &mut Keyboard)>>,
}

#[derive(Copy, Clone)]
pub struct BitmapData {
	bitmap_memory: *mut std::os::raw::c_void,
	bitmap_memory_size: usize,
	bitmap_info: BITMAPINFO,
	pub bitmap_width: i32,
	pub bitmap_height: i32,
	pub player_x: usize,
	pub player_y: usize,
	pub player_width: usize,
	pub player_height: usize,
}

impl Default for BitmapData {
	fn default() -> Self {
		unsafe { mem::transmute([0_u8; mem::size_of::<BitmapData>()]) }
	}
}

unsafe impl Sync for BitmapData {}

pub struct Keyboard {
	keyboard: [bool; 65536],
}

impl Default for Keyboard {
	fn default() -> Self {
		Keyboard {
			keyboard: [false; 65536],
		}
	}
}

impl Keyboard {
	#[inline]
	pub fn is_pressed(&self, key: Key) -> bool {
		self.keyboard[key as usize]
	}
}

impl Window {
	pub fn open() -> io::Result<Self> {
		unsafe {
			debug!("Create window");

			let mut window_data = Box::new(WindowData {
				bitmap_data: BitmapData {
					player_width: 50,
					player_height: 500,
					..Default::default()
				},
				..Default::default()
			});
			if let Err(err) = resize_dib_section(&mut window_data.bitmap_data, 1280, 720) {
				error!("resize_dib_section: {err}");
			}

			let h_instance = GetModuleHandleW(PCWSTR::null())?;

			let classname = "FileExplorerWindowClass".to_utf16_with_null();
			let wndclass = WNDCLASSW {
				style: CS_HREDRAW | CS_VREDRAW,
				lpfnWndProc: Some(main_window_callback),
				cbClsExtra: 0,
				cbWndExtra: 0,
				hInstance: h_instance,
				hIcon: HICON::default(),
				hCursor: HCURSOR::default(),
				hbrBackground: HBRUSH::default(),
				lpszMenuName: PCWSTR::null(),
				lpszClassName: PCWSTR(classname.as_ptr()),
			};

			let result = RegisterClassW(&wndclass);
			if result == 0 {
				return Err(io::Error::last_os_error());
			}
			debug!("Class registered");

			let window_title = "File Explorer".to_utf16_with_null();

			let window_data_ptr = (window_data.as_ref() as *const WindowData).cast::<c_void>();

			let hwnd = CreateWindowExW(
				WINDOW_EX_STYLE::default(),
				PCWSTR(classname.as_ptr()),
				PCWSTR(window_title.as_ptr()),
				WS_OVERLAPPEDWINDOW | WS_VISIBLE,
				CW_USEDEFAULT,
				CW_USEDEFAULT,
				CW_USEDEFAULT,
				CW_USEDEFAULT,
				HWND::default(),
				HMENU::default(),
				h_instance,
				Some(window_data_ptr),
			);
			debug!("Window created");

			if hwnd.0 == 0 {
				println!("lastoserror: {}", GetLastError().0);
				return Err(io::Error::last_os_error());
			}

			let window = Window {
				window: hwnd,
				classname,
				window_title,

				window_data,
			};

			Ok(window)
		}
	}

	pub fn process_messages(&mut self) -> ControlFlow<()> {
		unsafe {
			let mut msg = MaybeUninit::<MSG>::uninit();
			while PeekMessageW(msg.as_mut_ptr(), HWND::default(), 0, 0, PM_REMOVE).0 != 0 {
				if msg.assume_init_ref().message == WM_QUIT {
					return ControlFlow::Break(());
				}
				TranslateMessage(msg.as_ptr());
				DispatchMessageW(msg.as_ptr());
			}
			ControlFlow::Continue(())
		}
	}

	pub fn render(&self, state: &crate::State) {
		unsafe {
			render(self.window_data.bitmap_data, state.x_offset, state.y_offset);

			let device_context = match DeviceContext::get(self.window) {
				Ok(v) => v,
				Err(err) => {
					error!("Invalid DeviceContext: {err}");
					return;
				}
			};

			let (window_width, window_height) = match window_dimensions(self.window) {
				Ok(v) => v,
				Err(err) => {
					error!("{err}");
					return;
				}
			};

			display_bitmap(
				device_context.0,
				self.window_data.bitmap_data,
				window_width,
				window_height,
			);
		}
	}

	#[allow(dead_code)]
	pub fn on_key_press<F>(&mut self, key: Key, f: F)
	where
		F: Fn(&mut BitmapData, &mut Keyboard) + 'static,
	{
		self.window_data.key_handlers.insert(key, Box::new(f));
	}
}

/// Returns size of a given window in a form (width, height).
fn window_dimensions(window: HWND) -> io::Result<(i32, i32)> {
	unsafe {
		let mut rect = MaybeUninit::<RECT>::uninit();
		let result = GetClientRect(window, rect.as_mut_ptr());
		if result.0 == 0 {
			return Err(io::Error::last_os_error());
		}
		let rect = rect.assume_init();
		let width = rect.right - rect.left;
		let height = rect.bottom - rect.top;
		Ok((width, height))
	}
}

struct DeviceContext(pub HDC, pub HWND);

impl DeviceContext {
	fn get(window: HWND) -> io::Result<Self> {
		let device_context = unsafe { GetDC(window) };
		if device_context.is_invalid() {
			return Err(io::Error::last_os_error());
		}
		Ok(DeviceContext(device_context, window))
	}
}

impl Drop for DeviceContext {
	fn drop(&mut self) {
		unsafe {
			ReleaseDC(self.1, self.0);
		}
	}
}

unsafe extern "system" fn main_window_callback(
	window_handle: HWND,
	message: u32,
	w_param: WPARAM,
	l_param: LPARAM,
) -> LRESULT {
	//TODO Figure out how to return Rust errors instead of just logging them

	let window_data = &mut *(GetWindowLongPtrW(window_handle, GWLP_USERDATA) as *mut WindowData);
	let bitmap_data = &mut window_data.bitmap_data;
	let key_handlers = &mut window_data.key_handlers;

	let mut callback_result = 0;

	match message {
		WM_CREATE => {
			debug!("WM_CREATE");
			let create_struct = &*mem::transmute::<_, *const CREATESTRUCTW>(l_param);
			let window_data_ptr = create_struct.lpCreateParams as isize;
			SetWindowLongPtrW(window_handle, GWLP_USERDATA, window_data_ptr);
		}
		WM_SIZE => {
			/*
			debug!("WM_SIZE");

			let (width, height) = match window_dimensions(window_handle) {
				Ok(v) => v,
				Err(err) => {
					error!("{err}");
					return LRESULT(callback_result);
				}
			};

			info!("New size: {width}x{height}");

			if let Err(err) = resize_dib_section(bitmap_data, width, height) {
				error!("resize_dib_section: {err}");
			}
			*/
		}
		WM_DESTROY => {
			debug!("WM_DESTROY");
		}
		WM_CLOSE => {
			debug!("Close requested");
			PostQuitMessage(0);
		}
		WM_ACTIVATEAPP => {
			debug!("WM_ACTIVATEAPP");
		}
		WM_PAINT => unsafe {
			let mut paint = MaybeUninit::<PAINTSTRUCT>::uninit();
			let device_context = BeginPaint(window_handle, paint.as_mut_ptr());
			if device_context.is_invalid() {
				error!("Invalid DeviceContext: {}", io::Error::last_os_error());
				return LRESULT(callback_result);
			}
			let paint = paint.assume_init();

			let (window_width, window_height) = match window_dimensions(window_handle) {
				Ok(v) => v,
				Err(err) => {
					error!("{err}");
					return LRESULT(callback_result);
				}
			};

			display_bitmap(device_context, *bitmap_data, window_width, window_height);

			EndPaint(window_handle, &paint);
		},
		WM_KEYDOWN => {
			let was_down = window_data.keyboard.keyboard[w_param.0];
			window_data.keyboard.keyboard[w_param.0] = true;
			if !was_down {
				let key: Key = unsafe { std::mem::transmute(w_param.0 as u16) };
				if let Some(handler) = key_handlers.get(&key) {
					handler(bitmap_data, &mut window_data.keyboard);
				}
			}
		}
		WM_KEYUP => {
			window_data.keyboard.keyboard[w_param.0] = false;
		}
		_ => {
			callback_result = DefWindowProcW(window_handle, message, w_param, l_param).0;
		}
	}

	LRESULT(callback_result)
}

unsafe fn resize_dib_section(
	bitmap_data: &mut BitmapData,
	width: i32,
	height: i32,
) -> io::Result<()> {
	if width == 0 || height == 0 {
		return Ok(());
	}

	if !bitmap_data.bitmap_memory.is_null() {
		VirtualFree(bitmap_data.bitmap_memory, 0, MEM_RELEASE);
	}

	bitmap_data.bitmap_width = width;
	bitmap_data.bitmap_height = height;

	bitmap_data.bitmap_info = BITMAPINFO {
		bmiHeader: BITMAPINFOHEADER {
			biSize: mem::size_of::<BITMAPINFOHEADER>() as u32,
			biWidth: width,
			biHeight: -height,
			biPlanes: 1,
			biBitCount: 32,
			biCompression: BI_RGB.0 as u32,
			biSizeImage: 0,
			biXPelsPerMeter: 0,
			biYPelsPerMeter: 0,
			biClrUsed: 0,
			biClrImportant: 0,
		},
		bmiColors: [RGBQUAD {
			rgbBlue: 0,
			rgbGreen: 0,
			rgbRed: 0,
			rgbReserved: 0,
		}],
	};

	let bytes_per_pixel = 4;
	let bitmap_memory_size = (width as usize * height as usize) * bytes_per_pixel;

	// Memory allocated by VirtualAlloc is initialized to 0
	let bitmap_memory = VirtualAlloc(None, bitmap_memory_size, MEM_COMMIT, PAGE_READWRITE);
	if bitmap_memory.is_null() {
		return Err(io::Error::last_os_error());
	}
	bitmap_data.bitmap_memory = bitmap_memory;
	bitmap_data.bitmap_memory_size = bitmap_memory_size;

	render(*bitmap_data, 128, 0);

	Ok(())
}

unsafe fn display_bitmap(
	device_context: HDC,
	bitmap_data: BitmapData,
	window_width: i32,
	window_height: i32,
) {
	if window_width == 0 || window_height == 0 {
		return;
	}

	let result = StretchDIBits(
		device_context,
		0,
		0,
		window_width,
		window_height,
		0,
		0,
		bitmap_data.bitmap_width,
		bitmap_data.bitmap_height,
		Some(bitmap_data.bitmap_memory),
		&bitmap_data.bitmap_info,
		DIB_RGB_COLORS,
		SRCCOPY,
	);
	if matches!(result, 0 | GDI_ERROR) {
		error!("StretchDIBits failed: {}", io::Error::last_os_error());
	}
}

unsafe fn render(bitmap_data: BitmapData, x_offset: usize, y_offset: usize) {
	let bitmap_memory: &'static mut [u32] = slice::from_raw_parts_mut(
		bitmap_data.bitmap_memory.cast::<u32>(),
		bitmap_data.bitmap_memory_size / std::mem::size_of::<u32>(),
	);

	for y in 0..(bitmap_data.bitmap_height as usize) {
		for x in 0..(bitmap_data.bitmap_width as usize) {
			let pixel = &mut bitmap_memory[y * bitmap_data.bitmap_width as usize + x];
			//*pixel = 0xFF8000;
			//*pixel = (((x & 0xFF) << 0) | ((y & 0xFF) << 8)) as u32;
			*pixel = ((x + x_offset) & 0xFF | (((y + y_offset) & 0xFF) << 8)) as u32;
		}
	}

	for y in bitmap_data.player_y
		..(bitmap_data.player_y + bitmap_data.player_height).min(bitmap_data.bitmap_height as usize)
	{
		for x in bitmap_data.player_x
			..(bitmap_data.player_x + bitmap_data.player_width)
				.min(bitmap_data.bitmap_width as usize)
		{
			let pixel = &mut bitmap_memory[y * bitmap_data.bitmap_width as usize + x];
			*pixel = 0xd3869b;
		}
	}
}
