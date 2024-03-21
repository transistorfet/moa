use core::cmp;
use core::fmt::Write;
use emulator_hal::time;
use emulator_hal::bus::BusAccess;

use crate::{M68kError, CpuInfo};
use crate::state::Exceptions;
use crate::instructions::Size;

#[repr(u8)]
#[allow(dead_code)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[rustfmt::skip]
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
pub struct MemoryRequest<Instant> {
    pub i_n_bit: bool,
    pub access: MemAccess,
    pub code: FunctionCode,
    pub size: Size,
    pub address: u32,
    pub clock: Instant,
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

impl<Instant> Default for MemoryRequest<Instant>
where
    Instant: time::Instant,
{
    fn default() -> Self {
        Self {
            i_n_bit: false,
            access: MemAccess::Read,
            code: FunctionCode::Reserved0,
            size: Size::Word,
            address: 0,
            clock: Instant::START,
        }
    }
}

impl<Instant> MemoryRequest<Instant> {
    fn new(clock: Instant) -> Self {
        Self {
            i_n_bit: false,
            access: MemAccess::Read,
            code: FunctionCode::Reserved0,
            size: Size::Word,
            address: 0,
            clock,
        }
    }

    pub(crate) fn instruction<BusError>(&mut self, is_supervisor: bool, addr: u32) -> Result<u32, M68kError<BusError>> {
        self.i_n_bit = false;
        self.code = FunctionCode::program(is_supervisor);
        self.access = MemAccess::Read;
        self.address = addr;

        validate_address(addr)
    }

    #[inline]
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

//pub type M68kAddress = (FunctionCode, u32);
pub type M68kAddress = u32;
pub type M68kAddressSpace = (FunctionCode, u32);

#[derive(Clone, Debug)]
pub struct InstructionRequest<Instant> {
    pub request: MemoryRequest<Instant>,
    pub current_clock: Instant,
}

#[derive(Clone, Debug)]
pub struct M68kBusPort<Instant> {
    pub request: MemoryRequest<Instant>,
    pub data_bytewidth: usize,
    pub address_mask: u32,
    pub cycle_start_clock: Instant,
    pub current_clock: Instant,
}


impl<Instant> Default for M68kBusPort<Instant>
where
    Instant: time::Instant,
{
    fn default() -> Self {
        Self {
            request: Default::default(),
            data_bytewidth: 32 / 8,
            address_mask: 0xFFFF_FFFF,
            cycle_start_clock: Instant::START,
            current_clock: Instant::START,
        }
    }
}

impl<Instant> M68kBusPort<Instant>
where
    Instant: Copy,
{
    pub fn from_info(info: &CpuInfo, clock: Instant) -> Self {
        Self {
            request: MemoryRequest::new(clock),
            data_bytewidth: info.data_width as usize / 8,
            address_mask: 1_u32.checked_shl(info.address_width as u32).unwrap_or(0).wrapping_sub(1),
            cycle_start_clock: clock,
            current_clock: clock,
        }
    }

    fn read<Bus, BusError>(
        &mut self,
        bus: &mut Bus,
        clock: Instant,
        addr: M68kAddress,
        data: &mut [u8],
    ) -> Result<(), M68kError<BusError>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    {
        let addr = addr & self.address_mask;
        for i in (0..data.len()).step_by(self.data_bytewidth) {
            let addr_index = (addr + i as M68kAddress) & self.address_mask;
            let end = cmp::min(i + self.data_bytewidth, data.len());
            bus.read(clock, addr_index, &mut data[i..end])
                .map_err(|err| M68kError::BusError(err))?;
        }
        Ok(())
    }

    fn write<Bus, BusError>(
        &mut self,
        bus: &mut Bus,
        clock: Instant,
        addr: M68kAddress,
        data: &[u8],
    ) -> Result<(), M68kError<BusError>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    {
        let addr = addr & self.address_mask;
        for i in (0..data.len()).step_by(self.data_bytewidth) {
            let addr_index = (addr + i as M68kAddress) & self.address_mask;
            let end = cmp::min(i + self.data_bytewidth, data.len());
            bus.write(clock, addr_index, &data[i..end])
                .map_err(|err| M68kError::BusError(err))?;
        }
        Ok(())
    }

    fn read_sized<Bus, BusError>(&mut self, bus: &mut Bus, addr: M68kAddress, size: Size) -> Result<u32, M68kError<BusError>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    {
        let mut data = [0; 4];
        match size {
            Size::Byte => self.read(bus, self.current_clock, addr, &mut data[3..4]),
            Size::Word => self.read(bus, self.current_clock, addr, &mut data[2..4]),
            Size::Long => self.read(bus, self.current_clock, addr, &mut data[0..4]),
        }
        .map(|_| u32::from_be_bytes(data))
    }

    fn write_sized<Bus, BusError>(
        &mut self,
        bus: &mut Bus,
        addr: M68kAddress,
        size: Size,
        value: u32,
    ) -> Result<(), M68kError<BusError>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    {
        let data = value.to_be_bytes();
        match size {
            Size::Byte => self.write(bus, self.current_clock, addr, &data[3..4]),
            Size::Word => self.write(bus, self.current_clock, addr, &data[2..4]),
            Size::Long => self.write(bus, self.current_clock, addr, &data[0..4]),
        }
    }

    pub(crate) fn read_data_sized<Bus, BusError>(
        &mut self,
        bus: &mut Bus,
        is_supervisor: bool,
        addr: M68kAddress,
        size: Size,
    ) -> Result<u32, M68kError<BusError>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    {
        self.start_request(is_supervisor, addr, size, MemAccess::Read, MemType::Data, false)?;
        self.read_sized(bus, addr, size)
    }

    pub(crate) fn write_data_sized<Bus, BusError>(
        &mut self,
        bus: &mut Bus,
        is_supervisor: bool,
        addr: M68kAddress,
        size: Size,
        value: u32,
    ) -> Result<(), M68kError<BusError>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    {
        self.start_request(is_supervisor, addr, size, MemAccess::Write, MemType::Data, false)?;
        self.write_sized(bus, addr, size, value)
    }

    pub(crate) fn read_instruction_word<Bus, BusError>(
        &mut self,
        bus: &mut Bus,
        is_supervisor: bool,
        addr: u32,
    ) -> Result<u16, M68kError<BusError>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    {
        self.request.instruction(is_supervisor, addr)?;
        Ok(self.read_sized(bus, addr, Size::Word)? as u16)
    }

    pub(crate) fn read_instruction_long<Bus, BusError>(
        &mut self,
        bus: &mut Bus,
        is_supervisor: bool,
        addr: u32,
    ) -> Result<u32, M68kError<BusError>>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    {
        self.request.instruction(is_supervisor, addr)?;
        self.read_sized(bus, addr, Size::Long)
    }

    pub(crate) fn start_request<BusError>(
        &mut self,
        is_supervisor: bool,
        addr: u32,
        size: Size,
        access: MemAccess,
        mtype: MemType,
        i_n_bit: bool,
    ) -> Result<u32, M68kError<BusError>> {
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
}

fn validate_address<BusError>(addr: u32) -> Result<u32, M68kError<BusError>> {
    if addr & 0x1 == 0 {
        Ok(addr)
    } else {
        Err(M68kError::Exception(Exceptions::AddressError))
    }
}

pub fn dump_memory<Bus, Address, Instant>(bus: &mut Bus, clock: Instant, addr: Address, count: Address)
where
    Bus: BusAccess<Address, Instant = Instant>,
    Address: From<u32> + Into<u32> + Copy,
    Instant: Copy,
{
    let mut addr = addr.into();
    let mut count = count.into();
    while count > 0 {
        let mut line = format!("{:#010x}: ", addr);

        let to = if count < 16 { count / 2 } else { 8 };
        for _ in 0..to {
            let word = bus.read_beu16(clock, Address::from(addr));
            if word.is_err() {
                println!("{}", line);
                return;
            }
            write!(line, "{:#06x} ", word.unwrap()).unwrap();
            addr += 2;
            count -= 2;
        }
        println!("{}", line);
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

    pub(crate) fn get(&mut self, cpu: &M68k) -> Result<u32, M68kError> {

    }

    pub(crate) fn set(&mut self, cpu: &M68k, value: u32) -> Result<(), M68kError> {

    }

    pub(crate) fn complete(&self) -> Result<Self, M68kError> {

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
    pub(crate) fn get(&mut self, cpu: &M68k) -> Result<u32, M68kError> {

    }

    pub(crate) fn complete(&self) -> Result<Self, M68kError> {

    }
}

pub(crate) struct ReadUpdateAccess {
    size: Size,
    target: Target,
}

impl ReadUpdateAccess {
    pub(crate) fn get(&mut self, cpu: &M68k) -> Result<u32, M68kError> {

    }

    pub(crate) fn set(&mut self, cpu: &M68k, value: u32) -> Result<(), M68kError> {

    }

    pub(crate) fn complete(&self) -> Result<Self, M68kError> {

    }
}

pub(crate) struct WriteOnceAccess {
    size: Size,
    target: Target,
}

impl WriteOnceAccess {
    pub(crate) fn set(&mut self, cpu: &M68k, value: u32) -> Result<(), M68kError> {

    }

    pub(crate) fn complete(&self) -> Result<Self, M68kError> {

    }
}
*/
