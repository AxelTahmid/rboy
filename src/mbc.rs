use util::handle_io;

pub trait MBC {
	fn readrom(&self, a: u16) -> u8;
	fn readram(&self, a: u16) -> u8;
	fn writerom(&mut self, a: u16, v: u8);
	fn writeram(&mut self, a: u16, v: u8);
}

struct MBC0 {
	rom: Vec<u8>,
}

impl MBC0 {
	pub fn new(data: Vec<u8>) -> Option<MBC0> {
		Some(MBC0 { rom: data })
	}
}

struct MBC1 {
	rom: Vec<u8>,
	ram: Vec<u8>,
	ram_on: bool,
	ram_mode: bool,
	rombank: uint,
	rambank: uint,
	savepath: Option<Path>,
}

impl MBC1 {
	pub fn new(data: Vec<u8>, file: &Path) -> Option<MBC1> {
		let (svpath, ramsize) = match *data.get(0x147) {
			0x02 => (None, ram_size(*data.get(0x149))),
			0x03 => (Some(file.with_extension("gbsave")), ram_size(*data.get(0x149))),
			_ => (None, 0),
		};

		let mut res = MBC1 {
			rom: data,
			ram: ::std::vec::Vec::from_elem(ramsize, 0u8),
			ram_on: false,
			ram_mode: false,
			rombank: 1,
			rambank: 0,
			savepath: svpath,
		};
		match res.loadram()
		{
			false => None,
			true => Some(res),
		}
	}

	fn loadram(&mut self) -> bool {
		match self.savepath {
			None => {},
			Some(ref savepath) => if savepath.is_file()
			{
				self.ram = match ::std::io::File::open(savepath).read_to_end()
				{
					Err(_) => { error!("Could not open save file"); return false },
					Ok(data) => data,
				}
			},
		};
		true
	}
}

impl Drop for MBC1 {
	fn drop(&mut self) {
		match self.savepath {
			None => {},
			Some(ref path) =>
			{
				handle_io(::std::io::File::create(path).write(self.ram.as_slice()), "Could not write savefile");
			},
		};
	}
}

struct MBC3 {
	rom: Vec<u8>,
	ram: Vec<u8>,
	rombank: uint,
	rambank: uint,
	ram_on: bool,
	savepath: Option<Path>,
	rtc_ram: [u8,.. 5],
	rtc_lock: bool,
	rtc_zero: Option<i64>,
}

impl MBC3 {
	pub fn new(data: Vec<u8>, file: &Path) -> Option<MBC3> {
		let subtype = *data.get(0x147);
		let svpath = match subtype {
			0x0F | 0x10 | 0x13 => Some(file.with_extension("gbsave")),
			_ => None,
		};
		let ramsize = match subtype {
			0x10 | 0x12 | 0x13 => ram_size(*data.get(0x149)),
			_ => 0,
		};
		let rtc = match subtype {
			0x0F | 0x10 => Some(0),
			_ => None,
		};

		let mut res = MBC3 {
			rom: data,
			ram: ::std::vec::Vec::from_elem(ramsize, 0u8),
			rombank: 1,
			rambank: 0,
			ram_on: false,
			savepath: svpath,
			rtc_ram: [0u8,.. 5],
			rtc_lock: false,
			rtc_zero: rtc,
		};
		match res.loadram()
		{
			false => None,
			true => Some(res),
		}
	}

	fn loadram(&mut self) -> bool {
		match self.savepath {
			None => {},
			Some(ref savepath) => if savepath.is_file() {
				let mut file = ::std::io::File::open(savepath);
				let rtc = match handle_io(file.read_be_i64(), "Could not read RTC") {
					None => { return false; },
					Some(value) => value,
				};
				if self.rtc_zero.is_some() { self.rtc_zero = Some(rtc); }
				self.ram = match handle_io(file.read_to_end(), "Could not read ROM") {
					None => { return false; },
					Some(data) => data,
				};
			},
		};
		true
	}

	fn calc_rtc_reg(&mut self) {
		let tzero = match self.rtc_zero {
			Some(t) => t,
			None => return,
		};
		if self.rtc_ram[4] & 0x40 == 0x40 { return }

		let difftime: i64 = match ::time::get_time().sec - tzero {
			n if n >= 0 => { n },
			_ => { 0 },
		};
		self.rtc_ram[0] = (difftime % 60) as u8;
		self.rtc_ram[1] = ((difftime / 60) % 60) as u8;
		self.rtc_ram[2] = ((difftime / 3600) % 24) as u8;
		let days: i64 = difftime / (3600*24);
		self.rtc_ram[3] = days as u8;
		self.rtc_ram[4] = (self.rtc_ram[4] & 0xFE) | (((days >> 8) & 0x01) as u8);
		if days >= 512 {
			self.rtc_ram[4] |= 0x80;
			self.calc_rtc_zero();
		}
	}

	fn calc_rtc_zero(&mut self) {
		if self.rtc_zero.is_none() { return }
		let mut difftime: i64 = ::time::get_time().sec;
		difftime -= self.rtc_ram[0] as i64;
		difftime -= (self.rtc_ram[1] as i64) * 60;
		difftime -= (self.rtc_ram[2] as i64) * 3600;
		let days = ((self.rtc_ram[4] as i64 & 0x1) << 8) | (self.rtc_ram[3] as i64);
		difftime -= days * 3600 * 24;
		self.rtc_zero = Some(difftime);
	}
}

impl Drop for MBC3 {
	fn drop(&mut self) {
		match self.savepath {
			None => {},
			Some(ref path) => {
				let mut file = ::std::io::File::create(path);
				let rtc = match self.rtc_zero {
					Some(t) => t,
					None => 0,
				};
				let mut ok = true;
				if ok { ok = handle_io(file.write_be_i64(rtc), "Could not write savefile").is_some(); };
				if ok { handle_io(file.write(self.ram.as_slice()), "Could not write savefile"); };
			},
		};
	}
}

struct MBC5 {
	rom: Vec<u8>,
	ram: Vec<u8>,
	rombank: uint,
	rambank: uint,
	ram_on: bool,
	savepath: Option<Path>,
}

impl MBC5 {
	pub fn new(data: Vec<u8>, file: &Path) -> Option<MBC5> {
		let subtype = *data.get(0x147);
		let svpath = match subtype {
			0x1B | 0x1E => Some(file.with_extension("gbsave")),
			_ => None,
		};
		let ramsize = match subtype {
			0x1A | 0x1B | 0x1D | 0x1E => ram_size(*data.get(0x149)),
			_ => 0,
		};

		let mut res = MBC5 {
			rom: data,
			ram: ::std::vec::Vec::from_elem(ramsize, 0u8),
			rombank: 1,
			rambank: 0,
			ram_on: false,
			savepath: svpath,
		};
		match res.loadram()
		{
			false => None,
			true => Some(res),
		}
	}

	fn loadram(&mut self) -> bool {
		match self.savepath {
			None => {},
			Some(ref savepath) => if savepath.is_file() {
				self.ram = match ::std::io::File::open(savepath).read_to_end() {
					Err(_) => { error!("Could not read RAM"); return false; },
					Ok(data) => data,
				};
			},
		};
		true
	}
}

impl Drop for MBC5 {
	fn drop(&mut self) {
		match self.savepath {
			None => {},
			Some(ref path) =>
			{
				handle_io(::std::io::File::create(path).write(self.ram.as_slice()), "Could not write savefile");
			},
		};
	}
}

pub fn get_mbc(file: &Path) -> Option<Box<MBC>> {
	let data: Vec<u8> = match handle_io(::std::io::File::open(file).read_to_end(), "Could not read ROM")
	{
		Some(mbc) => { mbc },
		None => { return None; },
	};
	if data.len() < 0x150 { fail!("Rom size to small"); }
	if !check_checksum(&data)
	{
		return None;
	}
	match *data.get(0x147) {
		0x00 => MBC0::new(data).map(|v| box v as Box<MBC>),
		0x01 .. 0x03 => MBC1::new(data, file).map(|v| box v as Box<MBC>),
		0x0F .. 0x13 => MBC3::new(data, file).map(|v| box v as Box<MBC>),
		0x19 .. 0x1E => MBC5::new(data, file).map(|v| box v as Box<MBC>),
		m => { error!("Unsupported MBC type: {:02X}", m); None },
	}
}

fn ram_size(v: u8) -> uint {
	match v {
		1 => 0x800,
		2 => 0x2000,
		3 => 0x8000,
		4 => 0x20000,
		_ => 0,
	}
}

fn check_checksum(data: &Vec<u8>) -> bool {
	let mut value: u8 = 0;
	for i in range(0x134u, 0x14D) {
		value = value - *data.get(i) - 1;
	}
	match *data.get(0x14D) == value
	{
		true => true,
		false => { error!("Cartridge checksum is invalid. {:02X} != {:02X}", *data.get(0x14D), value); false },
	}
}

impl MBC for MBC0 {
	fn readrom(&self, a: u16) -> u8 { *self.rom.get(a as uint) }
	fn readram(&self, _a: u16) -> u8 { 0 }
	fn writerom(&mut self, _a: u16, _v: u8) { () }
	fn writeram(&mut self, _a: u16, _v: u8) { () }
}

impl MBC for MBC1 {
	fn readrom(&self, a: u16) -> u8 {
		if a < 0x4000 { *self.rom.get(a as uint) }
		else { *self.rom.get(self.rombank * 0x4000 | ((a as uint) & 0x3FFF) ) }
	}
	fn readram(&self, a: u16) -> u8 {
		if !self.ram_on { return 0 }
		let rambank = if self.ram_mode { self.rambank } else { 0 };
		*self.ram.get((rambank * 0x2000) | ((a & 0x1FFF) as uint))
	}

	fn writerom(&mut self, a: u16, v: u8) {
		match a {
			0x0000 .. 0x1FFF => { self.ram_on = v == 0x0A; },
			0x2000 .. 0x3FFF => {
				self.rombank = (self.rombank & 0x60) | match (v as uint) & 0x1F { 0 => 1, n => n }
			},
			0x4000 .. 0x5FFF => {
				if !self.ram_mode {
					self.rombank = self.rombank & 0x1F | (((v as uint) & 0x03) << 5)
				} else {
					self.rambank = (v as uint) & 0x03;
				}
			},
			0x6000 .. 0x7FFF => { self.ram_mode = (v & 0x01) == 0x01; },
			_ => fail!("Could not write to {:04X} (MBC1)", a),
		}
	}

	fn writeram(&mut self, a: u16, v: u8) {
		if !self.ram_on { return }
		let rambank = if self.ram_mode { self.rambank } else { 0 };
		*self.ram.get_mut((rambank * 0x2000) | ((a & 0x1FFF) as uint)) = v;
	}
}

impl MBC for MBC3 {
	fn readrom(&self, a: u16) -> u8 {
		if a < 0x4000 { *self.rom.get(a as uint) }
		else { *self.rom.get(self.rombank * 0x4000 | ((a as uint) & 0x3FFF)) }
	}
	fn readram(&self, a: u16) -> u8 {
		if !self.ram_on { return 0 }
		if self.rambank <= 3 {
			*self.ram.get(self.rambank * 0x2000 | ((a as uint) & 0x1FFF))
		} else {
			self.rtc_ram[self.rambank - 0x08]
		}
	}
	fn writerom(&mut self, a: u16, v: u8) {
		match a {
			0x0000 .. 0x1FFF => self.ram_on = v == 0x0A,
			0x2000 .. 0x3FFF => {
				self.rombank = match v & 0x7F { 0 => 1, n => n as uint }
			},
			0x4000 .. 0x5FFF => self.rambank = v as uint,
			0x6000 .. 0x7FFF => match v {
				0 => self.rtc_lock = false,
				1 => {
					if !self.rtc_lock { self.calc_rtc_reg(); };
					self.rtc_lock = true;
				},
				_ => {},
			},
			_ => fail!("Could not write to {:04X} (MBC3)", a),
		}
	}
	fn writeram(&mut self, a: u16, v: u8) {
		if self.ram_on == false { return }
		if self.rambank <= 3 {
			*self.ram.get_mut(self.rambank * 0x2000 | ((a as uint) & 0x1FFF)) = v;
		} else {
			self.rtc_ram[self.rambank - 0x8] = v;
			self.calc_rtc_zero();
		}
	}
}
impl MBC for MBC5 {
	fn readrom(&self, a: u16) -> u8 {
		if a < 0x4000 { *self.rom.get(a as uint) }
		else { *self.rom.get(self.rombank * 0x4000 | ((a as uint) & 0x3FFF)) }
	}
	fn readram(&self, a: u16) -> u8 {
		if !self.ram_on { return 0 }
		*self.ram.get(self.rambank * 0x2000 | ((a as uint) & 0x1FFF))
	}
	fn writerom(&mut self, a: u16, v: u8) {
		match a {
			0x0000 .. 0x1FFF => self.ram_on = v == 0x0A,
			0x2000 .. 0x2FFF => self.rombank = (self.rombank & 0x100) | (v as uint),
			0x3000 .. 0x3FFF => self.rombank = (self.rombank & 0x0FF) | ((v & 0x1) as uint << 8),
			0x4000 .. 0x5FFF => self.rambank = (v & 0x0F) as uint,
			0x6000 .. 0x7FFF => { /* ? */ },
			_ => fail!("Could not write to {:04X} (MBC5)", a),
		}
	}
	fn writeram(&mut self, a: u16, v: u8) {
		if self.ram_on == false { return }
		*self.ram.get_mut(self.rambank * 0x2000 | ((a as uint) & 0x1FFF)) = v;
	}
}

