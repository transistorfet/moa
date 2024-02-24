
use femtos::Instant;

use moa_core::{Error, Address, Addressable, BusPort};

use crate::state::{M68k, Exceptions};
use crate::instructions::Size;

#[repr(u8)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FunctionCode {
    Reserved0           = 0,
    UserData            = 1,
    UserProgram         = 2,
    Reserved3           = 3,
    Reserved4           = 4,
    SupervisorData      = 5,
    SupervisorProgram   = 6,
    CpuSpace            = 7,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MemType {
    Program,
    Data,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MemAccess {
    Read,
    Write,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
// TODO change to MemoryState or RequestState or AccessState or maybe even BusState
pub struct MemoryRequest {
    pub i_n_bit: bool,
    pub access: MemAccess,
    pub code: FunctionCode,
    pub size: Size,
    pub address: u32,
}

impl FunctionCode {
    pub fn program(is_supervisor: bool) -> Self {
        if is_supervisor {
            FunctionCode::SupervisorProgram
        } else {
            FunctionCode::UserProgram
        }
    }

    pub fn data(is_supervisor: bool) -> Self {
        if is_supervisor {
            FunctionCode::SupervisorData
        } else {
            FunctionCode::UserData
        }
    }
}

impl Default for MemoryRequest {
    fn default() -> Self {
        Self {
            i_n_bit: false,
            access: MemAccess::Read,
            code: FunctionCode::Reserved0,
            size: Size::Word,
            address: 0,
        }
    }
}

impl MemoryRequest {
    pub fn get_type_code(&self) -> u16 {
        let ins = match self.i_n_bit {
            false => 0x0000,
            true => 0x0008,
        };

        let rw = match self.access {
            MemAccess::Write => 0x0000,
            MemAccess::Read => 0x0010,
        };

        ins | rw | (self.code as u16)
    }
}

#[derive(Clone)]
pub struct M68kBusPort {
    pub port: BusPort,
    pub request: MemoryRequest,
    pub cycle_start_clock: Instant,
    pub current_clock: Instant,
}


impl M68k {
    // TODO should some of the ones from execute.rs move here
}

impl M68kBusPort {
    pub fn new(port: BusPort) -> Self {
        Self {
            port,
            request: Default::default(),
            cycle_start_clock: Instant::START,
            current_clock: Instant::START,
        }
    }

    pub fn data_width(&self) -> u8 {
        self.port.data_width()
    }

    pub fn init_cycle(&mut self, clock: Instant) {
        self.cycle_start_clock = clock;
        self.current_clock = clock;
    }

    pub(crate) fn read_instruction_word(&mut self, is_supervisor: bool, addr: u32) -> Result<u16, Error> {
        self.start_instruction_request(is_supervisor, addr)?;
        self.port.read_beu16(self.current_clock, addr as Address)
    }

    pub(crate) fn read_instruction_long(&mut self, is_supervisor: bool, addr: u32) -> Result<u32, Error> {
        self.start_instruction_request(is_supervisor, addr)?;
        self.port.read_beu32(self.current_clock, addr as Address)
    }

    pub(crate) fn read_data_sized(&mut self, is_supervisor: bool, addr: Address, size: Size) -> Result<u32, Error> {
        self.start_request(is_supervisor, addr as u32, size, MemAccess::Read, MemType::Data, false)?;
        match size {
            Size::Byte => self.port.read_u8(self.current_clock, addr).map(|value| value as u32),
            Size::Word => self.port.read_beu16(self.current_clock, addr).map(|value| value as u32),
            Size::Long => self.port.read_beu32(self.current_clock, addr),
        }
    }

    pub(crate) fn write_data_sized(&mut self, is_supervisor: bool, addr: Address, value: u32, size: Size) -> Result<(), Error> {
        self.start_request(is_supervisor, addr as u32, size, MemAccess::Write, MemType::Data, false)?;
        match size {
            Size::Byte => self.port.write_u8(self.current_clock, addr, value as u8),
            Size::Word => self.port.write_beu16(self.current_clock, addr, value as u16),
            Size::Long => self.port.write_beu32(self.current_clock, addr, value),
        }
    }

    pub(crate) fn start_instruction_request(&mut self, is_supervisor: bool, addr: u32) -> Result<u32, Error> {
        self.request.i_n_bit = false;
        self.request.code = FunctionCode::program(is_supervisor);
        self.request.access = MemAccess::Read;
        self.request.address = addr;

        validate_address(addr)
    }

    pub(crate) fn start_request(&mut self, is_supervisor: bool, addr: u32, size: Size, access: MemAccess, mtype: MemType, i_n_bit: bool) -> Result<u32, Error> {
        self.request.i_n_bit = i_n_bit;
        self.request.code = match mtype {
            MemType::Program => FunctionCode::program(is_supervisor),
            MemType::Data => FunctionCode::data(is_supervisor),
        };

        self.request.access = access;
        self.request.address = addr;

        if size == Size::Byte {
            Ok(addr)
        } else {
            validate_address(addr)
        }
    }

    pub(crate) fn dump_memory(&mut self, addr: u32, length: usize) {
        self.port.dump_memory(self.current_clock, addr as Address, length as u64);
    }
}

fn validate_address(addr: u32) -> Result<u32, Error> {
    if addr & 0x1 == 0 {
        Ok(addr)
    } else {
        Err(Error::processor(Exceptions::AddressError as u32))
    }
}



/*
pub(crate) struct TargetAccess {
    must_read: bool,
    must_write: bool,
    size: Size,
    target: Target,
}

impl TargetAccess {
    pub(crate) fn read_only(size: Size) -> Self {

    }

    pub(crate) fn read_update(size: Size) -> Self {

    }

    pub(crate) fn updated_only(size: Size) -> Self {

    }

    pub(crate) fn get(&mut self, cpu: &M68k) -> Result<u32, Error> {

    }

    pub(crate) fn set(&mut self, cpu: &M68k, value: u32) -> Result<(), Error> {

    }

    pub(crate) fn complete(&self) -> Result<Self, Error> {
        
    }
}


impl Target {
    pub(crate) fn read_once(self, size: Size) -> ReadOnceAccess {
        ReadOnceAccess {
            size,
            target: self,
            accessed: false,
        }
    }

    pub(crate) fn read_update(self, size: Size) -> ReadUpdateAccess {
        ReadUpdateAccess {
            size,
            target: self,
        }
    }

    pub(crate) fn write_once(self, size: Size) -> WriteOnceAccess {
        WriteOnceAccess {
            size,
            target: self,
        }
    }
}



pub(crate) struct ReadOnceAccess {
    size: Size,
    target: Target,
    accessed: bool,
}

impl ReadOnceAccess {
    pub(crate) fn get(&mut self, cpu: &M68k) -> Result<u32, Error> {

    }

    pub(crate) fn complete(&self) -> Result<Self, Error> {
        
    }
}

pub(crate) struct ReadUpdateAccess {
    size: Size,
    target: Target,
}

impl ReadUpdateAccess {
    pub(crate) fn get(&mut self, cpu: &M68k) -> Result<u32, Error> {

    }

    pub(crate) fn set(&mut self, cpu: &M68k, value: u32) -> Result<(), Error> {

    }

    pub(crate) fn complete(&self) -> Result<Self, Error> {
        
    }
}

pub(crate) struct WriteOnceAccess {
    size: Size,
    target: Target,
}

impl WriteOnceAccess {
    pub(crate) fn set(&mut self, cpu: &M68k, value: u32) -> Result<(), Error> {

    }

    pub(crate) fn complete(&self) -> Result<Self, Error> {
        
    }
}
*/
