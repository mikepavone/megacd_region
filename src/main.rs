mod cdrom;
use std::cmp::{max, min};
use std::env::args_os;
use std::fs::File;
use std::io::{Result, Write};
use encoding_rs::SHIFT_JIS;

fn be32(bytes: &[u8]) -> u32 {
	(bytes[0] as u32) << 24 |
	(bytes[1] as u32) << 16 |
	(bytes[2] as u32) << 8 |
	(bytes[3] as u32)
}

fn write_be32(bytes: &mut [u8], value: u32) {
	bytes[0] = (value >> 24) as u8;
	bytes[1] = (value >> 16) as u8;
	bytes[2] = (value >> 8) as u8;
	bytes[3] = value as u8;
}

fn write_be16(bytes: &mut [u8], value: u16) {
	bytes[0] = (value >> 8) as u8;
	bytes[1] = value as u8;
}

#[derive(Debug)]
#[derive(Clone)]
#[derive(Copy)]
enum Region {
	Japan,
	Usa,
	Europe,
}

impl Region {
	fn security_size(&self) -> usize {
		self.security_code().len()
	}
	fn security_code(&self) -> &[u8] {
		match self {
			Region::Japan => include_bytes!("../security_bins/japan_security.bin"),
			Region::Usa => include_bytes!("../security_bins/usa_security.bin"),
			Region::Europe => include_bytes!("../security_bins/europe_security.bin"),
		}
	}
	fn adapter_code(&self, dest_region: Region) -> &[u8] {
		match self {
			Region::Japan => match dest_region {
				Region::Japan => &[],
				Region::Usa => include_bytes!("../security_bins/japan_to_usa.bin"),
				Region::Europe => include_bytes!("../security_bins/japan_to_europe.bin"),
			},
			Region::Usa => match dest_region {
				Region::Japan => include_bytes!("../security_bins/usa_to_japan.bin"),
				Region::Usa => &[],
				Region::Europe => include_bytes!("../security_bins/usa_to_europe.bin"),
			},
			Region::Europe => match dest_region {
				Region::Japan => include_bytes!("../security_bins/europe_to_japan.bin"),
				Region::Usa => include_bytes!("../security_bins/europe_to_usa.bin"),
				Region::Europe => &[],
			},
		}
	}
	fn region_char(&self) -> u8 {
		match self {
			Region::Japan => b'J',
			Region::Usa => b'U',
			Region::Europe => b'E',
		}
	}
	fn inject_size(&self, dest_region: Region, ip: &mut Vec<u8>) {
		match self {
			Region::Japan => match dest_region {
				Region::Japan => {},
				Region::Usa => {
					let delta = ((ip.len() - 0x7B6) >> 1) as u16;
					write_be16(&mut ip[0x7A8..0x7AA], delta);
				},
				Region::Europe => {},
			},
			Region::Usa => match dest_region {
				Region::Japan => {},
				Region::Usa => {},
				Region::Europe => {},
			},
			Region::Europe => match dest_region {
				Region::Japan => {},
				Region::Usa => {},
				Region::Europe => {},
			},
		};
	}
}
fn round_sector(num: u32) -> u32 {
	(num + 0x7FF) & !0x7FF
}

fn main() -> Result<()> {
    println!("Hello, world!");
	let fname = match args_os().nth(1) {
		None => panic!("Usage: megacd_region FILE"),
		Some(p) => p,
	};
	
	let file = match File::open(fname.clone()) {
		Err(reason) => panic!("Failed to open file {}: {}", fname.to_string_lossy(), reason),
		Ok(f) => f,
	};
	let mut image = cdrom::Image::new(file)?;
	let sectors = image.read_sectors(0, 1)?;
	let header = &sectors[0];
	let (domestic_title, _encoding, _errors) = SHIFT_JIS.decode(&header.data[0x120..0x150]);
	let (overseas_title, _encoding, _errors) = SHIFT_JIS.decode(&header.data[0x150..0x180]);//String::from_utf8_lossy();
	println!("Domestic Title: {}", domestic_title);
	println!("Overseas Title: {}", overseas_title);
	let ip_start = be32(&header.data[0x30..0x34]);
	let ip_length = be32(&header.data[0x34..0x38]);
	let sp_start = be32(&header.data[0x40..0x44]);
	let sp_length = be32(&header.data[0x44..0x48]);
	println!("IP Start: {ip_start:X} Length: {ip_length:X}");
	println!("SP Start: {sp_start:X} Length: {sp_length:X}");
	let region = match header.data[0x20B] {
		0x64 => Region::Europe,
		0x7A => Region::Usa,
		0xA1 => Region::Japan,
		other => panic!("Image has invalid security code, byte at 0x20B is {other:02X}")
	};
	println!("Region: {region:?}, Security Size: {:X}", region.security_size());
	
	let outname = match args_os().nth(2) {
		None => { return Ok(()); },
		Some(n) => n,
	};
	let mut out = match File::create(outname.clone()) {
		Err(reason) => panic!("Failed to create file {}: {reason}", outname.to_string_lossy()),
		Ok(f) => f,
	};
	
	//TODO: make this configurable
	let dest_region = Region::Usa;
	let security_code = dest_region.security_code();
	let adapter = region.adapter_code(dest_region);
	
	let sp_sector_start = sp_start >> 11;
	let sp_sector_end = sp_sector_start + (sp_length >> 11);
	let mut extra_sector_start = sp_sector_start;
	let mut extra_sector_end = sp_sector_end;
	let (ip_start_sector, ip_end_sector) = if ip_start == 0x200 {
		if ip_length > 0x600 {
			(1, 1 + ((ip_length - 0x600) >> 11))
		} else {
			(0, 0)
		}
	} else {
		(ip_start >> 11, (ip_start >> 11) + (ip_length >> 11))
	};
	if ip_start_sector != ip_end_sector {
		extra_sector_start = min(extra_sector_start, ip_start_sector);
		extra_sector_end = max(extra_sector_end, ip_end_sector);
	}
	let extra = match image.read_sectors(extra_sector_start, extra_sector_end - extra_sector_start) {
		Ok(s) => s,
		Err(reason) => panic!("Failed to read boot code: {reason}")
	};
	
	
	let mut ip = Vec::with_capacity(0x800 - region.security_size() + security_code.len() + adapter.len() + ((ip_end_sector - ip_start_sector) as usize) * 0x800);
	ip.extend_from_slice(&header.data[0..0x200]);
	ip.extend_from_slice(&security_code);
	ip.extend_from_slice(&adapter);
	ip.extend_from_slice(&header.data[(0x200 + region.security_size())..0x800]);
	for sector in ip_start_sector..ip_end_sector {
		ip.extend_from_slice(&extra[(sector - extra_sector_start) as usize].data);
	}
	let mut sp = Vec::with_capacity(((sp_sector_end - sp_sector_start) as usize) * 0x800);
	for sector in sp_sector_start..sp_sector_end {
		sp.extend_from_slice(&extra[(sector - extra_sector_start) as usize].data);
	}
	let new_ip_start = if ip.len() <= 0x800 { 0x200 } else { 0x800 };
	let ip_rounded = round_sector(ip.len() as u32);
	let new_ip_len = if ip.len() <= 0x800 { 0x600 } else { ip_rounded - 0x800 };
	if (ip_rounded as usize + sp.len()) > 0x8000 {
		println!("WARNING: Boot code is too big, truncating SP. Original size {}, new size {}", sp.len(), 0x8000 - ip_rounded);
		sp.truncate((0x8000 - ip_rounded) as usize);
	}
	write_be32(&mut ip[0x30..0x34], new_ip_start);
	write_be32(&mut ip[0x34..0x38], new_ip_len);
	write_be32(&mut ip[0x40..0x44], ip_rounded);
	write_be32(&mut ip[0x44..0x48], round_sector(sp.len() as u32));
	ip[0x1f0] = dest_region.region_char();
	ip[0x1f1] = b' ';
	ip[0x1f2] = b' ';
	region.inject_size(dest_region, &mut ip);
	
	ip.extend(std::iter::repeat(0).take((round_sector(ip.len() as u32) as usize) - ip.len()));
	
	out.write_all(&ip)?;
	out.write_all(&sp)?;
	let rest_sector = round_sector((ip.len() + sp.len()) as u32) >> 11;
	let rest = image.read_sectors(rest_sector, image.num_sectors() - rest_sector)?;
	for sector in rest {
		out.write_all(&sector.data)?;
	}
	Ok(())
}
