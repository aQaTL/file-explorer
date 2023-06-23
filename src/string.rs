use std::iter;

pub trait WindowsStrings: AsRef<str> {
	fn to_utf16_with_null(&self) -> Vec<u16> {
		self.as_ref()
			.encode_utf16()
			.chain(iter::once(0))
			.collect::<Vec<_>>()
	}
}

impl<T> WindowsStrings for T where T: AsRef<str> {}
