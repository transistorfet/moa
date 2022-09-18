
use crate::system::System;
use crate::error::{ErrorType, Error};
use crate::devices::{ClockElapsed, Address, Steppable, Interruptable, Addressable, Debuggable, Transmutable};

use super::state::{M68k, M68kType, Status, Flags, Exceptions, InterruptPriority, FunctionCode, MemType, MemAccess};
use super::instructions::{
    Register,
    Size,
    Sign,
    Direction,
    ShiftDirection,
    XRegister,
    BaseRegister,
    IndexRegister,
    RegOrImmediate,
    ControlRegister,
    Condition,
    Target,
    Instruction,
    sign_extend_to_long,
};


const DEV_NAME: &'static str = "m68k-cpu";

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Used {
    Once,
    Twice,
}

impl Steppable for M68k {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        self.step_internal(system)
    }

    fn on_error(&mut self, _system: &System) {
        self.dump_state();
    }
}

impl Interruptable for M68k { }

impl Transmutable for M68k {
    fn as_steppable(&mut self) -> Option<&mut dyn Steppable> {
        Some(self)
    }

    fn as_interruptable(&mut self) -> Option<&mut dyn Interruptable> {
        Some(self)
    }

    fn as_debuggable(&mut self) -> Option<&mut dyn Debuggable> {
        Some(self)
    }
}


impl M68k {
    #[allow(dead_code)]
    pub fn is_running(&self) -> bool {
        self.state.status != Status::Stopped
    }

    pub fn step_internal(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        match self.state.status {
            Status::Init => self.init(),
            Status::Stopped => Err(Error::new("CPU stopped")),
            Status::Running => {
                match self.cycle_one(system) {
                    Ok(diff) => Ok(diff),
                    Err(Error { err: ErrorType::Processor, native, .. }) => {
                    // TODO match arm conditional is temporary: illegal instructions generate a top level error in order to debug and fix issues with decode
                    //Err(Error { err: ErrorType::Processor, native, .. }) if native != Exceptions::IllegalInstruction as u32 => {
                        self.exception(native as u8, false)?;
                        Ok(4)
                    },
                    Err(err) => Err(err),
                }
            },
        }
    }

    pub fn init(&mut self) -> Result<ClockElapsed, Error> {
        self.state.ssp = self.port.read_beu32(0)?;
        self.state.pc = self.port.read_beu32(4)?;
        self.state.status = Status::Running;
        Ok(16)
    }

    pub fn cycle_one(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        self.timer.cycle.start();
        self.decode_next()?;
        self.execute_current()?;
        self.timer.cycle.end();
        //if (self.timer.cycle.events % 500) == 0 {
        //    println!("{}", self.timer);
        //}

        self.check_pending_interrupts(system)?;
        self.check_breakpoints(system);
        Ok((1_000_000_000 / self.frequency as u64) * self.timing.calculate_clocks(false, 1) as ClockElapsed)
    }

    pub fn check_pending_interrupts(&mut self, system: &System) -> Result<(), Error> {
        self.state.pending_ipl = match system.get_interrupt_controller().check() {
            (true, priority) => InterruptPriority::from_u8(priority),
            (false, _) => InterruptPriority::NoInterrupt,
        };

        let current_ipl = self.state.current_ipl as u8;
        let pending_ipl = self.state.pending_ipl as u8;

        if self.state.pending_ipl != InterruptPriority::NoInterrupt {
            let priority_mask = ((self.state.sr & Flags::IntMask as u16) >> 8) as u8;

            if (pending_ipl > priority_mask || pending_ipl == 7) && pending_ipl >= current_ipl {
                debug!("{} interrupt: {} @ {} ns", DEV_NAME, pending_ipl, system.clock);
                self.state.current_ipl = self.state.pending_ipl;
                let ack_num = system.get_interrupt_controller().acknowledge(self.state.current_ipl as u8)?;
                self.exception(ack_num, true)?;
                return Ok(());
            }
        }

        if pending_ipl < current_ipl {
            self.state.current_ipl = self.state.pending_ipl;
        }

        Ok(())
    }

    pub fn exception(&mut self, number: u8, is_interrupt: bool) -> Result<(), Error> {
        debug!("{}: raising exception {}", DEV_NAME, number);

        if number == Exceptions::BusError as u8 || number == Exceptions::AddressError as u8 {
            let result = self.setup_group0_exception(number);
            if let Err(err) = result {
                self.state.status = Status::Stopped;
                return Err(err);
            }
        } else {
            self.setup_normal_exception(number, is_interrupt)?;
        }

        Ok(())
    }

    pub fn setup_group0_exception(&mut self, number: u8) -> Result<(), Error> {
        let sr = self.state.sr;
        let ins_word = self.decoder.instruction_word;
        let extra_code = self.state.request.get_type_code();
        let fault_size = self.state.request.size.in_bytes();
        let fault_address = self.state.request.address;

        // Changes to the flags must happen after the previous value has been pushed to the stack
        self.set_flag(Flags::Supervisor, true);
        self.set_flag(Flags::Tracing, false);

        let offset = (number as u16) << 2;
        if self.cputype >= M68kType::MC68010 {
            self.push_word(offset)?;
        }

        self.push_long(self.state.pc - fault_size)?;
        self.push_word(sr)?;
        self.push_word(ins_word)?;
        self.push_long(fault_address)?;
        self.push_word((ins_word & 0xFFF0) | extra_code)?;

        let vector = self.state.vbr + offset as u32;
        let addr = self.port.read_beu32(vector as Address)?;
        self.set_pc(addr)?;

        Ok(())
    }

    pub fn setup_normal_exception(&mut self, number: u8, is_interrupt: bool) -> Result<(), Error> {
        self.state.request.i_n_bit = true;

        // Changes to the flags must happen after the previous value has been pushed to the stack
        self.set_flag(Flags::Supervisor, true);
        self.set_flag(Flags::Tracing, false);
        if is_interrupt {
            self.state.sr = (self.state.sr & !(Flags::IntMask as u16)) | ((self.state.current_ipl as u16) << 8);
        }

        let sr = self.state.sr;
        let offset = (number as u16) << 2;
        if self.cputype >= M68kType::MC68010 {
            self.push_word(offset)?;
        }
        self.push_long(self.state.pc)?;
        self.push_word(sr)?;

        let vector = self.state.vbr + offset as u32;
        let addr = self.port.read_beu32(vector as Address)?;
        self.set_pc(addr)?;

        Ok(())
    }

    pub fn decode_next(&mut self) -> Result<(), Error> {
        self.timing.reset();

        self.timer.decode.start();
        self.start_instruction_request(self.state.pc)?;
        self.decoder.decode_at(&mut self.port, self.state.pc)?;
        self.timer.decode.end();

        self.timing.add_instruction(&self.decoder.instruction);

        if self.debugger.use_tracing {
            self.decoder.dump_decoded(&mut self.port);
        }

        self.state.pc = self.decoder.end;

        Ok(())
    }

    pub fn execute_current(&mut self) -> Result<(), Error> {
        self.timer.execute.start();
        match self.decoder.instruction {
            Instruction::ABCD(src, dest) => {
                let value = convert_from_bcd(self.get_target_value(src, Size::Byte, Used::Once)? as u8);
                let existing = convert_from_bcd(self.get_target_value(dest, Size::Byte, Used::Twice)? as u8);
                let result = existing.wrapping_add(value).wrapping_add(self.get_flag(Flags::Extend) as u8);
                let carry = result > 99;
                self.set_target_value(dest, convert_to_bcd(result) as u32, Size::Byte, Used::Twice)?;
                self.set_flag(Flags::Zero, result == 0);
                self.set_flag(Flags::Carry, carry);
                self.set_flag(Flags::Extend, carry);
            },
            Instruction::ADD(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let existing = self.get_target_value(dest, size, Used::Twice)?;
                let (result, carry) = overflowing_add_sized(existing, value, size);
                let overflow = get_add_overflow(existing, value, result, size);
                self.set_compare_flags(result, size, carry, overflow);
                self.set_flag(Flags::Extend, carry);
                self.set_target_value(dest, result, size, Used::Twice)?;
            },
            Instruction::ADDA(src, dest, size) => {
                let value = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
                let existing = *self.get_a_reg_mut(dest);
                let (result, _) = overflowing_add_sized(existing, value, Size::Long);
                *self.get_a_reg_mut(dest) = result;
            },
            Instruction::ADDX(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let existing = self.get_target_value(dest, size, Used::Twice)?;
                let extend = self.get_flag(Flags::Extend) as u32;
                let (result1, carry1) = overflowing_add_sized(existing, value, size);
                let (result2, carry2) = overflowing_add_sized(result1, extend, size);
                let overflow = get_add_overflow(existing, value, result2, size);

                // Handle flags
                let zero = self.get_flag(Flags::Zero);
                self.set_compare_flags(result2, size, carry1 || carry2, overflow);
                if self.get_flag(Flags::Zero) {
                    // ADDX can only clear the zero flag, so if it's set, restore it to whatever it was before
                    self.set_flag(Flags::Zero, zero);
                }
                self.set_flag(Flags::Extend, carry1 || carry2);

                self.set_target_value(dest, result2, size, Used::Twice)?;
            },
            Instruction::AND(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let existing = self.get_target_value(dest, size, Used::Twice)?;
                let result = get_value_sized(existing & value, size);
                self.set_target_value(dest, result, size, Used::Twice)?;
                self.set_logic_flags(result, size);
            },
            Instruction::ANDtoCCR(value) => {
                self.state.sr = (self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) & (value as u16));
            },
            Instruction::ANDtoSR(value) => {
                self.require_supervisor()?;
                self.set_sr(self.state.sr & value);
            },
            Instruction::ASd(count, target, size, shift_dir) => {
                let count = self.get_target_value(count, size, Used::Once)? % 64;

                let mut overflow = false;
                let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
                let mut previous_msb = get_msb(pair.0, size);
                for _ in 0..count {
                    pair = shift_operation(pair.0, size, shift_dir, true);
                    if get_msb(pair.0, size) != previous_msb {
                        overflow = true;
                    }
                    previous_msb = get_msb(pair.0, size);
                }
                self.set_target_value(target, pair.0, size, Used::Twice)?;

                // Adjust flags
                self.set_logic_flags(pair.0, size);
                self.set_flag(Flags::Overflow, overflow);
                if count != 0 {
                    self.set_flag(Flags::Extend, pair.1);
                    self.set_flag(Flags::Carry, pair.1);
                } else {
                    self.set_flag(Flags::Carry, false);
                }
            },
            Instruction::Bcc(cond, offset) => {
                let should_branch = self.get_current_condition(cond);
                if should_branch {
                    self.set_pc((self.decoder.start + 2).wrapping_add(offset as u32))?;
                }
            },
            Instruction::BRA(offset) => {
                self.set_pc((self.decoder.start + 2).wrapping_add(offset as u32))?;
            },
            Instruction::BSR(offset) => {
                self.push_long(self.state.pc)?;
                let sp = *self.get_stack_pointer_mut();
                self.debugger.stack_tracer.push_return(sp);
                self.set_pc((self.decoder.start + 2).wrapping_add(offset as u32))?;
            },
            Instruction::BCHG(bitnum, target, size) => {
                let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
                let mut value = self.get_target_value(target, size, Used::Twice)?;
                let mask = self.set_bit_test_flags(value, bitnum, size);
                value = (value & !mask) | (!(value & mask) & mask);
                self.set_target_value(target, value, size, Used::Twice)?;
            },
            Instruction::BCLR(bitnum, target, size) => {
                let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
                let mut value = self.get_target_value(target, size, Used::Twice)?;
                let mask = self.set_bit_test_flags(value, bitnum, size);
                value = value & !mask;
                self.set_target_value(target, value, size, Used::Twice)?;
            },
            Instruction::BSET(bitnum, target, size) => {
                let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
                let mut value = self.get_target_value(target, size, Used::Twice)?;
                let mask = self.set_bit_test_flags(value, bitnum, size);
                value = value | mask;
                self.set_target_value(target, value, size, Used::Twice)?;
            },
            Instruction::BTST(bitnum, target, size) => {
                let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
                let value = self.get_target_value(target, size, Used::Once)?;
                self.set_bit_test_flags(value, bitnum, size);
            },
            Instruction::BFCHG(target, offset, width) => {
                let (offset, width) = self.get_bit_field_args(offset, width);
                let mask = get_bit_field_mask(offset, width);
                let value = self.get_target_value(target, Size::Long, Used::Twice)?;
                let field = value & mask;
                self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
                self.set_target_value(target, (value & !mask) | (!field & mask), Size::Long, Used::Twice)?;
            },
            Instruction::BFCLR(target, offset, width) => {
                let (offset, width) = self.get_bit_field_args(offset, width);
                let mask = get_bit_field_mask(offset, width);
                let value = self.get_target_value(target, Size::Long, Used::Twice)?;
                let field = value & mask;
                self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
                self.set_target_value(target, value & !mask, Size::Long, Used::Twice)?;
            },
            Instruction::BFEXTS(target, offset, width, reg) => {
                let (offset, width) = self.get_bit_field_args(offset, width);
                let mask = get_bit_field_mask(offset, width);
                let value = self.get_target_value(target, Size::Long, Used::Once)?;
                let field = value & mask;
                self.set_bit_field_test_flags(field, get_bit_field_msb(offset));

                let right_offset = 32 - offset - width;
                let mut ext = 0;
                for _ in 0..(offset + right_offset) {
                    ext = (ext >> 1) | 0x80000000;
                }
                self.state.d_reg[reg as usize] = (field >> right_offset) | ext;
            },
            Instruction::BFEXTU(target, offset, width, reg) => {
                let (offset, width) = self.get_bit_field_args(offset, width);
                let mask = get_bit_field_mask(offset, width);
                let value = self.get_target_value(target, Size::Long, Used::Once)?;
                let field = value & mask;
                self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
                self.state.d_reg[reg as usize] = field >> (32 - offset - width);
            },
            //Instruction::BFFFO(target, offset, width, reg) => {
            //},
            //Instruction::BFINS(reg, target, offset, width) => {
            //},
            Instruction::BFSET(target, offset, width) => {
                let (offset, width) = self.get_bit_field_args(offset, width);
                let mask = get_bit_field_mask(offset, width);
                let value = self.get_target_value(target, Size::Long, Used::Twice)?;
                let field = value & mask;
                self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
                self.set_target_value(target, value | mask, Size::Long, Used::Twice)?;
            },
            Instruction::BFTST(target, offset, width) => {
                let (offset, width) = self.get_bit_field_args(offset, width);
                let mask = get_bit_field_mask(offset, width);
                let value = self.get_target_value(target, Size::Long, Used::Once)?;
                let field = value & mask;
                self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
            },
            //Instruction::BKPT(u8) => {
            //},
            Instruction::CHK(target, reg, size) => {
                let upper_bound = sign_extend_to_long(self.get_target_value(target, size, Used::Once)?, size) as i32;
                let dreg = sign_extend_to_long(self.state.d_reg[reg as usize], size) as i32;

                self.set_sr(self.state.sr & 0xFFF0);
                if dreg < 0 || dreg > upper_bound {
                    if dreg < 0 {
                        self.set_flag(Flags::Negative, true);
                    } else if dreg > upper_bound {
                        self.set_flag(Flags::Negative, false);
                    }
                    self.exception(Exceptions::ChkInstruction as u8, false)?;
                }
            },
            Instruction::CLR(target, size) => {
                if self.cputype == M68kType::MC68000 {
                    self.get_target_value(target, size, Used::Twice)?;
                    self.set_target_value(target, 0, size, Used::Twice)?;
                } else {
                    self.set_target_value(target, 0, size, Used::Once)?;
                }
                // Clear flags except Zero flag
                self.state.sr = (self.state.sr & 0xFFF0) | (Flags::Zero as u16);
            },
            Instruction::CMP(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let existing = self.get_target_value(dest, size, Used::Once)?;
                let (result, carry) = overflowing_sub_sized(existing, value, size);
                let overflow = get_sub_overflow(existing, value, result, size);
                self.set_compare_flags(result, size, carry, overflow);
            },
            Instruction::CMPA(src, reg, size) => {
                let value = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
                let existing = *self.get_a_reg_mut(reg);
                let (result, carry) = overflowing_sub_sized(existing, value, Size::Long);
                let overflow = get_sub_overflow(existing, value, result, Size::Long);
                self.set_compare_flags(result, Size::Long, carry, overflow);
            },
            Instruction::DBcc(cond, reg, offset) => {
                let condition_true = self.get_current_condition(cond);
                if !condition_true {
                    let next = ((get_value_sized(self.state.d_reg[reg as usize], Size::Word) as u16) as i16).wrapping_sub(1);
                    set_value_sized(&mut self.state.d_reg[reg as usize], next as u32, Size::Word);
                    if next != -1 {
                        self.set_pc((self.decoder.start + 2).wrapping_add(offset as u32))?;
                    }
                }
            },
            Instruction::DIVW(src, dest, sign) => {
                let value = self.get_target_value(src, Size::Word, Used::Once)?;
                if value == 0 {
                    self.exception(Exceptions::ZeroDivide as u8, false)?;
                    return Ok(());
                }

                let existing = get_value_sized(self.state.d_reg[dest as usize], Size::Long);
                let (remainder, quotient, overflow) = match sign {
                    Sign::Signed => {
                        let existing = existing as i32;
                        let value = sign_extend_to_long(value, Size::Word) as i32;
                        let quotient = existing / value;
                        (
                            (existing % value) as u32,
                            quotient as u32,
                            quotient > i16::MAX as i32 || quotient < i16::MIN as i32
                        )
                    },
                    Sign::Unsigned => {
                        let quotient = existing / value;
                        (
                            existing % value,
                            quotient,
                            (quotient & 0xFFFF0000) != 0
                        )
                    },
                };

                // Only update the register if the quotient was large than a 16-bit number
                if !overflow {
                    self.set_compare_flags(quotient as u32, Size::Word, false, false);
                    self.state.d_reg[dest as usize] = (remainder << 16) | (0xFFFF & quotient);
                } else {
                    self.set_flag(Flags::Carry, false);
                    self.set_flag(Flags::Overflow, true);
                }
            },
            Instruction::DIVL(src, dest_h, dest_l, sign) => {
                let value = self.get_target_value(src, Size::Long, Used::Once)?;
                if value == 0 {
                    self.exception(Exceptions::ZeroDivide as u8, false)?;
                    return Ok(());
                }

                let existing_l = self.state.d_reg[dest_l as usize];
                let (remainder, quotient) = match sign {
                    Sign::Signed => {
                        let value = (value as i32) as i64;
                        let existing = match dest_h {
                            Some(reg) => (((self.state.d_reg[reg as usize] as u64) << 32) | (existing_l as u64)) as i64,
                            None => (existing_l as i32) as i64,
                        };
                        ((existing % value) as u64, (existing / value) as u64)
                    },
                    Sign::Unsigned => {
                        let value = value as u64;
                        let existing_h = dest_h.map(|reg| self.state.d_reg[reg as usize]).unwrap_or(0);
                        let existing = ((existing_h as u64) << 32) | (existing_l as u64);
                        (existing % value, existing / value)
                    },
                };

                self.set_compare_flags(quotient as u32, Size::Long, false, (quotient & 0xFFFFFFFF00000000) != 0);
                if let Some(dest_h) = dest_h {
                    self.state.d_reg[dest_h as usize] = remainder as u32;
                }
                self.state.d_reg[dest_l as usize] = quotient as u32;
            },
            Instruction::EOR(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let existing = self.get_target_value(dest, size, Used::Twice)?;
                let result = get_value_sized(existing ^ value, size);
                self.set_target_value(dest, result, size, Used::Twice)?;
                self.set_logic_flags(result, size);
            },
            Instruction::EORtoCCR(value) => {
                self.set_sr((self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) ^ (value as u16)));
            },
            Instruction::EORtoSR(value) => {
                self.require_supervisor()?;
                self.set_sr(self.state.sr ^ value);
            },
            Instruction::EXG(target1, target2) => {
                let value1 = self.get_target_value(target1, Size::Long, Used::Twice)?;
                let value2 = self.get_target_value(target2, Size::Long, Used::Twice)?;
                self.set_target_value(target1, value2, Size::Long, Used::Twice)?;
                self.set_target_value(target2, value1, Size::Long, Used::Twice)?;
            },
            Instruction::EXT(reg, from_size, to_size) => {
                let input = get_value_sized(self.state.d_reg[reg as usize], from_size);
                let result = match (from_size, to_size) {
                    (Size::Byte, Size::Word) => ((((input as u8) as i8) as i16) as u16) as u32,
                    (Size::Word, Size::Long) => (((input as u16) as i16) as i32) as u32,
                    (Size::Byte, Size::Long) => (((input as u8) as i8) as i32) as u32,
                    _ => panic!("Unsupported size for EXT instruction"),
                };
                set_value_sized(&mut self.state.d_reg[reg as usize], result, to_size);
                self.set_logic_flags(result, to_size);
            },
            Instruction::ILLEGAL => {
                self.exception(Exceptions::IllegalInstruction as u8, false)?;
            },
            Instruction::JMP(target) => {
                let addr = self.get_target_address(target)?;
                self.set_pc(addr)?;
            },
            Instruction::JSR(target) => {
                let previous_pc = self.state.pc;
                let addr = self.get_target_address(target)?;
                self.set_pc(addr)?;

                // If the address is good, then push the old PC onto the stack
                self.push_long(previous_pc)?;
                let sp = *self.get_stack_pointer_mut();
                self.debugger.stack_tracer.push_return(sp);
            },
            Instruction::LEA(target, reg) => {
                let value = self.get_target_address(target)?;
                let addr = self.get_a_reg_mut(reg);
                *addr = value;
            },
            Instruction::LINK(reg, offset) => {
                *self.get_stack_pointer_mut() -= 4;
                let sp = *self.get_stack_pointer_mut();
                let value = *self.get_a_reg_mut(reg);
                self.set_address_sized(sp as Address, value, Size::Long)?;
                *self.get_a_reg_mut(reg) = sp;
                *self.get_stack_pointer_mut() = (sp as i32).wrapping_add(offset as i32) as u32;
            },
            Instruction::LSd(count, target, size, shift_dir) => {
                let count = self.get_target_value(count, size, Used::Once)? % 64;
                let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
                for _ in 0..count {
                    pair = shift_operation(pair.0, size, shift_dir, false);
                }
                self.set_target_value(target, pair.0, size, Used::Twice)?;

                // Adjust flags
                self.set_logic_flags(pair.0, size);
                self.set_flag(Flags::Overflow, false);
                if count != 0 {
                    self.set_flag(Flags::Extend, pair.1);
                    self.set_flag(Flags::Carry, pair.1);
                } else {
                    self.set_flag(Flags::Carry, false);
                }
            },
            Instruction::MOVE(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                self.set_logic_flags(value, size);
                self.set_target_value(dest, value, size, Used::Once)?;
            },
            Instruction::MOVEA(src, reg, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let value = sign_extend_to_long(value, size) as u32;
                let addr = self.get_a_reg_mut(reg);
                *addr = value;
            },
            Instruction::MOVEfromSR(target) => {
                self.require_supervisor()?;
                self.set_target_value(target, self.state.sr as u32, Size::Word, Used::Once)?;
            },
            Instruction::MOVEtoSR(target) => {
                self.require_supervisor()?;
                let value = self.get_target_value(target, Size::Word, Used::Once)? as u16;
                self.set_sr(value);
            },
            Instruction::MOVEtoCCR(target) => {
                let value = self.get_target_value(target, Size::Word, Used::Once)? as u16;
                self.set_sr((self.state.sr & 0xFF00) | (value & 0x00FF));
            },
            Instruction::MOVEC(target, control_reg, dir) => {
                self.require_supervisor()?;
                match dir {
                    Direction::FromTarget => {
                        let value = self.get_target_value(target, Size::Long, Used::Once)?;
                        let addr = self.get_control_reg_mut(control_reg);
                        *addr = value;
                    },
                    Direction::ToTarget => {
                        let addr = self.get_control_reg_mut(control_reg);
                        let value = *addr;
                        self.set_target_value(target, value, Size::Long, Used::Once)?;
                    },
                }
            },
            Instruction::MOVEM(target, size, dir, mask) => {
                self.execute_movem(target, size, dir, mask)?;
            },
            Instruction::MOVEP(dreg, areg, offset, size, dir) => {
                match dir {
                    Direction::ToTarget => {
                        let mut shift = (size.in_bits() as i32) - 8;
                        let mut addr = ((*self.get_a_reg_mut(areg) as i32) + (offset as i32)) as Address;
                        while shift >= 0 {
                            let byte = (self.state.d_reg[dreg as usize] >> shift) as u8;
                            self.port.write_u8(addr, byte)?;
                            addr += 2;
                            shift -= 8;
                        }
                    },
                    Direction::FromTarget => {
                        let mut shift = (size.in_bits() as i32) - 8;
                        let mut addr = ((*self.get_a_reg_mut(areg) as i32) + (offset as i32)) as Address;
                        while shift >= 0 {
                            let byte = self.port.read_u8(addr)?;
                            self.state.d_reg[dreg as usize] |= (byte as u32) << shift;
                            addr += 2;
                            shift -= 8;
                        }
                    },
                }
            },
            Instruction::MOVEQ(data, reg) => {
                let value = sign_extend_to_long(data as u32, Size::Byte) as u32;
                self.state.d_reg[reg as usize] = value;
                self.set_logic_flags(value, Size::Long);
            },
            Instruction::MOVEUSP(target, dir) => {
                self.require_supervisor()?;
                match dir {
                    Direction::ToTarget => self.set_target_value(target, self.state.usp, Size::Long, Used::Once)?,
                    Direction::FromTarget => { self.state.usp = self.get_target_value(target, Size::Long, Used::Once)?; },
                }
            },
            Instruction::MULW(src, dest, sign) => {
                let value = self.get_target_value(src, Size::Word, Used::Once)?;
                let existing = get_value_sized(self.state.d_reg[dest as usize], Size::Word);
                let result = match sign {
                    Sign::Signed => ((((existing as u16) as i16) as i64) * (((value as u16) as i16) as i64)) as u64,
                    Sign::Unsigned => existing as u64 * value as u64,
                };

                self.set_compare_flags(result as u32, Size::Long, false, false);
                self.state.d_reg[dest as usize] = result as u32;
            },
            Instruction::MULL(src, dest_h, dest_l, sign) => {
                let value = self.get_target_value(src, Size::Long, Used::Once)?;
                let existing = get_value_sized(self.state.d_reg[dest_l as usize], Size::Long);
                let result = match sign {
                    Sign::Signed => (((existing as i32) as i64) * ((value as i32) as i64)) as u64,
                    Sign::Unsigned => existing as u64 * value as u64,
                };

                self.set_compare_flags(result as u32, Size::Long, false, false);
                if let Some(dest_h) = dest_h {
                    self.state.d_reg[dest_h as usize] = (result >> 32) as u32;
                }
                self.state.d_reg[dest_l as usize] = (result & 0x00000000FFFFFFFF) as u32;
            },
            //Instruction::NBCD(Target) => {
            //},
            Instruction::NEG(target, size) => {
                let original = self.get_target_value(target, size, Used::Twice)?;
                let (result, overflow) = overflowing_sub_signed_sized(0, original, size);
                let carry = result != 0;
                self.set_target_value(target, result, size, Used::Twice)?;
                self.set_compare_flags(result, size, carry, overflow);
                self.set_flag(Flags::Extend, carry);
            },
            Instruction::NEGX(dest, size) => {
                let existing = self.get_target_value(dest, size, Used::Twice)?;
                let extend = self.get_flag(Flags::Extend) as u32;
                let (result1, carry1) = overflowing_sub_sized(0, existing, size);
                let (result2, carry2) = overflowing_sub_sized(result1, extend, size);
                let overflow = get_sub_overflow(0, existing, result2, size);

                // Handle flags
                let zero = self.get_flag(Flags::Zero);
                self.set_compare_flags(result2, size, carry1 || carry2, overflow);
                if self.get_flag(Flags::Zero) {
                    // NEGX can only clear the zero flag, so if it's set, restore it to whatever it was before
                    self.set_flag(Flags::Zero, zero);
                }
                self.set_flag(Flags::Extend, carry1 || carry2);

                self.set_target_value(dest, result2, size, Used::Twice)?;
            },
            Instruction::NOP => { },
            Instruction::NOT(target, size) => {
                let mut value = self.get_target_value(target, size, Used::Twice)?;
                value = get_value_sized(!value, size);
                self.set_target_value(target, value, size, Used::Twice)?;
                self.set_logic_flags(value, size);
            },
            Instruction::OR(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let existing = self.get_target_value(dest, size, Used::Twice)?;
                let result = get_value_sized(existing | value, size);
                self.set_target_value(dest, result, size, Used::Twice)?;
                self.set_logic_flags(result, size);
            },
            Instruction::ORtoCCR(value) => {
                self.set_sr((self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) | (value as u16)));
            },
            Instruction::ORtoSR(value) => {
                self.require_supervisor()?;
                self.set_sr(self.state.sr | value);
            },
            Instruction::PEA(target) => {
                let value = self.get_target_address(target)?;
                self.push_long(value)?;
            },
            Instruction::RESET => {
                self.require_supervisor()?;
                // TODO this only resets external devices and not internal ones
            },
            Instruction::ROd(count, target, size, shift_dir) => {
                let count = self.get_target_value(count, size, Used::Once)? % 64;
                let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
                for _ in 0..count {
                    pair = rotate_operation(pair.0, size, shift_dir, None);
                }
                self.set_target_value(target, pair.0, size, Used::Twice)?;

                // Adjust flags
                self.set_logic_flags(pair.0, size);
                if pair.1 {
                    self.set_flag(Flags::Carry, true);
                }
            },
            Instruction::ROXd(count, target, size, shift_dir) => {
                let count = self.get_target_value(count, size, Used::Once)? % 64;
                let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
                for _ in 0..count {
                    pair = rotate_operation(pair.0, size, shift_dir, Some(self.get_flag(Flags::Extend)));
                    self.set_flag(Flags::Extend, pair.1);
                }
                self.set_target_value(target, pair.0, size, Used::Twice)?;

                // Adjust flags
                self.set_logic_flags(pair.0, size);
                if pair.1 {
                    self.set_flag(Flags::Carry, true);
                }
            },
            Instruction::RTE => {
                self.require_supervisor()?;
                let sr = self.pop_word()?;
                let addr = self.pop_long()?;

                if self.cputype >= M68kType::MC68010 {
                    let _ = self.pop_word()?;
                }

                self.set_sr(sr);
                self.set_pc(addr)?;
            },
            Instruction::RTR => {
                let ccr = self.pop_word()?;
                let addr = self.pop_long()?;
                self.set_sr((self.state.sr & 0xFF00) | (ccr & 0x00FF));
                self.set_pc(addr)?;
            },
            Instruction::RTS => {
                self.debugger.stack_tracer.pop_return();
                let addr = self.pop_long()?;
                self.set_pc(addr)?;
            },
            //Instruction::RTD(i16) => {
            //},
            Instruction::Scc(cond, target) => {
                let condition_true = self.get_current_condition(cond);
                if condition_true {
                    self.set_target_value(target, 0xFF, Size::Byte, Used::Once)?;
                } else {
                    self.set_target_value(target, 0x00, Size::Byte, Used::Once)?;
                }
            },
            Instruction::STOP(flags) => {
                self.require_supervisor()?;
                self.set_sr(flags);
                self.state.status = Status::Stopped;
            },
            Instruction::SBCD(src, dest) => {
                let value = convert_from_bcd(self.get_target_value(src, Size::Byte, Used::Once)? as u8);
                let existing = convert_from_bcd(self.get_target_value(dest, Size::Byte, Used::Twice)? as u8);
                let result = existing.wrapping_sub(value).wrapping_sub(self.get_flag(Flags::Extend) as u8);
                let borrow = existing < value;
                self.set_target_value(dest, convert_to_bcd(result) as u32, Size::Byte, Used::Twice)?;
                self.set_flag(Flags::Zero, result == 0);
                self.set_flag(Flags::Carry, borrow);
                self.set_flag(Flags::Extend, borrow);
            },
            Instruction::SUB(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let existing = self.get_target_value(dest, size, Used::Twice)?;
                let (result, carry) = overflowing_sub_sized(existing, value, size);
                let overflow = get_sub_overflow(existing, value, result, size);
                self.set_compare_flags(result, size, carry, overflow);
                self.set_flag(Flags::Extend, carry);
                self.set_target_value(dest, result, size, Used::Twice)?;
            },
            Instruction::SUBA(src, dest, size) => {
                let value = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
                let existing = *self.get_a_reg_mut(dest);
                let (result, _) = overflowing_sub_sized(existing, value, Size::Long);
                *self.get_a_reg_mut(dest) = result;
            },
            Instruction::SUBX(src, dest, size) => {
                let value = self.get_target_value(src, size, Used::Once)?;
                let existing = self.get_target_value(dest, size, Used::Twice)?;
                let extend = self.get_flag(Flags::Extend) as u32;
                let (result1, carry1) = overflowing_sub_sized(existing, value, size);
                let (result2, carry2) = overflowing_sub_sized(result1, extend, size);
                let overflow = get_sub_overflow(existing, value, result2, size);

                // Handle flags
                let zero = self.get_flag(Flags::Zero);
                self.set_compare_flags(result2, size, carry1 || carry2, overflow);
                if self.get_flag(Flags::Zero) {
                    // SUBX can only clear the zero flag, so if it's set, restore it to whatever it was before
                    self.set_flag(Flags::Zero, zero);
                }
                self.set_flag(Flags::Extend, carry1 || carry2);

                self.set_target_value(dest, result2, size, Used::Twice)?;
            },
            Instruction::SWAP(reg) => {
                let value = self.state.d_reg[reg as usize];
                self.state.d_reg[reg as usize] = ((value & 0x0000FFFF) << 16) | ((value & 0xFFFF0000) >> 16);
                self.set_logic_flags(self.state.d_reg[reg as usize], Size::Long);
            },
            Instruction::TAS(target) => {
                let value = self.get_target_value(target, Size::Byte, Used::Twice)?;
                self.set_flag(Flags::Negative, (value & 0x80) != 0);
                self.set_flag(Flags::Zero, value == 0);
                self.set_flag(Flags::Overflow, false);
                self.set_flag(Flags::Carry, false);
                self.set_target_value(target, value | 0x80, Size::Byte, Used::Twice)?;
            },
            Instruction::TST(target, size) => {
                let value = self.get_target_value(target, size, Used::Once)?;
                self.set_logic_flags(value, size);
            },
            Instruction::TRAP(number) => {
                self.exception(32 + number, false)?;
            },
            Instruction::TRAPV => {
                if self.get_flag(Flags::Overflow) {
                    self.exception(Exceptions::TrapvInstruction as u8, false)?;
                }
            },
            Instruction::UNLK(reg) => {
                let value = *self.get_a_reg_mut(reg);
                *self.get_stack_pointer_mut() = value;
                let new_value = self.pop_long()?;
                let addr = self.get_a_reg_mut(reg);
                *addr = new_value;
            },
            Instruction::UnimplementedA(_) => {
                self.state.pc -= 2;
                self.exception(Exceptions::LineAEmulator as u8, false)?;
            },
            Instruction::UnimplementedF(_) => {
                self.state.pc -= 2;
                self.exception(Exceptions::LineFEmulator as u8, false)?;
            },
            _ => { return Err(Error::new("Unsupported instruction")); },
        }

        self.timer.execute.end();
        Ok(())
    }

    fn execute_movem(&mut self, target: Target, size: Size, dir: Direction, mask: u16) -> Result<(), Error> {
        let addr = self.get_target_address(target)?;

        // If we're using a MC68020 or higher, and it was Post-Inc/Pre-Dec target, then update the value before it's stored
        if self.cputype >= M68kType::MC68020 {
            match target {
                Target::IndirectARegInc(reg) | Target::IndirectARegDec(reg) => {
                    let a_reg_mut = self.get_a_reg_mut(reg);
                    *a_reg_mut = addr + (mask.count_ones() * size.in_bytes());
                }
                _ => { },
            }
        }

        let post_addr = match target {
            Target::IndirectARegInc(_) => {
                if dir != Direction::FromTarget {
                    return Err(Error::new(&format!("Cannot use {:?} with {:?}", target, dir)));
                }
                self.move_memory_to_registers(addr, size, mask)?
            },
            Target::IndirectARegDec(_) => {
                if dir != Direction::ToTarget {
                    return Err(Error::new(&format!("Cannot use {:?} with {:?}", target, dir)));
                }
                self.move_registers_to_memory_reverse(addr, size, mask)?
            },
            _ => {
                match dir {
                    Direction::ToTarget => self.move_registers_to_memory(addr, size, mask)?,
                    Direction::FromTarget => self.move_memory_to_registers(addr, size, mask)?,
                }
            },
        };

        // If it was Post-Inc/Pre-Dec target, then update the value
        match target {
            Target::IndirectARegInc(reg) | Target::IndirectARegDec(reg) => {
                let a_reg_mut = self.get_a_reg_mut(reg);
                *a_reg_mut = post_addr;
            }
            _ => { },
        }

        Ok(())
    }

    pub fn move_memory_to_registers(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, Error> {
        for i in 0..8 {
            if (mask & 0x01) != 0 {
                self.state.d_reg[i] = sign_extend_to_long(self.get_address_sized(addr as Address, size)?, size) as u32;
                (addr, _) = overflowing_add_sized(addr, size.in_bytes(), Size::Long);
            }
            mask >>= 1;
        }
        for i in 0..8 {
            if (mask & 0x01) != 0 {
                *self.get_a_reg_mut(i) = sign_extend_to_long(self.get_address_sized(addr as Address, size)?, size) as u32;
                (addr, _) = overflowing_add_sized(addr, size.in_bytes(), Size::Long);
            }
            mask >>= 1;
        }
        Ok(addr)
    }

    pub fn move_registers_to_memory(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, Error> {
        for i in 0..8 {
            if (mask & 0x01) != 0 {
                self.set_address_sized(addr as Address, self.state.d_reg[i], size)?;
                addr += size.in_bytes();
            }
            mask >>= 1;
        }
        for i in 0..8 {
            if (mask & 0x01) != 0 {
                let value = *self.get_a_reg_mut(i);
                self.set_address_sized(addr as Address, value, size)?;
                addr += size.in_bytes();
            }
            mask >>= 1;
        }
        Ok(addr)
    }

    pub fn move_registers_to_memory_reverse(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, Error> {
        for i in (0..8).rev() {
            if (mask & 0x01) != 0 {
                let value = *self.get_a_reg_mut(i);
                addr -= size.in_bytes();
                self.set_address_sized(addr as Address, value, size)?;
            }
            mask >>= 1;
        }
        for i in (0..8).rev() {
            if (mask & 0x01) != 0 {
                addr -= size.in_bytes();
                self.set_address_sized(addr as Address, self.state.d_reg[i], size)?;
            }
            mask >>= 1;
        }
        Ok(addr)
    }

    pub fn get_target_value(&mut self, target: Target, size: Size, used: Used) -> Result<u32, Error> {
        match target {
            Target::Immediate(value) => Ok(value),
            Target::DirectDReg(reg) => Ok(get_value_sized(self.state.d_reg[reg as usize], size)),
            Target::DirectAReg(reg) => Ok(get_value_sized(*self.get_a_reg_mut(reg), size)),
            Target::IndirectAReg(reg) => {
                let addr = *self.get_a_reg_mut(reg);
                self.get_address_sized(addr as Address, size)
            },
            Target::IndirectARegInc(reg) => {
                let addr = self.post_increment_areg_target(reg, size, used);
                self.get_address_sized(addr as Address, size)
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.pre_decrement_areg_target(reg, size, Used::Once);
                self.get_address_sized(addr as Address, size)
            },
            Target::IndirectRegOffset(base_reg, index_reg, displacement) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                self.get_address_sized(base_value.wrapping_add(displacement as u32).wrapping_add(index_value as u32) as Address, size)
            },
            Target::IndirectMemoryPreindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32).wrapping_add(index_value as u32) as Address, Size::Long)?;
                self.get_address_sized(intermediate.wrapping_add(outer_disp as u32) as Address, size)
            },
            Target::IndirectMemoryPostindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32) as Address, Size::Long)?;
                self.get_address_sized(intermediate.wrapping_add(index_value as u32).wrapping_add(outer_disp as u32) as Address, size)
            },
            Target::IndirectMemory(addr, _) => {
                self.get_address_sized(addr as Address, size)
            },
        }
    }

    pub fn set_target_value(&mut self, target: Target, value: u32, size: Size, used: Used) -> Result<(), Error> {
        match target {
            Target::DirectDReg(reg) => {
                set_value_sized(&mut self.state.d_reg[reg as usize], value, size);
            },
            Target::DirectAReg(reg) => {
                set_value_sized(self.get_a_reg_mut(reg), value, size);
            },
            Target::IndirectAReg(reg) => {
                let addr = *self.get_a_reg_mut(reg);
                self.set_address_sized(addr as Address, value, size)?;
            },
            Target::IndirectARegInc(reg) => {
                let addr = self.post_increment_areg_target(reg, size, Used::Once);
                self.set_address_sized(addr as Address, value, size)?;
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.pre_decrement_areg_target(reg, size, used);
                self.set_address_sized(addr as Address, value, size)?;
            },
            Target::IndirectRegOffset(base_reg, index_reg, displacement) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                self.set_address_sized(base_value.wrapping_add(displacement as u32).wrapping_add(index_value as u32) as Address, value, size)?;
            },
            Target::IndirectMemoryPreindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32).wrapping_add(index_value as u32) as Address, Size::Long)?;
                self.set_address_sized(intermediate.wrapping_add(outer_disp as u32) as Address, value, size)?;
            },
            Target::IndirectMemoryPostindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32) as Address, Size::Long)?;
                self.set_address_sized(intermediate.wrapping_add(index_value as u32).wrapping_add(outer_disp as u32) as Address, value, size)?;
            },
            Target::IndirectMemory(addr, _) => {
                self.set_address_sized(addr as Address, value, size)?;
            },
            _ => return Err(Error::new(&format!("Unimplemented addressing target: {:?}", target))),
        }
        Ok(())
    }

    pub fn get_target_address(&mut self, target: Target) -> Result<u32, Error> {
        let addr = match target {
            Target::IndirectAReg(reg) | Target::IndirectARegInc(reg) | Target::IndirectARegDec(reg) => *self.get_a_reg_mut(reg),
            Target::IndirectRegOffset(base_reg, index_reg, displacement) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                base_value.wrapping_add(displacement as u32).wrapping_add(index_value as u32)
            },
            Target::IndirectMemoryPreindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32).wrapping_add(index_value as u32) as Address, Size::Long)?;
                intermediate.wrapping_add(outer_disp as u32)
            },
            Target::IndirectMemoryPostindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32) as Address, Size::Long)?;
                intermediate.wrapping_add(index_value as u32).wrapping_add(outer_disp as u32)
            },
            Target::IndirectMemory(addr, _) => {
                addr
            },
            _ => return Err(Error::new(&format!("Invalid addressing target: {:?}", target))),
        };
        Ok(addr)
    }

    pub fn post_increment_areg_target(&mut self, reg: Register, mut size: Size, used: Used) -> u32 {
        // If using A7 (the stack pointer) then increment by a minimum of 2 bytes to keep it word aligned
        if reg == 7 && size == Size::Byte {
            size = Size::Word;
        }

        let reg_addr = self.get_a_reg_mut(reg);
        let addr = *reg_addr;
        if used != Used::Twice {
            *reg_addr = addr.wrapping_add(size.in_bytes());
        }
        addr
    }

    pub fn pre_decrement_areg_target(&mut self, reg: Register, mut size: Size, used: Used) -> u32 {
        // If using A7 (the stack pointer) then decrement by a minimum of 2 bytes to keep it word aligned
        if reg == 7 && size == Size::Byte {
            size = Size::Word;
        }

        let reg_addr = self.get_a_reg_mut(reg);
        if used != Used::Twice {
            *reg_addr = (*reg_addr).wrapping_sub(size.in_bytes());
        }
        *reg_addr
    }

    pub fn get_address_sized(&mut self, addr: Address, size: Size) -> Result<u32, Error> {
        self.start_request(addr as u32, size, MemAccess::Read, MemType::Data, false)?;
        match size {
            Size::Byte => self.port.read_u8(addr).map(|value| value as u32),
            Size::Word => self.port.read_beu16(addr).map(|value| value as u32),
            Size::Long => self.port.read_beu32(addr),
        }
    }

    pub fn set_address_sized(&mut self, addr: Address, value: u32, size: Size) -> Result<(), Error> {
        self.start_request(addr as u32, size, MemAccess::Write, MemType::Data, false)?;
        match size {
            Size::Byte => self.port.write_u8(addr, value as u8),
            Size::Word => self.port.write_beu16(addr, value as u16),
            Size::Long => self.port.write_beu32(addr, value),
        }
    }

    pub fn start_instruction_request(&mut self, addr: u32) -> Result<u32, Error> {
        self.state.request.i_n_bit = false;
        self.state.request.code = FunctionCode::program(self.state.sr);
        self.state.request.access = MemAccess::Read;
        self.state.request.address = addr;

        validate_address(addr)
    }

    pub fn start_request(&mut self, addr: u32, size: Size, access: MemAccess, mtype: MemType, i_n_bit: bool) -> Result<u32, Error> {
        self.state.request.i_n_bit = i_n_bit;
        self.state.request.code = match mtype {
            MemType::Program => FunctionCode::program(self.state.sr),
            MemType::Data => FunctionCode::data(self.state.sr),
        };

        self.state.request.access = access;
        self.state.request.address = addr;

        if size == Size::Byte {
            Ok(addr)
        } else {
            validate_address(addr)
        }
    }

    fn push_word(&mut self, value: u16) -> Result<(), Error> {
        *self.get_stack_pointer_mut() -= 2;
        let addr = *self.get_stack_pointer_mut();
        self.start_request(addr, Size::Word, MemAccess::Write, MemType::Data, false)?;
        self.port.write_beu16(addr as Address, value)
    }

    fn pop_word(&mut self) -> Result<u16, Error> {
        let addr = *self.get_stack_pointer_mut();
        let value = self.port.read_beu16(addr as Address)?;
        self.start_request(addr, Size::Word, MemAccess::Read, MemType::Data, false)?;
        *self.get_stack_pointer_mut() += 2;
        Ok(value)
    }

    fn push_long(&mut self, value: u32) -> Result<(), Error> {
        *self.get_stack_pointer_mut() -= 4;
        let addr = *self.get_stack_pointer_mut();
        self.start_request(addr, Size::Long, MemAccess::Write, MemType::Data, false)?;
        self.port.write_beu32(addr as Address, value)
    }

    fn pop_long(&mut self) -> Result<u32, Error> {
        let addr = *self.get_stack_pointer_mut();
        let value = self.port.read_beu32(addr as Address)?;
        self.start_request(addr, Size::Long, MemAccess::Read, MemType::Data, false)?;
        *self.get_stack_pointer_mut() += 4;
        Ok(value)
    }

    fn set_pc(&mut self, value: u32) -> Result<(), Error> {
        self.state.pc = value;
        self.start_request(self.state.pc, Size::Word, MemAccess::Read, MemType::Program, true)?;
        Ok(())
    }

    pub fn get_bit_field_args(&self, offset: RegOrImmediate, width: RegOrImmediate) -> (u32, u32) {
        let offset = self.get_reg_or_immediate(offset);
        let mut width = self.get_reg_or_immediate(width) % 32;
        if width == 0 {
            width = 32;
        }
        (offset, width)
    }

    fn get_reg_or_immediate(&self, value: RegOrImmediate) -> u32 {
        match value {
            RegOrImmediate::DReg(reg) => self.state.d_reg[reg as usize],
            RegOrImmediate::Immediate(value) => value as u32,
        }
    }

    fn get_x_reg_value(&self, xreg: XRegister) -> u32 {
        match xreg {
            XRegister::DReg(reg) => self.state.d_reg[reg as usize],
            XRegister::AReg(reg) => self.get_a_reg(reg),
        }
    }

    fn get_base_reg_value(&self, base_reg: BaseRegister) -> u32 {
        match base_reg {
            BaseRegister::None => 0,
            BaseRegister::PC => self.decoder.start + 2,
            BaseRegister::AReg(reg) if reg == 7 => if self.is_supervisor() { self.state.ssp } else { self.state.usp },
            BaseRegister::AReg(reg) => self.state.a_reg[reg as usize],
        }
    }

    fn get_index_reg_value(&self, index_reg: &Option<IndexRegister>) -> i32 {
        match index_reg {
            None => 0,
            Some(IndexRegister { xreg, scale, size }) => {
                sign_extend_to_long(self.get_x_reg_value(*xreg), *size) << scale
            }
        }
    }

    fn get_control_reg_mut(&mut self, control_reg: ControlRegister) -> &mut u32 {
        match control_reg {
            ControlRegister::VBR => &mut self.state.vbr,
        }
    }

    #[inline(always)]
    fn get_stack_pointer_mut(&mut self) -> &mut u32 {
        if self.is_supervisor() { &mut self.state.ssp } else { &mut self.state.usp }
    }

    #[inline(always)]
    fn get_a_reg(&self, reg: Register) -> u32 {
        if reg == 7 {
            if self.is_supervisor() { self.state.ssp } else { self.state.usp }
        } else {
            self.state.a_reg[reg as usize]
        }
    }

    #[inline(always)]
    fn get_a_reg_mut(&mut self, reg: Register) -> &mut u32 {
        if reg == 7 {
            if self.is_supervisor() { &mut self.state.ssp } else { &mut self.state.usp }
        } else {
            &mut self.state.a_reg[reg as usize]
        }
    }

    #[inline(always)]
    fn is_supervisor(&self) -> bool {
        self.state.sr & (Flags:: Supervisor as u16) != 0
    }

    #[inline(always)]
    fn require_supervisor(&self) -> Result<(), Error> {
        if self.is_supervisor() {
            Ok(())
        } else {
            Err(Error::processor(Exceptions::PrivilegeViolation as u32))
        }
    }

    fn set_sr(&mut self, value: u16) {
        let mask = if self.cputype <= M68kType::MC68010 { 0xA71F } else { 0xF71F };
        self.state.sr = value & mask;
    }

    #[inline(always)]
    fn get_flag(&self, flag: Flags) -> bool {
        (self.state.sr & (flag as u16)) != 0
    }

    #[inline(always)]
    fn set_flag(&mut self, flag: Flags, value: bool) {
        self.state.sr = (self.state.sr & !(flag as u16)) | (if value { flag as u16 } else { 0 });
    }

    fn set_compare_flags(&mut self, value: u32, size: Size, carry: bool, overflow: bool) {
        let value = sign_extend_to_long(value, size);

        let mut flags = 0x0000;
        if value < 0 {
            flags |= Flags::Negative as u16;
        }
        if value == 0 {
            flags |= Flags::Zero as u16;
        }
        if carry {
            flags |= Flags::Carry as u16;
        }
        if overflow {
            flags |= Flags::Overflow as u16;
        }
        self.state.sr = (self.state.sr & 0xFFF0) | flags;
    }

    fn set_logic_flags(&mut self, value: u32, size: Size) {
        let mut flags = 0x0000;
        if get_msb(value, size) {
            flags |= Flags::Negative as u16;
        }
        if value == 0 {
            flags |= Flags::Zero as u16;
        }
        self.state.sr = (self.state.sr & 0xFFF0) | flags;
    }

    fn set_bit_test_flags(&mut self, value: u32, bitnum: u32, size: Size) -> u32 {
        let mask = 0x1 << (bitnum % size.in_bits());
        self.set_flag(Flags::Zero, (value & mask) == 0);
        mask
    }

    fn set_bit_field_test_flags(&mut self, field: u32, msb_mask: u32) {
        let mut flags = 0x0000;
        if (field & msb_mask) != 0 {
            flags |= Flags::Negative as u16;
        }
        if field == 0 {
            flags |= Flags::Zero as u16;
        }
        self.state.sr = (self.state.sr & 0xFFF0) | flags;
    }

    fn get_current_condition(&self, cond: Condition) -> bool {
        match cond {
            Condition::True => true,
            Condition::False => false,
            Condition::High => !self.get_flag(Flags::Carry) && !self.get_flag(Flags::Zero),
            Condition::LowOrSame => self.get_flag(Flags::Carry) || self.get_flag(Flags::Zero),
            Condition::CarryClear => !self.get_flag(Flags::Carry),
            Condition::CarrySet => self.get_flag(Flags::Carry),
            Condition::NotEqual => !self.get_flag(Flags::Zero),
            Condition::Equal => self.get_flag(Flags::Zero),
            Condition::OverflowClear => !self.get_flag(Flags::Overflow),
            Condition::OverflowSet => self.get_flag(Flags::Overflow),
            Condition::Plus => !self.get_flag(Flags::Negative),
            Condition::Minus => self.get_flag(Flags::Negative),
            Condition::GreaterThanOrEqual => (self.get_flag(Flags::Negative) && self.get_flag(Flags::Overflow)) || (!self.get_flag(Flags::Negative) && !self.get_flag(Flags::Overflow)),
            Condition::LessThan => (self.get_flag(Flags::Negative) && !self.get_flag(Flags::Overflow)) || (!self.get_flag(Flags::Negative) && self.get_flag(Flags::Overflow)),
            Condition::GreaterThan =>
                (self.get_flag(Flags::Negative) && self.get_flag(Flags::Overflow) && !self.get_flag(Flags::Zero))
                || (!self.get_flag(Flags::Negative) && !self.get_flag(Flags::Overflow) && !self.get_flag(Flags::Zero)),
            Condition::LessThanOrEqual =>
                self.get_flag(Flags::Zero)
                || (self.get_flag(Flags::Negative) && !self.get_flag(Flags::Overflow))
                || (!self.get_flag(Flags::Negative) && self.get_flag(Flags::Overflow)),
        }
    }
}

fn validate_address(addr: u32) -> Result<u32, Error> {
    if addr & 0x1 == 0 {
        Ok(addr)
    } else {
        Err(Error::processor(Exceptions::AddressError as u32))
    }
}

fn overflowing_add_sized(operand1: u32, operand2: u32, size: Size) -> (u32, bool) {
    match size {
        Size::Byte => {
            let (result, carry) = (operand1 as u8).overflowing_add(operand2 as u8);
            (result as u32, carry)
        },
        Size::Word => {
            let (result, carry) = (operand1 as u16).overflowing_add(operand2 as u16);
            (result as u32, carry)
        },
        Size::Long => operand1.overflowing_add(operand2),
    }
}

fn overflowing_sub_sized(operand1: u32, operand2: u32, size: Size) -> (u32, bool) {
    match size {
        Size::Byte => {
            let (result, carry) = (operand1 as u8).overflowing_sub(operand2 as u8);
            (result as u32, carry)
        },
        Size::Word => {
            let (result, carry) = (operand1 as u16).overflowing_sub(operand2 as u16);
            (result as u32, carry)
        },
        Size::Long => operand1.overflowing_sub(operand2),
    }
}

fn overflowing_sub_signed_sized(operand1: u32, operand2: u32, size: Size) -> (u32, bool) {
    match size {
        Size::Byte => {
            let (result, overflow) = (operand1 as i8).overflowing_sub(operand2 as i8);
            (result as u32, overflow)
        },
        Size::Word => {
            let (result, overflow) = (operand1 as i16).overflowing_sub(operand2 as i16);
            (result as u32, overflow)
        },
        Size::Long => {
            let (result, overflow) = (operand1 as i32).overflowing_sub(operand2 as i32);
            (result as u32, overflow)
        },
    }
}

fn shift_operation(value: u32, size: Size, dir: ShiftDirection, arithmetic: bool) -> (u32, bool) {
    match dir {
        ShiftDirection::Left => {
            let bit = get_msb(value, size);
            match size {
                Size::Byte => (((value as u8) << 1) as u32, bit),
                Size::Word => (((value as u16) << 1) as u32, bit),
                Size::Long => ((value << 1) as u32, bit),
            }
        },
        ShiftDirection::Right => {
            let mask = if arithmetic { get_msb_mask(value, size) } else { 0 };
            ((value >> 1) | mask, (value & 0x1) != 0)
        },
    }
}

fn rotate_operation(value: u32, size: Size, dir: ShiftDirection, use_extend: Option<bool>) -> (u32, bool) {
    match dir {
        ShiftDirection::Left => {
            let bit = get_msb(value, size);
            let mask = if use_extend.unwrap_or(bit) { 0x01 } else { 0x00 };
            match size {
                Size::Byte => (mask | ((value as u8) << 1) as u32, bit),
                Size::Word => (mask | ((value as u16) << 1) as u32, bit),
                Size::Long => (mask | (value << 1) as u32, bit),
            }
        },
        ShiftDirection::Right => {
            let bit = if (value & 0x01) != 0 { true } else { false };
            let mask = if use_extend.unwrap_or(bit) { get_msb_mask(0xffffffff, size) } else { 0x0 };
            ((value >> 1) | mask, bit)
        },
    }
}

fn convert_from_bcd(value: u8) -> u8 {
    (value >> 4) * 10 + (value & 0x0F)
}

fn convert_to_bcd(value: u8) -> u8 {
    (((value / 10) & 0x0F) << 4) | ((value % 10) & 0x0F)
}

fn get_value_sized(value: u32, size: Size) -> u32 {
    match size {
        Size::Byte => { 0x000000FF & value },
        Size::Word => { 0x0000FFFF & value },
        Size::Long => { value },
    }
}

fn set_value_sized(addr: &mut u32, value: u32, size: Size) {
    match size {
        Size::Byte => { *addr = (*addr & 0xFFFFFF00) | (0x000000FF & value); }
        Size::Word => { *addr = (*addr & 0xFFFF0000) | (0x0000FFFF & value); }
        Size::Long => { *addr = value; }
    }
}

fn get_add_overflow(operand1: u32, operand2: u32, result: u32, size: Size) -> bool {
    let msb1 = get_msb(operand1, size);
    let msb2 = get_msb(operand2, size);
    let msb_res = get_msb(result, size);

    (msb1 && msb2 && !msb_res) || (!msb1 && !msb2 && msb_res)
}

fn get_sub_overflow(operand1: u32, operand2: u32, result: u32, size: Size) -> bool {
    let msb1 = get_msb(operand1, size);
    let msb2 = !get_msb(operand2, size);
    let msb_res = get_msb(result, size);

    (msb1 && msb2 && !msb_res) || (!msb1 && !msb2 && msb_res)
}

#[inline(always)]
fn get_msb(value: u32, size: Size) -> bool {
    match size {
        Size::Byte => (value & 0x00000080) != 0,
        Size::Word => (value & 0x00008000) != 0,
        Size::Long => (value & 0x80000000) != 0,
    }
}

#[inline(always)]
fn get_msb_mask(value: u32, size: Size) -> u32 {
    match size {
        Size::Byte => value & 0x00000080,
        Size::Word => value & 0x00008000,
        Size::Long => value & 0x80000000,
    }
}

fn get_bit_field_mask(offset: u32, width: u32) -> u32 {
    let mut mask = 0;
    for _ in 0..width {
        mask = (mask >> 1) | 0x80000000;
    }
    mask >> offset
}

fn get_bit_field_msb(offset: u32) -> u32 {
    0x80000000 >> offset
}


