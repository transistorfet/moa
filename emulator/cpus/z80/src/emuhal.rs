use core::fmt;
use core::marker::PhantomData;
use emulator_hal::{BusAccess, Instant as EmuInstant, ErrorType, Step, Inspect, Debug};
use crate::state::{Z80, Z80Error, Z80Address, Z80IOAddress, Z80AddressSpace, Status};

#[derive(Clone, Debug)]
pub enum Z80BusError<MemError, IOError>
where
    MemError: ErrorType,
    IOError: ErrorType,
{
    Memory(MemError),
    IO(IOError),
}

impl<MemError, IOError> ErrorType for Z80BusError<MemError, IOError>
where
    MemError: ErrorType,
    IOError: ErrorType,
{
}

pub struct Z80Port<MemBus, IOBus, Instant>
where
    MemBus: BusAccess<Z80Address, Instant = Instant>,
    IOBus: BusAccess<Z80IOAddress, Instant = Instant>,
{
    mem_bus: MemBus,
    io_bus: IOBus,
    instant: PhantomData<Instant>,
}

impl<MemBus, IOBus, Instant> Z80Port<MemBus, IOBus, Instant>
where
    MemBus: BusAccess<Z80Address, Instant = Instant>,
    IOBus: BusAccess<Z80IOAddress, Instant = Instant>,
{
    pub fn new(mem_bus: MemBus, io_bus: IOBus) -> Self {
        Self {
            mem_bus,
            io_bus,
            instant: PhantomData,
        }
    }
}

impl<MemBus, IOBus, Instant> BusAccess<Z80AddressSpace> for Z80Port<MemBus, IOBus, Instant>
where
    Instant: EmuInstant,
    MemBus: BusAccess<Z80Address, Instant = Instant>,
    IOBus: BusAccess<Z80IOAddress, Instant = Instant>,
{
    type Instant = Instant;
    type Error = Z80BusError<MemBus::Error, IOBus::Error>;

    #[inline]
    fn read(&mut self, now: Self::Instant, addr: Z80AddressSpace, data: &mut [u8]) -> Result<usize, Self::Error> {
        match addr {
            Z80AddressSpace::Memory(addr) => self.mem_bus.read(now, addr, data).map_err(Z80BusError::Memory),
            Z80AddressSpace::IO(addr) => self.io_bus.read(now, addr, data).map_err(Z80BusError::IO),
        }
    }

    #[inline]
    fn write(&mut self, now: Self::Instant, addr: Z80AddressSpace, data: &[u8]) -> Result<usize, Self::Error> {
        match addr {
            Z80AddressSpace::Memory(addr) => self.mem_bus.write(now, addr, data).map_err(Z80BusError::Memory),
            Z80AddressSpace::IO(addr) => self.io_bus.write(now, addr, data).map_err(Z80BusError::IO),
        }
    }
}

impl ErrorType for Z80Error {}

impl<Instant, Bus> Step<Z80AddressSpace, Bus> for Z80<Instant>
where
    Instant: EmuInstant,
    Bus: BusAccess<Z80AddressSpace, Instant = Instant>,
{
    type Error = Z80Error;

    fn is_running(&mut self) -> bool {
        self.state.status == Status::Running
    }

    fn reset(&mut self, _now: Bus::Instant, _bus: &mut Bus) -> Result<(), Self::Error> {
        self.clear_state();
        Ok(())
    }

    fn step(&mut self, now: Bus::Instant, bus: &mut Bus) -> Result<Bus::Instant, Self::Error> {
        let mut executor = self.begin(now, bus)?;
        let clocks = executor.step_one()?;
        self.previous_cycle = executor.end();
        Ok(now + Instant::hertz_to_duration(self.frequency.as_hz() as u64) * clocks as u32)
    }
}


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Z80Info {
    State,
}

impl<Bus, BusError, Instant, Writer> Inspect<Z80AddressSpace, Bus, Writer> for Z80<Instant>
where
    Bus: BusAccess<Z80AddressSpace, Instant = Instant, Error = BusError>,
    BusError: ErrorType,
    Instant: EmuInstant,
    Writer: fmt::Write,
{
    type InfoType = Z80Info;

    type Error = Z80Error;

    fn inspect(&mut self, info: Self::InfoType, bus: &mut Bus, writer: &mut Writer) -> Result<(), Self::Error> {
        match info {
            Z80Info::State => self
                .dump_state(writer, Instant::START, bus)
                .map_err(|_| Z80Error::Other("error while formatting state".to_string())),
        }
    }

    fn brief_summary(&mut self, bus: &mut Bus, writer: &mut Writer) -> Result<(), Self::Error> {
        self.inspect(Z80Info::State, bus, writer)
    }

    fn detailed_summary(&mut self, bus: &mut Bus, writer: &mut Writer) -> Result<(), Self::Error> {
        self.inspect(Z80Info::State, bus, writer)
    }
}

/// Control the execution of a CPU device for debugging purposes
impl<Bus, BusError, Instant, Writer> Debug<Z80AddressSpace, Bus, Writer> for Z80<Instant>
where
    Bus: BusAccess<Z80AddressSpace, Instant = Instant, Error = BusError>,
    BusError: ErrorType,
    Instant: EmuInstant,
    Writer: fmt::Write,
{
    // TODO this should be a new type
    type DebugError = Z80Error;

    fn get_execution_address(&mut self) -> Result<Z80AddressSpace, Self::DebugError> {
        Ok(Z80AddressSpace::Memory(self.state.pc))
    }

    fn set_execution_address(&mut self, address: Z80AddressSpace) -> Result<(), Self::DebugError> {
        if let Z80AddressSpace::Memory(address) = address {
            self.state.pc = address;
            Ok(())
        } else {
            Err(Z80Error::Other("PC can only be set to a memory address, given an IO address".to_string()))
        }
    }

    fn add_breakpoint(&mut self, address: Z80AddressSpace) {
        if let Z80AddressSpace::Memory(address) = address {
            self.debugger.breakpoints.push(address);
        }
    }

    fn remove_breakpoint(&mut self, address: Z80AddressSpace) {
        if let Z80AddressSpace::Memory(address) = address {
            if let Some(index) = self.debugger.breakpoints.iter().position(|a| *a == address) {
                self.debugger.breakpoints.remove(index);
            }
        }
    }

    fn clear_breakpoints(&mut self) {
        self.debugger.breakpoints.clear();
    }
}
