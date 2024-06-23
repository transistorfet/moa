use core::marker::PhantomData;
use emulator_hal::{BusAccess, Instant as EmuInstant, ErrorType, Step, Inspect, Debug, IntoAddress, FromAddress, NoBus};
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

pub struct Z80Port<'a, MemBus, IOBus, Instant>
where
    MemBus: BusAccess<Z80Address, Instant = Instant>,
    IOBus: BusAccess<Z80IOAddress, Instant = Instant>,
{
    mem_bus: &'a mut MemBus,
    io_bus: &'a mut IOBus,
    instant: PhantomData<Instant>,
}

impl<'a, MemBus, IOBus, Instant> Z80Port<'a, MemBus, IOBus, Instant>
where
    MemBus: BusAccess<Z80Address, Instant = Instant>,
    IOBus: BusAccess<Z80IOAddress, Instant = Instant>,
{
    pub fn new(mem_bus: &'a mut MemBus, io_bus: &'a mut IOBus) -> Self {
        Self {
            mem_bus,
            io_bus,
            instant: PhantomData,
        }
    }
}


impl<'a, MemBus, IOBus, Instant> BusAccess<Z80AddressSpace> for Z80Port<'a, MemBus, IOBus, Instant>
where
    Instant: EmuInstant,
    MemBus: BusAccess<Z80Address, Instant = Instant>,
    IOBus: BusAccess<Z80IOAddress, Instant = Instant>,
{
    type Instant = Instant;
    type Error = Z80BusError<MemBus::Error, IOBus::Error>;

    #[inline]
    fn read(
        &mut self,
        now: Self::Instant,
        addr: Z80AddressSpace,
        data: &mut [u8],
    ) -> Result<usize, Self::Error> {
        match addr {
            Z80AddressSpace::Memory(addr) => self.mem_bus.read(now, addr, data).map_err(|err| Z80BusError::Memory(err)),
            Z80AddressSpace::IO(addr) => self.io_bus.read(now, addr, data).map_err(|err| Z80BusError::IO(err)),
        }
    }

    #[inline]
    fn write(
        &mut self,
        now: Self::Instant,
        addr: Z80AddressSpace,
        data: &[u8],
    ) -> Result<usize, Self::Error> {
        match addr {
            Z80AddressSpace::Memory(addr) => self.mem_bus.write(now, addr, data).map_err(|err| Z80BusError::Memory(err)),
            Z80AddressSpace::IO(addr) => self.io_bus.write(now, addr, data).map_err(|err| Z80BusError::IO(err)),
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

/*
impl<Instant, MemBus, IOBus> Step<(&mut MemBus, &mut IOBus)> for Z80<Instant>
where
    Instant: EmuInstant,
    MemBus: BusAccess<Z80Address, Instant = Instant>,
    IOBus: BusAccess<Z80IOAddress, Instant = Instant>,
{
    type Instant = Instant;
    type Error = Z80Error;

    fn is_running(&mut self) -> bool {
        self.state.status == Status::Running
    }

    fn reset(&mut self, _now: Self::Instant, _bus: (&mut MemBus, &mut IOBus)) -> Result<(), Self::Error> {
        self.clear_state();
        Ok(())
    }

    fn step(&mut self, now: Self::Instant, bus: (&mut MemBus, &mut IOBus)) -> Result<Self::Instant, Self::Error> {
        let executor = self.begin(now, bus.0, bus.1)?;
        let clocks = executor.step_one()?;
        self.previous_cycle = executor.end();
        Ok(now + Instant::hertz_to_duration(self.frequency.as_hz() as u64) * clocks as u32)
    }
}
*/
