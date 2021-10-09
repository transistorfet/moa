
use crate::error::Error;
use crate::system::System;
use crate::memory::{Address, Addressable};
use crate::devices::{Clock, Steppable, Interruptable};

use super::decode::{
    M68kDecoder,
    Instruction,
    Target,
    Size,
    Sign,
    Direction,
    Condition,
    ShiftDirection,
    ControlRegister,
    RegisterType,
    sign_extend_to_long
};

use super::state::{MC68010, Status, Flags, InterruptPriority};

impl Steppable for MC68010 {
    fn step(&mut self, system: &System) -> Result<Clock, Error> {
        self.step_internal(system)?;
        Ok(1)
    }

    fn on_error(&mut self, system: &System) {
        self.dump_state(system);
    }
}

impl Interruptable for MC68010 {
    fn interrupt_state_change(&mut self, state: bool, priority: u8, number: u8) -> Result<(), Error> {
        let ipl = if state {
            InterruptPriority::from_u8(priority)
        } else {
            InterruptPriority::NoInterrupt
        };

        if ipl != self.state.pending_ipl {
            self.state.pending_ipl = ipl;
            if ipl != InterruptPriority::NoInterrupt {
                self.state.ipl_ack_num = number;
            }
        }
        Ok(())
    }
}

impl MC68010 {
    pub fn is_running(&self) -> bool {
        self.state.status != Status::Stopped
    }

    pub fn init(&mut self, system: &System) -> Result<(), Error> {
        println!("Initializing CPU");

        self.state.msp = system.get_bus().read_beu32(0)?;
        self.state.pc = system.get_bus().read_beu32(4)?;
        self.state.status = Status::Running;

        Ok(())
    }

    pub fn step_internal(&mut self, system: &System) -> Result<(), Error> {
        match self.state.status {
            Status::Init => self.init(system),
            Status::Stopped | Status::Halted => Err(Error::new("CPU stopped")),
            Status::Running => {
                let timer = self.timer.cycle.start();
                self.decode_next(system)?;
                self.execute_current(system)?;
                self.timer.cycle.end(timer);

                //if (self.timer.cycle.events % 500) == 0 {
                //    println!("{}", self.timer);
                //}

                self.check_pending_interrupts(system)?;

                Ok(())
            },
        }
    }

    pub fn check_pending_interrupts(&mut self, system: &System) -> Result<(), Error> {
        let current_ipl = self.state.current_ipl as u8;
        let pending_ipl = self.state.pending_ipl as u8;

        if self.state.pending_ipl != InterruptPriority::NoInterrupt {
            let priority_mask = ((self.state.sr & 0x700) >> 8) as u8;

            if (pending_ipl >= priority_mask || pending_ipl == 7) && pending_ipl >= current_ipl {
                self.state.current_ipl = self.state.pending_ipl;
                self.exception(system, self.state.ipl_ack_num)?;
                return Ok(());
            }
        }

        if pending_ipl < current_ipl {
            self.state.current_ipl = self.state.pending_ipl;
        }

        Ok(())
    }

    pub fn exception(&mut self, system: &System, number: u8) -> Result<(), Error> {
        println!("raising exception {}", number);
        let offset = (number as u16) << 2;
        self.push_word(system, offset)?;
        self.push_long(system, self.state.pc)?;
        self.push_word(system, self.state.sr)?;
        self.set_flag(Flags::Supervisor, true);
        self.set_flag(Flags::Tracing, false);
        self.state.pc = system.get_bus().read_beu32((self.state.vbr + offset as u32) as Address)?;
        Ok(())
    }

    pub fn decode_next(&mut self, system: &System) -> Result<(), Error> {
        self.check_breakpoints();

        let timer = self.timer.decode.start();
        self.decoder = M68kDecoder::decode_at(system, self.state.pc)?;
        self.timer.decode.end(timer);

        if self.debugger.use_tracing {
            // Print instruction bytes for debugging
            let ins_data: Result<String, Error> =
                (0..((self.decoder.end - self.decoder.start) / 2)).map(|offset|
                    Ok(format!("{:04x} ", system.get_bus().read_beu16((self.decoder.start + (offset * 2)) as Address)?))
                ).collect();
            debug!("{:#010x}: {}\n\t{:?}\n", self.decoder.start, ins_data?, self.decoder.instruction);
        }

        if self.debugger.use_debugger {
            self.run_debugger(system);
        }

        self.state.pc = self.decoder.end;
        Ok(())
    }

    pub fn execute_current(&mut self, system: &System) -> Result<(), Error> {
        let timer = self.timer.decode.start();
        match self.decoder.instruction {
            Instruction::ADD(src, dest, size) => {
                let value = self.get_target_value(system, src, size)?;
                let existing = self.get_target_value(system, dest, size)?;
                let (result, carry) = overflowing_add_sized(existing, value, size);
                match dest {
                    Target::DirectAReg(_) => { },
                    _ => self.set_compare_flags(result, size, carry, get_overflow(existing, value, result, size)),
                }
                self.set_target_value(system, dest, result, size)?;
            },
            Instruction::AND(src, dest, size) => {
                let value = self.get_target_value(system, src, size)?;
                let existing = self.get_target_value(system, dest, size)?;
                let result = get_value_sized(existing & value, size);
                self.set_target_value(system, dest, result, size)?;
                self.set_logic_flags(result, size);
            },
            Instruction::ANDtoCCR(value) => {
                self.state.sr = self.state.sr | (value as u16);
            },
            Instruction::ANDtoSR(value) => {
                self.state.sr = self.state.sr | value;
            },
            Instruction::ASd(count, target, size, shift_dir) => {
                let count = self.get_target_value(system, count, size)? % 64;
                let mut pair = (self.get_target_value(system, target, size)?, false);
                let original = pair.0;
                for _ in 0..count {
                    pair = shift_operation(pair.0, size, shift_dir, true);
                }
                self.set_logic_flags(pair.0, size);
                if pair.1 {
                    self.set_flag(Flags::Carry, true);
                    self.set_flag(Flags::Extend, true);
                }
                if get_msb(pair.0, size) != get_msb(original, size) {
                    self.set_flag(Flags::Overflow, true);
                }
                self.set_target_value(system, target, pair.0, size)?;
            },
            Instruction::Bcc(cond, offset) => {
                let should_branch = self.get_current_condition(cond);
                if should_branch {
                    self.state.pc = (self.decoder.start + 2).wrapping_add(offset as u32);
                }
            },
            Instruction::BRA(offset) => {
                self.state.pc = (self.decoder.start + 2).wrapping_add(offset as u32);
            },
            Instruction::BSR(offset) => {
                self.push_long(system, self.state.pc)?;
                let sp = *self.get_stack_pointer_mut();
                self.debugger.stack_tracer.push_return(sp);
                self.state.pc = (self.decoder.start + 2).wrapping_add(offset as u32);
            },
            Instruction::BTST(bitnum, target, size) => {
                let bitnum = self.get_target_value(system, bitnum, Size::Byte)?;
                let value = self.get_target_value(system, target, size)?;
                self.set_bit_test_flags(value, bitnum, size);
            },
            Instruction::BCHG(bitnum, target, size) => {
                let bitnum = self.get_target_value(system, bitnum, Size::Byte)?;
                let mut value = self.get_target_value(system, target, size)?;
                let mask = self.set_bit_test_flags(value, bitnum, size);
                value = (value & !mask) | (!(value & mask) & mask);
                self.set_target_value(system, target, value, size)?;
            },
            Instruction::BCLR(bitnum, target, size) => {
                let bitnum = self.get_target_value(system, bitnum, Size::Byte)?;
                let mut value = self.get_target_value(system, target, size)?;
                let mask = self.set_bit_test_flags(value, bitnum, size);
                value = value & !mask;
                self.set_target_value(system, target, value, size)?;
            },
            Instruction::BSET(bitnum, target, size) => {
                let bitnum = self.get_target_value(system, bitnum, Size::Byte)?;
                let mut value = self.get_target_value(system, target, size)?;
                let mask = self.set_bit_test_flags(value, bitnum, size);
                value = value | mask;
                self.set_target_value(system, target, value, size)?;
            },
            Instruction::CLR(target, size) => {
                self.set_target_value(system, target, 0, size)?;
                // Clear flags except Zero flag
                self.state.sr = (self.state.sr & 0xFFF0) | (Flags::Zero as u16);
            },
            Instruction::CMP(src, dest, size) => {
                let value = self.get_target_value(system, src, size)?;
                let existing = self.get_target_value(system, dest, size)?;
                let (result, carry) = overflowing_sub_sized(existing, value, size);
                self.set_compare_flags(result, size, carry, get_overflow(existing, value, result, size));
            },
            Instruction::CMPA(src, reg, size) => {
                let value = sign_extend_to_long(self.get_target_value(system, src, size)?, size) as u32;
                let existing = *self.get_a_reg_mut(reg);
                let (result, carry) = overflowing_sub_sized(existing, value, Size::Long);
                self.set_compare_flags(result, Size::Long, carry, get_overflow(existing, value, result, Size::Long));
            },
            Instruction::DBcc(cond, reg, offset) => {
                let condition_true = self.get_current_condition(cond);
                if !condition_true {
                    let next = (get_value_sized(self.state.d_reg[reg as usize], Size::Word) as u16) as i16 - 1;
                    set_value_sized(&mut self.state.d_reg[reg as usize], next as u32, Size::Word);
                    if next != -1 {
                        self.state.pc = (self.decoder.start + 2).wrapping_add(offset as u32);
                    }
                }
            },
            Instruction::DIV(src, dest, size, sign) => {
                if size == Size::Long {
                    return Err(Error::new("Unsupported multiplication size"));
                }

                let value = self.get_target_value(system, src, size)?;
                let existing = self.get_target_value(system, dest, Size::Long)?;
                let result = match sign {
                    Sign::Signed => ((existing as i16 % value as i16) as u32) << 16 | (0xFFFF & (existing as i16 / value as i16) as u32),
                    Sign::Unsigned => ((existing as u16 % value as u16) as u32) << 16 | (0xFFFF & (existing as u16 / value as u16) as u32),
                };
                self.set_target_value(system, dest, result, Size::Long)?;
            },
            Instruction::EOR(src, dest, size) => {
                let value = self.get_target_value(system, src, size)?;
                let existing = self.get_target_value(system, dest, size)?;
                let result = get_value_sized(existing ^ value, size);
                self.set_target_value(system, dest, result, size)?;
                self.set_logic_flags(result, size);
            },
            Instruction::EORtoCCR(value) => {
                self.state.sr = self.state.sr ^ (value as u16);
            },
            Instruction::EORtoSR(value) => {
                self.state.sr = self.state.sr ^ value;
            },
            //Instruction::EXG(Target, Target) => {
            //},
            Instruction::EXT(reg, size) => {
                let byte = (self.state.d_reg[reg as usize] as u8) as i8;
                let result = match size {
                    Size::Byte => (byte as u8) as u32,
                    Size::Word => ((byte as i16) as u16) as u32,
                    Size::Long => (byte as i32) as u32,
                };
                set_value_sized(&mut self.state.d_reg[reg as usize], result, size);
                self.set_logic_flags(result, size);
            },
            //Instruction::ILLEGAL => {
            //},
            Instruction::JMP(target) => {
                self.state.pc = self.get_target_address(target)?;
            },
            Instruction::JSR(target) => {
                self.push_long(system, self.state.pc)?;
                let sp = *self.get_stack_pointer_mut();
                self.debugger.stack_tracer.push_return(sp);
                self.state.pc = self.get_target_address(target)?;
            },
            Instruction::LEA(target, reg) => {
                let value = self.get_target_address(target)?;
                let addr = self.get_a_reg_mut(reg);
                *addr = value;
            },
            Instruction::LINK(reg, offset) => {
                let value = *self.get_a_reg_mut(reg);
                self.push_long(system, value)?;
                let sp = *self.get_stack_pointer_mut();
                let addr = self.get_a_reg_mut(reg);
                *addr = sp;
                *self.get_stack_pointer_mut() = sp.wrapping_add((offset as i32) as u32);
            },
            Instruction::LSd(count, target, size, shift_dir) => {
                let count = self.get_target_value(system, count, size)? % 64;
                let mut pair = (self.get_target_value(system, target, size)?, false);
                for _ in 0..count {
                    pair = shift_operation(pair.0, size, shift_dir, false);
                }
                self.set_logic_flags(pair.0, size);
                if pair.1 {
                    self.set_flag(Flags::Carry, true);
                    self.set_flag(Flags::Extend, true);
                }
                self.set_target_value(system, target, pair.0, size)?;
            },
            Instruction::MOVE(src, dest, size) => {
                let value = self.get_target_value(system, src, size)?;
                self.set_logic_flags(value, size);
                self.set_target_value(system, dest, value, size)?;
            },
            Instruction::MOVEA(src, reg, size) => {
                let value = self.get_target_value(system, src, size)?;
                let value = sign_extend_to_long(value, size) as u32;
                let addr = self.get_a_reg_mut(reg);
                *addr = value;
            },
            Instruction::MOVEfromSR(target) => {
                self.set_target_value(system, target, self.state.sr as u32, Size::Word)?;
            },
            Instruction::MOVEtoSR(target) => {
                self.state.sr = self.get_target_value(system, target, Size::Word)? as u16;
            },
            Instruction::MOVEtoCCR(target) => {
                let value = self.get_target_value(system, target, Size::Word)? as u16;
                self.state.sr = (self.state.sr & 0xFF00) | (value & 0x00FF);
            },
            Instruction::MOVEC(target, control_reg, dir) => {
                match dir {
                    Direction::FromTarget => {
                        let value = self.get_target_value(system, target, Size::Long)?;
                        let addr = self.get_control_reg_mut(control_reg);
                        *addr = value;
                    },
                    Direction::ToTarget => {
                        let addr = self.get_control_reg_mut(control_reg);
                        let value = *addr;
                        self.set_target_value(system, target, value, Size::Long)?;
                    },
                }
            },
            Instruction::MOVEUSP(target, dir) => {
                match dir {
                    Direction::ToTarget => self.set_target_value(system, target, self.state.usp, Size::Long)?,
                    Direction::FromTarget => { self.state.usp = self.get_target_value(system, target, Size::Long)?; },
                }
            },
            Instruction::MOVEM(target, size, dir, mask) => {
                // TODO moving words requires a sign extension to 32 bits
                if size != Size::Long { return Err(Error::new("Unsupported size in MOVEM instruction")); }

                let mut addr = self.get_target_address(target)?;
                if dir == Direction::ToTarget {
                    let mut mask = mask;
                    for i in (0..8).rev() {
                        if (mask & 0x01) != 0 {
                            let value = *self.get_a_reg_mut(i);
                            addr -= size.in_bytes();
                            set_address_sized(system, addr as Address, value, size)?;
                        }
                        mask >>= 1;
                    }
                    for i in (0..8).rev() {
                        if (mask & 0x01) != 0 {
                            addr -= size.in_bytes();
                            set_address_sized(system, addr as Address, self.state.d_reg[i], size)?;
                        }
                        mask >>= 1;
                    }
                } else {
                    let mut mask = mask;
                    for i in 0..8 {
                        if (mask & 0x01) != 0 {
                            self.state.d_reg[i] = get_address_sized(system, addr as Address, size)?;
                            addr += size.in_bytes();
                        }
                        mask >>= 1;
                    }
                    for i in 0..8 {
                        if (mask & 0x01) != 0 {
                            *self.get_a_reg_mut(i) = get_address_sized(system, addr as Address, size)?;
                            addr += size.in_bytes();
                        }
                        mask >>= 1;
                    }
                }

                // If it was Post-Inc/Pre-Dec target, then update the value
                match target {
                    Target::IndirectARegInc(reg) | Target::IndirectARegDec(reg) => {
                        let a_reg_mut = self.get_a_reg_mut(reg);
                        *a_reg_mut = addr;
                    }
                    _ => { },
                }
            },
            Instruction::MOVEQ(data, reg) => {
                let value = sign_extend_to_long(data as u32, Size::Byte) as u32;
                self.state.d_reg[reg as usize] = value;
                self.set_logic_flags(value, Size::Long);
            },
            Instruction::MUL(src, dest, size, sign) => {
                if size == Size::Long {
                    return Err(Error::new("Unsupported multiplication size"));
                }

                let value = self.get_target_value(system, src, size)?;
                let existing = self.get_target_value(system, dest, size)?;
                let result = match sign {
                    Sign::Signed => (sign_extend_to_long(existing, Size::Word) * sign_extend_to_long(value, Size::Word)) as u32,
                    Sign::Unsigned => existing as u32 * value as u32,
                };
                self.set_target_value(system, dest, result, Size::Long)?;
            },
            //Instruction::NBCD(Target) => {
            //},
            Instruction::NEG(target, size) => {
                let original = self.get_target_value(system, target, size)?;
                let (value, _) = (0 as u32).overflowing_sub(original);
                self.set_target_value(system, target, value, size)?;
                self.set_compare_flags(value, size, value != 0, get_overflow(0, original, value, size));
            },
            //Instruction::NEGX(Target, Size) => {
            //},
            Instruction::NOP => { },
            Instruction::NOT(target, size) => {
                let mut value = self.get_target_value(system, target, size)?;
                value = get_value_sized(!value, size);
                self.set_target_value(system, target, value, size)?;
                self.set_logic_flags(value, size);
            },
            Instruction::OR(src, dest, size) => {
                let value = self.get_target_value(system, src, size)?;
                let existing = self.get_target_value(system, dest, size)?;
                let result = get_value_sized(existing | value, size);
                self.set_target_value(system, dest, result, size)?;
                self.set_logic_flags(result, size);
            },
            Instruction::ORtoCCR(value) => {
                self.state.sr = self.state.sr | (value as u16);
            },
            Instruction::ORtoSR(value) => {
                self.state.sr = self.state.sr | value;
            },
            Instruction::PEA(target) => {
                let value = self.get_target_address(target)?;
                self.push_long(system, value)?;
            },
            //Instruction::RESET => {
            //},
            Instruction::ROd(count, target, size, shift_dir) => {
                let count = self.get_target_value(system, count, size)? % 64;
                let mut pair = (self.get_target_value(system, target, size)?, false);
                for _ in 0..count {
                    pair = rotate_operation(pair.0, size, shift_dir);
                }
                self.set_logic_flags(pair.0, size);
                if pair.1 {
                    self.set_flag(Flags::Carry, true);
                }
                self.set_target_value(system, target, pair.0, size)?;
            },
            //Instruction::ROXd(Target, Target, Size, ShiftDirection) => {
            //},
            Instruction::RTE => {
                self.state.sr = self.pop_word(system)?;
                self.state.pc = self.pop_long(system)?;
                let _ = self.pop_word(system)?;
            },
            //Instruction::RTR => {
            //},
            Instruction::RTS => {
                self.debugger.stack_tracer.pop_return();
                self.state.pc = self.pop_long(system)?;
            },
            Instruction::Scc(cond, target) => {
                let condition_true = self.get_current_condition(cond);
                if condition_true {
                    self.set_target_value(system, target, 0xFF, Size::Byte)?;
                } else {
                    self.set_target_value(system, target, 0x00, Size::Byte)?;
                }
            },
            Instruction::STOP(flags) => {
                self.state.sr = flags;
                self.state.status = Status::Stopped;
            },
            Instruction::SUB(src, dest, size) => {
                let value = self.get_target_value(system, src, size)?;
                let existing = self.get_target_value(system, dest, size)?;
                let (result, carry) = overflowing_sub_sized(existing, value, size);
                match dest {
                    Target::DirectAReg(_) => { },
                    _ => self.set_compare_flags(result, size, carry, get_overflow(existing, value, result, size)),
                }
                self.set_target_value(system, dest, result, size)?;
            },
            Instruction::SWAP(reg) => {
                let value = self.state.d_reg[reg as usize];
                self.state.d_reg[reg as usize] = ((value & 0x0000FFFF) << 16) | ((value & 0xFFFF0000) >> 16);
            },
            //Instruction::TAS(Target) => {
            //},
            Instruction::TST(target, size) => {
                let value = self.get_target_value(system, target, size)?;
                self.set_logic_flags(value, size);
            },
            Instruction::TRAP(number) => {
                self.exception(system, 32 + number)?;
            },
            Instruction::TRAPV => {
                if self.get_flag(Flags::Overflow) {
                    self.exception(system, 7)?;
                }
            },
            Instruction::UNLK(reg) => {
                let value = *self.get_a_reg_mut(reg);
                *self.get_stack_pointer_mut() = value;
                let new_value = self.pop_long(system)?;
                let addr = self.get_a_reg_mut(reg);
                *addr = new_value;
            },
            _ => { return Err(Error::new("Unsupported instruction")); },
        }

        self.timer.execute.end(timer);
        Ok(())
    }

    fn push_word(&mut self, system: &System, value: u16) -> Result<(), Error> {
        let reg = self.get_stack_pointer_mut();
        *reg -= 2;
        system.get_bus().write_beu16(*reg as Address, value)
    }

    fn pop_word(&mut self, system: &System) -> Result<u16, Error> {
        let reg = self.get_stack_pointer_mut();
        let value = system.get_bus().read_beu16(*reg as Address)?;
        *reg += 2;
        Ok(value)
    }

    fn push_long(&mut self, system: &System, value: u32) -> Result<(), Error> {
        let reg = self.get_stack_pointer_mut();
        *reg -= 4;
        system.get_bus().write_beu32(*reg as Address, value)
    }

    fn pop_long(&mut self, system: &System) -> Result<u32, Error> {
        let reg = self.get_stack_pointer_mut();
        let value = system.get_bus().read_beu32(*reg as Address)?;
        *reg += 4;
        Ok(value)
    }

    pub fn get_target_value(&mut self, system: &System, target: Target, size: Size) -> Result<u32, Error> {
        match target {
            Target::Immediate(value) => Ok(value),
            Target::DirectDReg(reg) => Ok(get_value_sized(self.state.d_reg[reg as usize], size)),
            Target::DirectAReg(reg) => Ok(get_value_sized(*self.get_a_reg_mut(reg), size)),
            Target::IndirectAReg(reg) => get_address_sized(system, *self.get_a_reg_mut(reg) as Address, size),
            Target::IndirectARegInc(reg) => {
                let addr = self.get_a_reg_mut(reg);
                let result = get_address_sized(system, *addr as Address, size);
                *addr += size.in_bytes();
                result
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.get_a_reg_mut(reg);
                *addr -= size.in_bytes();
                get_address_sized(system, *addr as Address, size)
            },
            Target::IndirectARegOffset(reg, offset) => {
                let addr = self.get_a_reg_mut(reg);
                get_address_sized(system, (*addr).wrapping_add(offset as u32) as Address, size)
            },
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, target_size) => {
                let reg_offset = sign_extend_to_long(self.get_x_reg_value(rtype, xreg), target_size);
                let addr = self.get_a_reg_mut(reg);
                get_address_sized(system, (*addr).wrapping_add(reg_offset as u32).wrapping_add(offset as u32) as Address, size)
            },
            Target::IndirectMemory(addr) => {
                get_address_sized(system, addr as Address, size)
            },
            Target::IndirectPCOffset(offset) => {
                get_address_sized(system, (self.decoder.start + 2).wrapping_add(offset as u32) as Address, size)
            },
            Target::IndirectPCXRegOffset(rtype, xreg, offset, target_size) => {
                let reg_offset = sign_extend_to_long(self.get_x_reg_value(rtype, xreg), target_size);
                get_address_sized(system, (self.decoder.start + 2).wrapping_add(reg_offset as u32).wrapping_add(offset as u32) as Address, size)
            },
        }
    }

    pub fn set_target_value(&mut self, system: &System, target: Target, value: u32, size: Size) -> Result<(), Error> {
        match target {
            Target::DirectDReg(reg) => {
                set_value_sized(&mut self.state.d_reg[reg as usize], value, size);
            },
            Target::DirectAReg(reg) => {
                set_value_sized(self.get_a_reg_mut(reg), value, size);
            },
            Target::IndirectAReg(reg) => {
                set_address_sized(system, *self.get_a_reg_mut(reg) as Address, value, size)?;
            },
            Target::IndirectARegInc(reg) => {
                let addr = self.get_a_reg_mut(reg);
                set_address_sized(system, *addr as Address, value, size)?;
                *addr += size.in_bytes();
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.get_a_reg_mut(reg);
                *addr -= size.in_bytes();
                set_address_sized(system, *addr as Address, value, size)?;
            },
            Target::IndirectARegOffset(reg, offset) => {
                let addr = self.get_a_reg_mut(reg);
                set_address_sized(system, (*addr).wrapping_add(offset as u32) as Address, value, size)?;
            },
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, target_size) => {
                let reg_offset = sign_extend_to_long(self.get_x_reg_value(rtype, xreg), target_size);
                let addr = self.get_a_reg_mut(reg);
                set_address_sized(system, (*addr).wrapping_add(reg_offset as u32).wrapping_add(offset as u32) as Address, value, size)?;
            },
            Target::IndirectMemory(addr) => {
                set_address_sized(system, addr as Address, value, size)?;
            },
            _ => return Err(Error::new(&format!("Unimplemented addressing target: {:?}", target))),
        }
        Ok(())
    }

    pub fn get_target_address(&mut self, target: Target) -> Result<u32, Error> {
        let addr = match target {
            Target::IndirectAReg(reg) | Target::IndirectARegInc(reg) | Target::IndirectARegDec(reg) => *self.get_a_reg_mut(reg),
            Target::IndirectARegOffset(reg, offset) => {
                let addr = self.get_a_reg_mut(reg);
                (*addr).wrapping_add(offset as u32)
            },
            Target::IndirectARegXRegOffset(reg, rtype, xreg, offset, target_size) => {
                let reg_offset = sign_extend_to_long(self.get_x_reg_value(rtype, xreg), target_size);
                let addr = self.get_a_reg_mut(reg);
                (*addr).wrapping_add(reg_offset as u32).wrapping_add(offset as u32)
            },
            Target::IndirectMemory(addr) => {
                addr
            },
            Target::IndirectPCOffset(offset) => {
                (self.decoder.start + 2).wrapping_add(offset as u32)
            },
            Target::IndirectPCXRegOffset(rtype, xreg, offset, target_size) => {
                let reg_offset = sign_extend_to_long(self.get_x_reg_value(rtype, xreg), target_size);
                (self.decoder.start + 2).wrapping_add(reg_offset as u32).wrapping_add(offset as u32)
            },
            _ => return Err(Error::new(&format!("Invalid addressing target: {:?}", target))),
        };
        Ok(addr)
    }

    fn get_control_reg_mut(&mut self, control_reg: ControlRegister) -> &mut u32 {
        match control_reg {
            ControlRegister::VBR => &mut self.state.vbr,
        }
    }

    #[inline(always)]
    fn get_stack_pointer_mut(&mut self) -> &mut u32 {
        if self.is_supervisor() { &mut self.state.msp } else { &mut self.state.usp }
    }

    #[inline(always)]
    fn get_a_reg_mut(&mut self, reg: u8) -> &mut u32 {
        if reg == 7 {
            if self.is_supervisor() { &mut self.state.msp } else { &mut self.state.usp }
        } else {
            &mut self.state.a_reg[reg as usize]
        }
    }

    fn get_x_reg_value(&self, rtype: RegisterType, reg: u8) -> u32 {
        match rtype {
            RegisterType::Data => self.state.d_reg[reg as usize],
            RegisterType::Address => self.state.a_reg[reg as usize],
        }
    }

    fn is_supervisor(&self) -> bool {
        self.state.sr & (Flags:: Supervisor as u16) != 0
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

fn rotate_operation(value: u32, size: Size, dir: ShiftDirection) -> (u32, bool) {
    match dir {
        ShiftDirection::Left => {
            let bit = get_msb(value, size);
            let mask = if bit { 0x01 } else { 0x00 };
            match size {
                Size::Byte => (mask | ((value as u8) << 1) as u32, bit),
                Size::Word => (mask | ((value as u16) << 1) as u32, bit),
                Size::Long => (mask | (value << 1) as u32, bit),
            }
        },
        ShiftDirection::Right => {
            let bit = if (value & 0x01) != 0 { true } else { false };
            let mask = if bit { get_msb_mask(0xffffffff, size) } else { 0x0 };
            ((value >> 1) | mask, bit)
        },
    }
}


fn get_value_sized(value: u32, size: Size) -> u32 {
    match size {
        Size::Byte => { 0x000000FF & value },
        Size::Word => { 0x0000FFFF & value },
        Size::Long => { value },
    }
}

fn get_address_sized(system: &System, addr: Address, size: Size) -> Result<u32, Error> {
    match size {
        Size::Byte => system.get_bus().read_u8(addr).map(|value| value as u32),
        Size::Word => system.get_bus().read_beu16(addr).map(|value| value as u32),
        Size::Long => system.get_bus().read_beu32(addr),
    }
}

fn set_value_sized(addr: &mut u32, value: u32, size: Size) {
    match size {
        Size::Byte => { *addr = (*addr & 0xFFFFFF00) | (0x000000FF & value); }
        Size::Word => { *addr = (*addr & 0xFFFF0000) | (0x0000FFFF & value); }
        Size::Long => { *addr = value; }
    }
}

fn set_address_sized(system: &System, addr: Address, value: u32, size: Size) -> Result<(), Error> {
    match size {
        Size::Byte => system.get_bus().write_u8(addr, value as u8),
        Size::Word => system.get_bus().write_beu16(addr, value as u16),
        Size::Long => system.get_bus().write_beu32(addr, value),
    }
}

fn get_overflow(operand1: u32, operand2: u32, result: u32, size: Size) -> bool {
    let msb1 = get_msb(operand1, size);
    let msb2 = get_msb(operand2, size);
    let msb_res = get_msb(result, size);

    msb1 && msb2 && !msb_res
}

fn get_msb(value: u32, size: Size) -> bool {
    match size {
        Size::Byte => (value & 0x00000080) != 0,
        Size::Word => (value & 0x00008000) != 0,
        Size::Long => (value & 0x80000000) != 0,
    }
}

fn get_msb_mask(value: u32, size: Size) -> u32 {
    match size {
        Size::Byte => value & 0x00000080,
        Size::Word => value & 0x00008000,
        Size::Long => value & 0x80000000,
    }
}

