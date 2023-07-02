#![allow(clippy::upper_case_acronyms)]

use std::{
	fmt::Display,
	io::{Read, Write},
	path::PathBuf,
};

use flate2::read::ZlibDecoder;

pub struct Png {
	header: IHDR,
	img_data: Vec<u8>,
}

#[derive(Debug)]
pub enum Error {
	Io {
		err: std::io::Error,
		filename: PathBuf,
	},
	BadMagic,
	FileEnd,
	ChecksumFailed,
	ExpectedIHDR,
	IncompleteBlock {
		block_kind: &'static str,
	},
	NoIDAT,
	Deflate(std::io::Error),
	OnlyRGBA,
	InterlaceNotSupported,
	InvalidFilterType,
}

impl Display for Error {
	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
		match self {
			Error::Io { err, filename } => {
				write!(f, "Failed to load {}: {}.", filename.display(), err)
			}
			Error::BadMagic => write!(f, "Unknown file format."),
			Error::FileEnd => write!(f, "File ended abruptly. Not enough data."),
			Error::ChecksumFailed => write!(f, "Checksum doesn't match."),
			Error::ExpectedIHDR => write!(f, "Expected IHDR block."),
			Error::IncompleteBlock { block_kind } => {
				write!(f, "Missing fields in block {block_kind}")
			}
			Error::NoIDAT => write!(f, "Missing actual image data."),
			Error::Deflate(err) => write!(f, "Failed to decompress: {err}."),
			Error::OnlyRGBA => write!(f, "This parser only supports RGBA images."),
			Error::InterlaceNotSupported => {
				write!(f, "This parser only supportes not interlaced images.")
			}
			Error::InvalidFilterType => write!(f, "Unknown filter type."),
		}
	}
}

impl std::error::Error for Error {}

impl Png {
	pub fn load_from_path(p: &str) -> Result<Self, Error> {
		let data = std::fs::read(p).map_err(|err| Error::Io {
			err,
			filename: p.into(),
		})?;

		let mut state = parser::State { current_byte: 0 };
		let data = parser::Data { data: &data };

		parser::parse_magic(&mut state, &data)?;

		let ihdr = match parser::parse_block(&mut state, &data)? {
			PngBlock {
				block: PngBlockKind::IHDR(ihdr),
				..
			} => ihdr,
			_ => return Err(Error::ExpectedIHDR),
		};
		println!("IHDR: {ihdr:#?}");

		// We only support one type of PNG :/
		if ihdr.color_type != 6 || ihdr.bit_depth != 8 {
			return Err(Error::OnlyRGBA);
		}
		if ihdr.interlace_method != 0 {
			return Err(Error::InterlaceNotSupported);
		}

		let mut blocks = Vec::new();
		loop {
			let block = parser::parse_block(&mut state, &data)?;
			if let PngBlockKind::IEND = block.block {
				break;
			}
			blocks.push(block);
		}

		let decompressed_img = decompress_img_data(&blocks)?;
		let decompressed_img_data = parser::Data {
			data: &decompressed_img,
		};
		let mut decompresseed_img_state = parser::State { current_byte: 0 };

		let mut raw_img = Vec::with_capacity(decompressed_img.len());
		let mut row_idx = 0;
		while decompresseed_img_state.current_byte != decompressed_img.len() {
			let filter_type: FilterType =
				parser::get_u8(&mut decompresseed_img_state, &decompressed_img_data)?.try_into()?;
			let encoded_line = parser::get_slice(
				&mut decompresseed_img_state,
				&decompressed_img_data,
				ihdr.width as usize * 4,
			)?;
			decode_filter(&mut raw_img, encoded_line, filter_type, row_idx);
			row_idx += 1;
		}

		raw_img
			.as_mut_slice()
			.chunks_exact_mut(4)
			.map(|chunk| &mut chunk[0..3])
			.for_each(|chunk| chunk.reverse());

		Ok(Png {
			header: ihdr,
			img_data: raw_img,
		})
	}
}

impl From<Png> for crate::Texture {
	fn from(img: Png) -> Self {
		let mut img_data = std::mem::ManuallyDrop::new(img.img_data);
		let ptr = img_data.as_mut_ptr().cast::<u32>();
		let len = img_data.len() / 4;
		let cap = img_data.capacity() / 4;
		let bitmap = unsafe { Vec::<u32>::from_raw_parts(ptr, len, cap) };
		crate::Texture {
			bitmap,
			width: img.header.width as usize,
			height: img.header.height as usize,
		}
	}
}

#[allow(dead_code)]
#[derive(Debug)]
struct PngBlock<'a> {
	len: usize,
	chunk_type: [u8; 4],
	checksum: u32,

	block: PngBlockKind<'a>,
}

#[derive(Debug)]
enum PngBlockKind<'a> {
	IHDR(IHDR),
	IEND,
	IDAT(IDAT<'a>),

	Unknown,
}

/// Image Header
#[allow(dead_code)]
#[derive(Debug)]
struct IHDR {
	width: u32,
	height: u32,
	bit_depth: u8,
	color_type: u8,
	compression_method: u8,
	filter_method: u8,
	interlace_method: u8,
}

/// Image Data
#[derive(Debug)]
struct IDAT<'a> {
	data: &'a [u8],
}

mod parser {
	use super::{Error, PngBlock, PngBlockKind, IDAT, IHDR};

	pub(super) struct State {
		/// Index of current byte in Self.data
		pub current_byte: usize,
	}

	pub(super) struct Data<'a> {
		pub data: &'a [u8],
	}

	pub(super) fn parse_magic(state: &mut State, data: &Data) -> Result<(), Error> {
		if state.current_byte + 8 > data.data.len() {
			return Err(Error::BadMagic);
		}

		let expected_png_signature = &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
		let png_signature = &data.data[state.current_byte..(state.current_byte + 8)];
		let signature_valid = png_signature == expected_png_signature;

		if signature_valid {
			state.current_byte += 8;
			Ok(())
		} else {
			Err(Error::BadMagic)
		}
	}

	pub(super) fn parse_block<'data>(
		state: &mut State,
		data: &'data Data,
	) -> Result<PngBlock<'data>, Error> {
		let len = get_u32(state, data)? as usize;
		let chunk_type = get_slice(state, data, 4)?;
		let (chunk_data, block) = match chunk_type {
			[b'I', b'H', b'D', b'R'] => parse_ihdr(state, data, len)?,
			[b'I', b'E', b'N', b'D'] => parse_iend(len)?,
			[b'I', b'D', b'A', b'T'] => parse_idat(state, data, len)?,
			_ => {
				let data = get_slice(state, data, len)?;
				let block = PngBlockKind::Unknown;
				(data, block)
			}
		};

		let checksum = get_u32(state, data)?;
		let calculated_checksum = {
			let mut hasher = crc32fast::Hasher::new();
			hasher.update(chunk_type);
			hasher.update(chunk_data);
			hasher.finalize()
		};
		if calculated_checksum != checksum {
			return Err(Error::ChecksumFailed);
		}

		Ok(PngBlock {
			len,
			chunk_type: chunk_type.try_into().unwrap(),
			checksum,
			block,
		})
	}

	fn parse_ihdr<'data>(
		state: &mut State,
		data: &'data Data,
		expected_len: usize,
	) -> Result<(&'data [u8], PngBlockKind<'data>), Error> {
		let start = state.current_byte;

		let width = get_u32(state, data)?;
		let height = get_u32(state, data)?;
		let bit_depth = get_u8(state, data)?;
		let color_type = get_u8(state, data)?;
		let compression_method = get_u8(state, data)?;
		let filter_method = get_u8(state, data)?;
		let interlace_method = get_u8(state, data)?;

		let parsed_bytes = state.current_byte - start;
		if expected_len != parsed_bytes {
			return Err(Error::IncompleteBlock { block_kind: "IHDR" });
		}

		let data = &data.data[start..state.current_byte];
		Ok((
			data,
			PngBlockKind::IHDR(IHDR {
				width,
				height,
				bit_depth,
				color_type,
				compression_method,
				filter_method,
				interlace_method,
			}),
		))
	}

	fn parse_iend<'data>(expected_len: usize) -> Result<(&'data [u8], PngBlockKind<'data>), Error> {
		if expected_len != 0 {
			return Err(Error::IncompleteBlock { block_kind: "IEND" });
		}

		let data = &[];
		Ok((data, PngBlockKind::IEND))
	}

	fn parse_idat<'data>(
		state: &mut State,
		data: &'data Data,
		expected_len: usize,
	) -> Result<(&'data [u8], PngBlockKind<'data>), Error> {
		let data = get_slice(state, data, expected_len)?;

		Ok((data, PngBlockKind::IDAT(IDAT { data })))
	}

	fn get_u32(state: &mut State, data: &Data) -> Result<u32, Error> {
		if state.current_byte + 4 > data.data.len() {
			return Err(Error::FileEnd);
		}
		// PNG uses Big Endian
		let n = u32::from_be_bytes(
			data.data[state.current_byte..(state.current_byte + 4)]
				.try_into()
				.unwrap(),
		);
		state.current_byte += 4;
		Ok(n)
	}

	pub(super) fn get_u8(state: &mut State, data: &Data) -> Result<u8, Error> {
		if state.current_byte + 1 > data.data.len() {
			return Err(Error::FileEnd);
		}
		let n = data.data[state.current_byte];
		state.current_byte += 1;
		Ok(n)
	}

	pub(super) fn get_slice<'data>(
		state: &mut State,
		data: &'data Data,
		len: usize,
	) -> Result<&'data [u8], Error> {
		if state.current_byte + len > data.data.len() {
			return Err(Error::FileEnd);
		}
		state.current_byte += len;
		Ok(&data.data[(state.current_byte - len)..state.current_byte])
	}
}

struct IDATBlockStream<'a> {
	blocks: &'a [PngBlock<'a>],
	current_data_block: &'a [u8],
	current_data_block_idx: usize,
	current_buf_idx: usize,
}

impl<'a> IDATBlockStream<'a> {
	fn new(blocks: &'a [PngBlock<'a>]) -> Result<Self, Error> {
		let (data_block_idx, data_block) = blocks
			.iter()
			.enumerate()
			.find_map(|(idx, block)| match block {
				PngBlock {
					block: PngBlockKind::IDAT(idat),
					..
				} => Some((idx, idat.data)),
				_ => None,
			})
			.ok_or(Error::NoIDAT)?;

		Ok(IDATBlockStream {
			blocks,
			current_data_block: data_block,
			current_data_block_idx: data_block_idx,
			current_buf_idx: 0,
		})
	}
}

impl<'a> std::io::Read for IDATBlockStream<'a> {
	fn read(&mut self, mut buf: &mut [u8]) -> std::io::Result<usize> {
		if self.current_buf_idx == self.current_data_block.len() {
			let next_data_block = self
				.blocks
				.iter()
				.enumerate()
				.skip(self.current_data_block_idx + 1)
				.find_map(|(idx, block)| match block {
					PngBlock {
						block: PngBlockKind::IDAT(idat),
						..
					} => Some((idx, idat.data)),
					_ => None,
				});

			let Some((next_data_block_idx, next_data_block)) = next_data_block else {
				return Ok(0);
			};
			self.current_data_block = next_data_block;
			self.current_data_block_idx = next_data_block_idx;
			self.current_buf_idx = 0;
		}

		let written = buf.write(&self.current_data_block[self.current_buf_idx..])?;
		self.current_buf_idx += written;
		Ok(written)
	}
}

fn decompress_img_data(blocks: &[PngBlock<'_>]) -> Result<Vec<u8>, Error> {
	let block_stream = IDATBlockStream::new(blocks)?;

	let mut deflater = ZlibDecoder::new(block_stream);
	let mut data = Vec::new();
	deflater.read_to_end(&mut data).map_err(Error::Deflate)?;

	Ok(data)
}

#[repr(u8)]
#[allow(dead_code)]
enum FilterType {
	None = 0,
	Sub = 1,
	Up = 2,
	Average = 3,
	Paeth = 4,
}

impl TryFrom<u8> for FilterType {
	type Error = Error;

	fn try_from(v: u8) -> Result<Self, Self::Error> {
		if v > FilterType::Paeth as u8 {
			return Err(Error::InvalidFilterType);
		}
		unsafe { Ok(std::mem::transmute::<_, FilterType>(v)) }
	}
}

fn decode_filter(
	output_img: &mut Vec<u8>,
	encoded_line: &[u8],
	filter_type: FilterType,
	y_idx: usize,
) {
	match filter_type {
		FilterType::None => {
			output_img.extend_from_slice(encoded_line);
		}
		FilterType::Sub => {
			for x_idx in 0..encoded_line.len() {
				output_img.push(
					(((encoded_line[x_idx] as u16)
						+ ((x_idx > 3)
							.then(|| {
								output_img
									.get(y_idx * encoded_line.len() + (x_idx - 4))
									.copied()
									.unwrap_or_default()
							})
							.unwrap_or_default() as u16))
						& 0xff) as u8,
				)
			}
		}
		FilterType::Up => {
			for x_idx in 0..encoded_line.len() {
				output_img.push(
					(((encoded_line[x_idx] as u16)
						+ ((y_idx > 0)
							.then(|| {
								output_img
									.get((y_idx - 1) * encoded_line.len() + x_idx)
									.copied()
									.unwrap_or_default()
							})
							.unwrap_or_default() as u16))
						& 0xff) as u8,
				);
			}
		}
		FilterType::Average => {
			for x_idx in 0..encoded_line.len() {
				output_img.push(
					(((encoded_line[x_idx] as u16)
						+ (((x_idx > 3)
							.then(|| {
								output_img
									.get(y_idx * encoded_line.len() + (x_idx - 4))
									.copied()
									.unwrap_or_default()
							})
							.unwrap_or_default() as u16) + ((y_idx > 0)
							.then(|| {
								output_img
									.get((y_idx - 1) * encoded_line.len() + x_idx)
									.copied()
									.unwrap_or_default()
							})
							.unwrap_or_default() as u16))
							/ 2) & 0xff) as u8,
				);
			}
		}
		FilterType::Paeth => {
			for x_idx in 0..encoded_line.len() {
				let a = (x_idx > 3)
					.then(|| output_img[y_idx * encoded_line.len() + (x_idx - 4)])
					.unwrap_or_default() as i16;
				let b = (y_idx > 0)
					.then(|| output_img[(y_idx - 1) * encoded_line.len() + x_idx])
					.unwrap_or_default() as i16;
				let c = (x_idx > 3 && y_idx > 0)
					.then(|| output_img[(y_idx - 1) * encoded_line.len() + (x_idx - 4)])
					.unwrap_or_default() as i16;
				output_img
					.push((((encoded_line[x_idx] as u16) + (paeth(a, b, c) as u16)) & 0xff) as u8);
			}
		}
	}
}

fn paeth(a: i16, b: i16, c: i16) -> u8 {
	let p = a + b - c;
	let pa = (p - a).abs();
	let pb = (p - b).abs();
	let pc = (p - c).abs();
	if pa <= pb && pa <= pc {
		a as u8
	} else if pb <= pc {
		b as u8
	} else {
		c as u8
	}
}
