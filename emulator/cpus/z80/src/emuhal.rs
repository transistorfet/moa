
use emulator_hal::{BusAccess, Instant as EmuInstant, Error as EmuError, Step, Inspect, Debug, IntoAddress};
use crate::state::{Z80, Z80Error, Z80Address, Status};

impl EmuError for Z80Error {}

impl<Instant, Bus> Step<Bus> for Z80<Instant>
where
    Instant: EmuInstant,
    Bus: BusAccess<Z80Address, Instant = Instant>,
{
    type Instant = Instant;
    type Error = Z80Error;

    fn is_running(&mut self) -> bool {
        self.state.status == Status::Running
    }

    fn reset(&mut self, _now: Self::Instant, _bus: &mut Bus) -> Result<(), Self::Error> {
        self.clear_state();
        Ok(())
    }

    fn step(&mut self, now: Self::Instant, bus: &mut Bus) -> Result<Self::Instant, Self::Error> {
        let mut executor = self.begin(now, bus)?;
        executor.step_one()?;
        self.previous_cycle = executor.end();
        // TODO fix this
        Ok(now)
    }
}

/*
impl<Instant, MemBus, IoBus> Step<(&mut MemBus, &mut IoBus)> for Z80<Instant>
where
    Instant: EmuInstant,
    MemBus: BusAccess<Z80Address, Instant = Instant>,
    IoBus: BusAccess<Z80Address, Instant = Instant>,
{
    type Instant = Instant;
    type Error = Z80Error;

    fn is_running(&mut self) -> bool {
        self.state.status == Status::Running
    }

    fn reset(&mut self, _now: Self::Instant, _bus: (&mut MemBus, &mut IoBus)) -> Result<(), Self::Error> {
        self.clear_state();
        Ok(())
    }

    fn step(&mut self, now: Self::Instant, bus: (&mut MemBus, &mut IoBus)) -> Result<Self::Instant, Self::Error> {
        let executor = self.begin(now, bus)?;
        executor.step_one()?;
        self.previous_cycle = executor.end();
        // TODO fix this
        Ok(now)
    }
}
*/

