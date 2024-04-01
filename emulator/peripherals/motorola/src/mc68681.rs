use core::marker::PhantomData;
use core::convert::Infallible;
use core::ops::{Deref, DerefMut};
use femtos::{Instant, Duration, Frequency};
use emulator_hal::{BusAccess, BusAdapter, Step, Instant as EmuInstant, Error as EmuError};

use moa_core::{System, Bus, Address, Steppable, Addressable, Transmutable};
use moa_host::Tty;

use moa_system::{DeviceInterface, Error, MoaBus, MoaStep};

type DeviceAddress = u64;

const REG_MR1A_MR2A: DeviceAddress = 0x01;
const REG_SRA_RD: DeviceAddress = 0x03;
const REG_CSRA_WR: DeviceAddress = 0x03;
const REG_CRA_WR: DeviceAddress = 0x05;
const REG_TBA_WR: DeviceAddress = 0x07;
const REG_RBA_RD: DeviceAddress = 0x07;

const REG_MR1B_MR2B: DeviceAddress = 0x11;
const REG_SRB_RD: DeviceAddress = 0x13;
const REG_CSRB_WR: DeviceAddress = 0x13;
const REG_CRB_WR: DeviceAddress = 0x15;
const REG_TBB_WR: DeviceAddress = 0x17;
const REG_RBB_RD: DeviceAddress = 0x17;

const REG_ACR_WR: DeviceAddress = 0x09;

const REG_CTUR_WR: DeviceAddress = 0x0D;
const REG_CTLR_WR: DeviceAddress = 0x0F;
const REG_START_RD: DeviceAddress = 0x1D;
const REG_STOP_RD: DeviceAddress = 0x1F;

const REG_IPCR_RD: DeviceAddress = 0x09;
const REG_OPCR_WR: DeviceAddress = 0x1B;
const REG_INPUT_RD: DeviceAddress = 0x1B;
const REG_OUT_SET: DeviceAddress = 0x1D;
const REG_OUT_RESET: DeviceAddress = 0x1F;

const REG_ISR_RD: DeviceAddress = 0x0B;
const REG_IMR_WR: DeviceAddress = 0x0B;
const REG_IVR_WR: DeviceAddress = 0x19;


// Status Register Bits (SRA/SRB)
#[allow(dead_code)]
const SR_RECEIVED_BREAK: u8 = 0x80;
#[allow(dead_code)]
const SR_FRAMING_ERROR: u8 = 0x40;
#[allow(dead_code)]
const SR_PARITY_ERROR: u8 = 0x20;
#[allow(dead_code)]
const SR_OVERRUN_ERROR: u8 = 0x10;
#[allow(dead_code)]
const SR_TX_EMPTY: u8 = 0x08;
#[allow(dead_code)]
const SR_TX_READY: u8 = 0x04;
#[allow(dead_code)]
const SR_RX_FULL: u8 = 0x02;
#[allow(dead_code)]
const SR_RX_READY: u8 = 0x01;


// Interrupt Status/Mask Bits (ISR/IVR)
//const ISR_INPUT_CHANGE: u8 = 0x80;
//const ISR_CH_B_BREAK_CHANGE: u8 = 0x40;
const ISR_CH_B_RX_READY_FULL: u8 = 0x20;
const ISR_CH_B_TX_READY: u8 = 0x10;
const ISR_TIMER_CHANGE: u8 = 0x08;
//const ISR_CH_A_BREAK_CHANGE: u8 = 0x04;
const ISR_CH_A_RX_READY_FULL: u8 = 0x02;
const ISR_CH_A_TX_READY: u8 = 0x01;


const DEV_NAME: &str = "mc68681";

#[derive(Default)]
pub struct MC68681Port {
    tty: Option<Box<dyn Tty>>,
    status: u8,

    tx_enabled: bool,

    rx_enabled: bool,
    input: u8,
}

impl MC68681Port {
    pub fn connect(&mut self, pty: Box<dyn Tty>) -> String {
        let name = pty.device_name();
        println!("{}: opening pts {}", DEV_NAME, name);
        self.tty = Some(pty);
        name
    }

    pub fn send_byte(&mut self, data: u8) {
        self.tty.as_mut().map(|tty| tty.write(data));
        self.set_tx_status(false);
    }

    pub fn set_tx_status(&mut self, value: bool) {
        match value {
            true => {
                self.status |= SR_TX_READY | SR_TX_EMPTY;
            },
            false => {
                self.status &= !(SR_TX_READY | SR_TX_EMPTY);
            },
        }
    }

    pub fn set_rx_status(&mut self, value: bool) {
        match value {
            true => {
                self.status |= SR_RX_READY;
            },
            false => {
                self.status &= !SR_RX_READY;
            },
        }
    }

    pub fn check_rx(&mut self) -> bool {
        if self.rx_enabled && (self.status & SR_RX_READY) == 0 && self.tty.is_some() {
            let tty = self.tty.as_mut().unwrap();
            let result = tty.read();
            if let Some(input) = result {
                self.input = input;
                self.set_rx_status(true);
                return true;
            }
        }
        false
    }

    pub fn check_tx(&mut self) -> bool {
        self.set_tx_status(self.tx_enabled);
        self.tx_enabled
    }

    pub fn handle_command(&mut self, data: u8) -> Option<bool> {
        let rx_cmd = data & 0x03;
        if rx_cmd == 0b01 {
            self.rx_enabled = true;
        } else if rx_cmd == 0b10 {
            self.rx_enabled = false;
        }

        let tx_cmd = (data & 0x0C) >> 2;
        if tx_cmd == 0b01 {
            self.tx_enabled = true;
            self.set_tx_status(true);
            return Some(true);
        } else if tx_cmd == 0b10 {
            self.tx_enabled = false;
            self.set_tx_status(false);
            return Some(false);
        }

        None
    }
}

pub struct MC68681<Address, Instant, Error> {
    frequency: Frequency,

    acr: u8,
    pub port_a: MC68681Port,
    pub port_b: MC68681Port,

    int_mask: u8,
    int_status: u8,
    int_vector: u8,

    timer_preload: u16,
    timer_count: u16,
    is_timing: bool,
    timer_divider: u16,

    input_pin_change: u8,
    input_state: u8,
    output_conf: u8,
    output_state: u8,

    address: PhantomData<Address>,
    instant: PhantomData<Instant>,
    error: PhantomData<Error>,
}

impl<Address, Instant, Error> Default for MC68681<Address, Instant, Error> {
    fn default() -> Self {
        Self {
            frequency: Frequency::from_hz(3_686_400),

            acr: 0,
            port_a: MC68681Port::default(),
            port_b: MC68681Port::default(),

            int_mask: 0,
            int_status: 0,
            int_vector: 0,

            timer_preload: 0,
            timer_count: 0,
            is_timing: true,
            timer_divider: 0,

            input_pin_change: 0,
            input_state: 0,
            output_conf: 0,
            output_state: 0,

            address: PhantomData,
            instant: PhantomData,
            error: PhantomData,
        }
    }
}

impl<Address, Instant, Error> MC68681<Address, Instant, Error> {
    pub fn address_space(&self) -> usize {
        0x30
    }

    pub fn set_interrupt_flag(&mut self, flag: u8, value: bool) {
        self.int_status = (self.int_status & !flag) | (if value { flag } else { 0 });
    }

    pub fn get_interrupt_flag(&mut self) -> (bool, u8, u8) {
        ((self.int_status & self.int_mask) != 0, 4, self.int_vector)
    }
}

impl<Address, Instant, Error, Bus> Step<Bus> for MC68681<Address, Instant, Error>
where
    Address: Into<DeviceAddress> + Copy,
    Instant: EmuInstant,
    Bus: BusAccess<Address, Instant = Instant> + ?Sized,
{
    type Instant = Instant;
    type Error = Error;

    fn is_running(&mut self) -> bool {
        true
    }

    fn reset(&mut self, _now: Self::Instant, _bus: &mut Bus) -> Result<(), Self::Error> {
        Ok(())
    }

    fn step(&mut self, now: Self::Instant, bus: &mut Bus) -> Result<Self::Instant, Self::Error> {
        if self.port_a.check_rx() {
            self.set_interrupt_flag(ISR_CH_A_RX_READY_FULL, true);
        }

        if self.port_b.check_rx() {
            self.set_interrupt_flag(ISR_CH_B_RX_READY_FULL, true);
        }

        if self.is_timing {
            self.timer_divider = self.timer_divider.wrapping_sub(1);
            if self.timer_divider == 0 {
                self.timer_divider = 1;
                self.timer_count = self.timer_count.wrapping_sub(1);

                if self.timer_count == 0 {
                    self.set_interrupt_flag(ISR_TIMER_CHANGE, true);
                    if (self.acr & 0x40) == 0 {
                        self.is_timing = false;
                    } else {
                        self.timer_count = self.timer_preload;
                    }
                }
            }
        }

        // TODO this has been added to the Steppable impl, but isn't handled by Step
        //self.check_interrupt_state(system)?;

        if self.port_a.check_tx() {
            self.set_interrupt_flag(ISR_CH_A_TX_READY, true);
        }

        if self.port_b.check_tx() {
            self.set_interrupt_flag(ISR_CH_B_TX_READY, true);
        }

        Ok(now + Instant::hertz_to_duration(self.frequency.as_hz() as u64))
    }
}

impl<Address, Instant, Error> BusAccess<Address> for MC68681<Address, Instant, Error>
where
    Address: Into<DeviceAddress> + Copy,
    Instant: EmuInstant,
    Error: EmuError,
{
    type Instant = Instant;
    type Error = Error;

    #[inline]
    fn read(&mut self, _clock: Self::Instant, addr: Address, data: &mut [u8]) -> Result<usize, Self::Error> {
        let addr = addr.into();

        match addr {
            REG_SRA_RD => data[0] = self.port_a.status,
            REG_RBA_RD => {
                data[0] = self.port_a.input;
                self.port_a.set_rx_status(false);
                self.set_interrupt_flag(ISR_CH_A_RX_READY_FULL, false);
            },
            REG_SRB_RD => data[0] = self.port_b.status,
            REG_RBB_RD => {
                data[0] = self.port_b.input;
                self.port_b.set_rx_status(false);
                self.set_interrupt_flag(ISR_CH_B_RX_READY_FULL, false);
            },
            REG_ISR_RD => {
                data[0] = self.int_status;
            },
            REG_IPCR_RD => {
                data[0] = self.input_pin_change;
            },
            REG_INPUT_RD => {
                data[0] = self.input_state;
            },
            REG_START_RD => {
                self.timer_count = self.timer_preload;
                self.is_timing = true;
            },
            REG_STOP_RD => {
                if (self.acr & 0x40) == 0 {
                    // Counter Mode
                    self.is_timing = false;
                    self.timer_count = self.timer_preload;
                } else {
                    // Timer Mode
                    // Do nothing except reset the ISR bit
                }
                self.set_interrupt_flag(ISR_TIMER_CHANGE, false);
            },
            _ => {},
        }

        if addr != REG_SRA_RD && addr != REG_SRB_RD {
            log::debug!("{}: read from {:0x} of {:0x}", DEV_NAME, addr, data[0]);
        }

        Ok(1)
    }

    #[inline]
    fn write(&mut self, _clock: Self::Instant, addr: Address, data: &[u8]) -> Result<usize, Self::Error> {
        let addr = addr.into();

        log::debug!("{}: writing {:0x} to {:0x}", DEV_NAME, data[0], addr);
        match addr {
            REG_MR1A_MR2A | REG_MR1B_MR2B | REG_CSRA_WR | REG_CSRB_WR => {
                // NOTE we aren't simulating the serial speeds, so we aren't doing anything with these settings atm
            },
            REG_ACR_WR => {
                self.acr = data[0];
            },
            REG_TBA_WR => {
                log::debug!("{}a: write {}", DEV_NAME, data[0] as char);
                self.port_a.send_byte(data[0]);
                self.set_interrupt_flag(ISR_CH_A_TX_READY, false);
            },
            REG_CRA_WR => {
                if let Some(value) = self.port_a.handle_command(data[0]) {
                    self.set_interrupt_flag(ISR_CH_A_TX_READY, value);
                }
            },
            REG_TBB_WR => {
                log::debug!("{}b: write {:x}", DEV_NAME, data[0]);
                self.port_b.send_byte(data[0]);
                self.set_interrupt_flag(ISR_CH_B_TX_READY, false);
            },
            REG_CRB_WR => {
                if let Some(value) = self.port_b.handle_command(data[0]) {
                    self.set_interrupt_flag(ISR_CH_B_TX_READY, value);
                }
            },
            REG_CTUR_WR => {
                self.timer_preload = (self.timer_preload & 0x00FF) | ((data[0] as u16) << 8);
            },
            REG_CTLR_WR => {
                self.timer_preload = (self.timer_preload & 0xFF00) | (data[0] as u16);
            },
            REG_IMR_WR => {
                self.int_mask = data[0];
            },
            REG_IVR_WR => {
                self.int_vector = data[0];
            },
            REG_OPCR_WR => {
                self.output_conf = data[0];
            },
            REG_OUT_SET => {
                self.output_state |= data[0];
            },
            REG_OUT_RESET => {
                self.output_state &= !data[0];
            },
            _ => {},
        }
        Ok(1)
    }
}

pub struct MoaMC68681(MC68681<u64, Instant, Error>);

impl Default for MoaMC68681 {
    fn default() -> Self {
        MoaMC68681(MC68681::default())
    }
}

impl DeviceInterface for MC68681<u64, Instant, Error> {
    fn as_bus_access(&mut self) -> Option<&mut MoaBus> {
        Some(self)
    }

    fn as_step(&mut self) -> Option<&mut MoaStep> {
        Some(self)
    }
}

impl Deref for MoaMC68681 {
    type Target = MC68681<u64, Instant, Error>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for MoaMC68681 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

//// OLD INTERFACE
/*
impl Steppable for MC68681 {
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let duration = <Self as Step<u64, Bus>>::step(self, system.clock, &mut *system.bus.borrow_mut())
            .map(|next| next.duration_since(system.clock));

        let flags = self.get_interrupt_flag();
        system
            .get_interrupt_controller()
            .set(flags.0, flags.1, flags.2)?;
        duration
    }
}

impl Addressable for MC68681 {
    fn size(&self) -> usize {
        self.address_space()
    }

    fn read(&mut self, clock: Instant, addr: Address, data: &mut [u8]) -> Result<(), Error> {
        <Self as BusAccess<u8>>::read(self, clock, addr as u8, data)
            .map_err(|err| Error::new(format!("{:?}", err)))?;
        Ok(())
    }

    fn write(&mut self, clock: Instant, addr: Address, data: &[u8]) -> Result<(), Error> {
        <Self as BusAccess<u8>>::write(self, clock, addr as u8, data)
            .map_err(|err| Error::new(format!("{:?}", err)))?;
        Ok(())
    }
}

impl Transmutable for MC68681 {
    fn as_addressable(&mut self) -> Option<&mut dyn Addressable> {
        Some(self)
    }

    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }
}
*/
