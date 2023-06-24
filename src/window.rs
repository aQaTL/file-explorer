use core::slice;
use std::{
	io,
	mem::{self, MaybeUninit},
	ops::ControlFlow,
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
		UI::{
			Input::KeyboardAndMouse::{VIRTUAL_KEY, VK_DOWN, VK_LEFT, VK_RIGHT, VK_UP},
			WindowsAndMessaging::{
				CreateWindowExW, DefWindowProcW, DispatchMessageW, GetClientRect, PeekMessageW,
				PostQuitMessage, RegisterClassW, TranslateMessage, CS_HREDRAW, CS_OWNDC,
				CS_VREDRAW, CW_USEDEFAULT, HCURSOR, HICON, HMENU, MSG, PM_REMOVE, WINDOW_EX_STYLE,
				WM_ACTIVATEAPP, WM_CLOSE, WM_DESTROY, WM_KEYDOWN, WM_PAINT, WM_QUIT, WM_SIZE,
				WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
			},
		},
	},
};

use crate::string::WindowsStrings;

#[allow(dead_code)]
pub struct Window {
	window: HWND,
	classname: Vec<u16>,
	window_title: Vec<u16>,
}

impl Window {
	pub fn open() -> io::Result<Self> {
		unsafe {
			debug!("Create window");

			let h_instance = GetModuleHandleW(PCWSTR::null())?;

			let classname = "FileExplorerWindowClass".to_utf16_with_null();

			let wndclass = WNDCLASSW {
				style: CS_OWNDC | CS_HREDRAW | CS_VREDRAW,
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

			BITMAP_DATA.player_width = 50;
			BITMAP_DATA.player_height = 100;

			let window_title = "File Explorer".to_utf16_with_null();

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
				None,
			);
			debug!("Window created");

			if hwnd.0 == 0 {
				println!("lastoserror: {}", GetLastError().0);
				return Err(io::Error::last_os_error());
			}

			Ok(Window {
				window: hwnd,
				classname,
				window_title,
			})
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

	pub fn render(&self, x_offset: usize, y_offset: usize) {
		unsafe {
			render(x_offset, y_offset);

			let device_context = match DeviceContext::get(self.window) {
				Ok(v) => v,
				Err(err) => {
					error!("Invalid DeviceContext: {err}");
					return;
				}
			};

			let client_rect = {
				let mut rect = MaybeUninit::<RECT>::uninit();
				let result = GetClientRect(self.window, rect.as_mut_ptr());
				if result.0 == 0 {
					error!("{}", io::Error::last_os_error());
					return;
				}
				rect.assume_init()
			};

			let window_width = client_rect.right - client_rect.left;
			let window_height = client_rect.bottom - client_rect.top;

			update_window(
				device_context.0,
				&client_rect,
				0,
				0,
				window_width,
				window_height,
			);
		}
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
	window: HWND,
	message: u32,
	w_param: WPARAM,
	l_param: LPARAM,
) -> LRESULT {
	//TODO Figure out how to return Rust errors instead of just logging them

	let mut callback_result = 0;

	match message {
		WM_SIZE => {
			debug!("WM_SIZE");

			let client_rect = unsafe {
				let mut rect = MaybeUninit::<RECT>::uninit();
				let result = GetClientRect(window, rect.as_mut_ptr());
				if result.0 == 0 {
					error!("{}", io::Error::last_os_error());
					return LRESULT(callback_result);
				}
				rect.assume_init()
			};
			let width = client_rect.right - client_rect.left;
			let height = client_rect.bottom - client_rect.top;

			if let Err(err) = resize_dib_section(width, height) {
				error!("resize_dib_section: {err}");
			}
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
			let device_context = BeginPaint(window, paint.as_mut_ptr());
			if device_context.is_invalid() {
				error!("Invalid DeviceContext: {}", io::Error::last_os_error());
				return LRESULT(callback_result);
			}
			let paint = paint.assume_init();

			let x = paint.rcPaint.left;
			let y = paint.rcPaint.top;
			let width = paint.rcPaint.right - paint.rcPaint.left;
			let height = paint.rcPaint.bottom - paint.rcPaint.top;

			let client_rect = {
				let mut rect = MaybeUninit::<RECT>::uninit();
				let result = GetClientRect(window, rect.as_mut_ptr());
				if result.0 == 0 {
					error!("{}", io::Error::last_os_error());
					return LRESULT(callback_result);
				}
				rect.assume_init()
			};

			update_window(device_context, &client_rect, x, y, width, height);

			EndPaint(window, &paint);
		},
		WM_KEYDOWN => match VIRTUAL_KEY(w_param.0 as u16) {
			VK_UP => {
				if BITMAP_DATA.player_y > 0 {
					BITMAP_DATA.player_y -= 10;
				}
			}
			VK_DOWN => {
				if (BITMAP_DATA.player_y as i32)
					< BITMAP_DATA.bitmap_height - BITMAP_DATA.player_height as i32
				{
					BITMAP_DATA.player_y += 10;
				}
			}
			VK_LEFT => {
				if BITMAP_DATA.player_x > 0 {
					BITMAP_DATA.player_x -= 10;
				}
			}
			VK_RIGHT => {
				if (BITMAP_DATA.player_x as i32)
					< BITMAP_DATA.bitmap_width - BITMAP_DATA.player_width as i32
				{
					BITMAP_DATA.player_x += 10;
				}
			}
			_ => (),
		},
		_ => {
			callback_result = DefWindowProcW(window, message, w_param, l_param).0;
		}
	}

	LRESULT(callback_result)
}

static mut BITMAP_DATA: BitmapData =
	unsafe { mem::transmute([0_u8; mem::size_of::<BitmapData>()]) };

struct BitmapData {
	bitmap_memory: *mut std::os::raw::c_void,
	bitmap_memory_size: usize,
	bitmap_info: BITMAPINFO,
	bitmap_width: i32,
	bitmap_height: i32,
	player_x: usize,
	player_y: usize,
	player_width: usize,
	player_height: usize,
}

unsafe impl Sync for BitmapData {}

unsafe fn resize_dib_section(width: i32, height: i32) -> io::Result<()> {
	// Free DIBSection

	if !BITMAP_DATA.bitmap_memory.is_null() {
		VirtualFree(BITMAP_DATA.bitmap_memory, 0, MEM_RELEASE);
	}

	BITMAP_DATA.bitmap_width = width;
	BITMAP_DATA.bitmap_height = height;

	BITMAP_DATA.bitmap_info = BITMAPINFO {
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
	// Memory allocated by VirtualAloc is initialized to 0
	let bitmap_memory = VirtualAlloc(None, bitmap_memory_size, MEM_COMMIT, PAGE_READWRITE);
	if bitmap_memory.is_null() {
		return Err(io::Error::last_os_error());
	}
	BITMAP_DATA.bitmap_memory = bitmap_memory;
	BITMAP_DATA.bitmap_memory_size = bitmap_memory_size;

	render(128, 0);

	Ok(())
}

#[allow(unused_variables)]
unsafe fn update_window(
	device_context: HDC,
	window_rect: &RECT,
	x: i32,
	y: i32,
	width: i32,
	height: i32,
) {
	let window_width = window_rect.right - window_rect.left;
	let window_height = window_rect.bottom - window_rect.top;

	let result = StretchDIBits(
		device_context,
		0,
		0,
		BITMAP_DATA.bitmap_width,
		BITMAP_DATA.bitmap_height,
		0,
		0,
		window_width,
		window_height,
		Some(BITMAP_DATA.bitmap_memory),
		&BITMAP_DATA.bitmap_info,
		DIB_RGB_COLORS,
		SRCCOPY,
	);
	if matches!(result, 0 | GDI_ERROR) {
		error!("StretchDIBits failed: {}", io::Error::last_os_error());
		return;
	}

	return;
}

unsafe fn render(x_offset: usize, y_offset: usize) {
	let bitmap_memory: &'static mut [u32] = slice::from_raw_parts_mut(
		BITMAP_DATA.bitmap_memory.cast::<u32>(),
		BITMAP_DATA.bitmap_memory_size,
	);

	for y in 0..(BITMAP_DATA.bitmap_height as usize) {
		for x in 0..(BITMAP_DATA.bitmap_width as usize) {
			let pixel = &mut bitmap_memory[y * BITMAP_DATA.bitmap_width as usize + x];
			//*pixel = 0xFF8000;
			//*pixel = (((x & 0xFF) << 0) | ((y & 0xFF) << 8)) as u32;
			*pixel = ((x + x_offset) & 0xFF | (((y + y_offset) & 0xFF) << 8)) as u32;
		}
	}

	for y in BITMAP_DATA.player_y..(BITMAP_DATA.player_y + BITMAP_DATA.player_height) {
		for x in BITMAP_DATA.player_x..(BITMAP_DATA.player_x + BITMAP_DATA.player_width) {
			let pixel = &mut bitmap_memory[y * BITMAP_DATA.bitmap_width as usize + x];
			*pixel = 0xd3869b;
		}
	}
}
