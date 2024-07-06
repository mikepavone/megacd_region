use std::fs::File;
use std::io::{Read, Result, Seek, SeekFrom};
use std::vec::Vec;

pub struct Image {
	f: File,
	sectors: u32,
	data_only: bool,
}

pub struct Mode1Sector {
	min: u8,
	sec: u8,
	frame: u8,
	crc: u32,
	pub data: [u8; 2048],
	rspc: [u8; 276],
	rspc_crc_valid: bool,
}

fn from_bcd(num: u8) -> u8 {
	(num & 0xF) + (num >> 4) * 10
}

impl Image {
	pub fn new(mut f: File) -> Result<Image> {
		f.seek(SeekFrom::Start(0))?;
		let mut sync = [0u8; 12];
		f.read_exact(&mut sync)?;
		const BIN_SYNC: [u8; 12] = [0u8, 0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8, 0xFFu8, 0u8];
		let fsize = f.seek(SeekFrom::End(0))?;
		let data_only = sync != BIN_SYNC;
		let sectors = (fsize / if data_only { 2048 } else { 2352 }) as u32;
		Ok(Image { f, sectors, data_only })
	}
	
	pub fn read_sectors(&mut self, start: u32, length: u32) -> Result<Vec<Mode1Sector>> {
		let mut results = Vec::with_capacity(length as usize);
		let pos = (start as u64) * (if self.data_only { 2048 } else { 2352 });
		self.f.seek(SeekFrom::Start(pos))?;
		let end = start + length;
		for lba in start..end {
			let mut sector = Mode1Sector {
				min: 0, sec: 0, frame: 0, crc: 0,
				data: [0; 2048],
				rspc: [0; 276],
				rspc_crc_valid: false,
			};
			if self.data_only {
				self.f.read_exact(&mut sector.data)?;
				let lba_header = lba + 150;
				sector.frame = (lba_header % 75) as u8;
				let seconds = lba_header / 75;
				sector.sec = (seconds % 60) as u8;
				sector.min = (seconds / 60) as u8;
			} else {
				let mut header = [0u8; 16];
				let mut footer = [0u8; 12];
				
				self.f.read_exact(&mut header)?;
				self.f.read_exact(&mut sector.data)?;
				self.f.read_exact(&mut footer)?;
				self.f.read_exact(&mut sector.rspc)?;
				sector.min = from_bcd(header[12]);
				sector.sec = from_bcd(header[13]);
				sector.frame = from_bcd(header[14]);
				sector.crc = footer[0] as u32;
				sector.crc |= (footer[1] as u32) << 8;
				sector.crc |= (footer[2] as u32) << 16;
				sector.crc |= (footer[3] as u32) << 24;
				sector.rspc_crc_valid = true;
			}
			results.push(sector);
		}
		Ok(results)
	}
	
	pub fn num_sectors(&self) -> u32 {
		self.sectors
	}
}

impl Mode1Sector {
	pub fn ensure_rspc_crc_valid(&mut self) {
		if !self.rspc_crc_valid {
			panic!("not implemented yet");
		}
	}
}