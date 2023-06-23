use std::{io, mem::MaybeUninit, ops::ControlFlow};

use log::{debug, error};
use windows::{
	core::PCWSTR,
	Win32::{
		Foundation::{GetLastError, HWND, LPARAM, LRESULT, WPARAM},
		Graphics::Gdi::{BeginPaint, EndPaint, PatBlt, BLACKNESS, HBRUSH, PAINTSTRUCT, WHITENESS},
		System::LibraryLoader::GetModuleHandleW,
		UI::WindowsAndMessaging::{
			CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostQuitMessage,
			RegisterClassW, TranslateMessage, CS_HREDRAW, CS_OWNDC, CS_VREDRAW, CW_USEDEFAULT,
			HCURSOR, HICON, HMENU, MSG, WINDOW_EX_STYLE, WM_ACTIVATEAPP, WM_CLOSE, WM_DESTROY,
			WM_PAINT, WM_SIZE, WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
		},
	},
};

use crate::string::WindowsStrings;

#[allow(dead_code)]
pub struct Window {
	hwnd: HWND,
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
				hwnd,
				classname,
				window_title,
			})
		}
	}

	pub fn get_message(&mut self) -> ControlFlow<io::Result<()>> {
		unsafe {
			let mut msg = MaybeUninit::<MSG>::uninit();
			match GetMessageW(msg.as_mut_ptr(), HWND::default(), 0, 0).0 {
				-1 => return ControlFlow::Break(Err(io::Error::last_os_error())),
				0 => return ControlFlow::Break(Ok(())),
				_ => (),
			}
			let msg = msg.assume_init();

			TranslateMessage(&msg);
			DispatchMessageW(&msg);

			ControlFlow::Continue(())
		}
	}
}

unsafe extern "system" fn main_window_callback(
	window: HWND,
	message: u32,
	w_param: WPARAM,
	l_param: LPARAM,
) -> LRESULT {
	let mut callback_result = 0;

	match message {
		WM_SIZE => {
			debug!("WM_SIZE");
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

			let result = PatBlt(device_context, x, y, width, height, BLACKNESS);
			if result.0 == 0 {
				error!("PatBlt failed: {}", io::Error::last_os_error());
				return LRESULT(callback_result);
			}
			let result = PatBlt(
				device_context,
				x + 100,
				y + 100,
				x + 200,
				y + 200,
				WHITENESS,
			);
			if result.0 == 0 {
				error!("PatBlt failed: {}", io::Error::last_os_error());
				return LRESULT(callback_result);
			}

			EndPaint(window, &paint);
		},
		_ => {
			callback_result = DefWindowProcW(window, message, w_param, l_param).0;
		}
	}

	LRESULT(callback_result)
}
