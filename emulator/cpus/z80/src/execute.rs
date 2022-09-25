
use moa_core::{System, Error, ErrorType, ClockElapsed, Address, Steppable, Addressable, Interruptable, Debuggable, Transmutable, read_beu16, write_beu16};

use crate::decode::{Condition, Instruction, LoadTarget, Target, RegisterPair, IndexRegister, SpecialRegister, IndexRegisterHalf, Size, Direction};
use crate::state::{Z80, Status, Flags, Register};


const DEV_NAME: &'static str = "z80-cpu";

const FLAGS_NUMERIC: u8                 = 0xC0;
const FLAGS_ARITHMETIC: u8              = 0x17;
const FLAGS_CARRY_HALF_CARRY: u8        = 0x11;


enum RotateType {
    Bit8,
    Bit9,
}


impl Steppable for Z80 {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        let clocks = if self.reset.get() {
//println!("RESET");
            self.reset()?
        } else if self.bus_request.get() {
//println!("BUS REQ");
            4
        } else {
//println!("RUNNING {:?}", self.decoder.instruction);
            self.step_internal(system)?
        };

        Ok((1_000_000_000 / self.frequency as ClockElapsed) * clocks as ClockElapsed)
    }

    fn on_error(&mut self, _system: &System) {
        self.dump_state();
    }
}

impl Interruptable for Z80 { }


impl Transmutable for Z80 {
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


impl Z80 {
    pub fn step_internal(&mut self, system: &System) -> Result<u16, Error> {
        match self.state.status {
            Status::Init => self.init(),
            Status::Halted => Err(Error::new("CPU stopped")),
            Status::Running => {
                match self.cycle_one(system) {
                    Ok(clocks) => Ok(clocks),
                    Err(Error { err: ErrorType::Processor, .. }) => {
                        //self.exception(system, native as u8, false)?;
                        Ok(4)
                    },
                    Err(err) => Err(err),
                }
            },
        }
    }

    pub fn init(&mut self) -> Result<u16, Error> {
        self.state.pc = 0;
        self.state.status = Status::Running;
        Ok(16)
    }

    pub fn reset(&mut self) -> Result<u16, Error> {
        self.clear_state();
        Ok(16)
    }

    pub fn cycle_one(&mut self, system: &System) -> Result<u16, Error> {
        self.decode_next()?;
        self.execute_current()?;
        //self.check_pending_interrupts(system)?;
        self.check_breakpoints(system);
        Ok(self.decoder.execution_time)
    }

    pub fn decode_next(&mut self) -> Result<(), Error> {
        self.decoder.decode_at(&mut self.port, self.state.pc)?;
        self.state.pc = self.decoder.end;
        Ok(())
    }

    pub fn execute_current(&mut self) -> Result<(), Error> {
        match self.decoder.instruction {
            Instruction::ADCa(target) => {
                let src = self.get_target_value(target)?;
                let acc = self.get_register_value(Register::A);

                let (result1, carry1, overflow1, half_carry1) = add_bytes(acc, self.get_flag(Flags::Carry) as u8);
                let (result2, carry2, overflow2, half_carry2) = add_bytes(result1, src);
                self.set_arithmetic_op_flags(result2 as u16, Size::Byte, false, carry1 | carry2, overflow1 | overflow2, half_carry1 | half_carry2);

                self.set_register_value(Register::A, result2);
            },
            Instruction::ADC16(dest_pair, src_pair) => {
                let src = self.get_register_pair_value(src_pair);
                let dest = self.get_register_pair_value(dest_pair);

                let (result1, carry1, overflow1, half_carry1) = add_words(dest, self.get_flag(Flags::Carry) as u16);
                let (result2, carry2, overflow2, half_carry2) = add_words(result1, src);
                self.set_arithmetic_op_flags(result2, Size::Word, false, carry1 | carry2, overflow1 | overflow2, half_carry1 | half_carry2);

                self.set_register_pair_value(dest_pair, result2);
            },
            Instruction::ADDa(target) => {
                let src = self.get_target_value(target)?;
                let acc = self.get_register_value(Register::A);

                let (result, carry, overflow, half_carry) = add_bytes(acc, src);
                self.set_arithmetic_op_flags(result as u16, Size::Byte, false, carry, overflow, half_carry);

                self.set_register_value(Register::A, result);
            },
            Instruction::ADD16(dest_pair, src_pair) => {
                let src = self.get_register_pair_value(src_pair);
                let dest = self.get_register_pair_value(dest_pair);

                let (result, carry, _, half_carry) = add_words(dest, src);
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::Carry, carry);
                self.set_flag(Flags::HalfCarry, half_carry);

                self.set_register_pair_value(dest_pair, result);
            },
            Instruction::AND(target) => {
                let acc = self.get_register_value(Register::A);
                let value = self.get_target_value(target)?;
                let result = acc & value;
                self.set_register_value(Register::A, result);
                self.set_logic_op_flags(result, false, true);
            },
            Instruction::BIT(bit, target) => {
                let value = self.get_target_value(target)?;
                let result = value & (1 << bit);
                self.set_flag(Flags::Zero, result == 0);
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::HalfCarry, true);
            },
            Instruction::CALL(addr) => {
                self.push_word(self.decoder.end)?;
                self.state.pc = addr;
            },
            Instruction::CALLcc(cond, addr) => {
                if self.get_current_condition(cond) {
                    self.push_word(self.decoder.end)?;
                    self.state.pc = addr;
                }
            },
            Instruction::CCF => {
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::HalfCarry, self.get_flag(Flags::Carry));
                self.set_flag(Flags::Carry, !self.get_flag(Flags::Carry));
            },
            Instruction::CP(target) => {
                let src = self.get_target_value(target)?;
                let acc = self.get_register_value(Register::A);

                let (result, carry, overflow) = sub_bytes(acc, src);
                self.set_arithmetic_op_flags(result as u16, Size::Byte, true, carry, overflow, (result & 0x10) != 0);
            },
            //Instruction::CPD => {
            //},
            //Instruction::CPDR => {
            //},
            //Instruction::CPI => {
            //},
            //Instruction::CPIR => {
            //},
            Instruction::CPL => {
                let value = self.get_register_value(Register::A);
                self.set_register_value(Register::A, !value);
                self.set_flag(Flags::HalfCarry, true);
                self.set_flag(Flags::AddSubtract, true);
            },
            //Instruction::DAA => {
            //},
            Instruction::DEC16(regpair) => {
                let value = self.get_register_pair_value(regpair);

                let (result, _, _) = sub_words(value, 1);

                self.set_register_pair_value(regpair, result);
            },
            Instruction::DEC8(target) => {
                let value = self.get_target_value(target)?;

                let (result, _, overflow) = sub_bytes(value, 1);
                let carry = self.get_flag(Flags::Carry);        // Preserve the carry bit, according to Z80 reference
                self.set_arithmetic_op_flags(result as u16, Size::Byte, true, carry, overflow, (result & 0x10) != 0);

                self.set_target_value(target, result)?;
            },
            Instruction::DI => {
                self.state.interrupts_enabled = false;
            },
            Instruction::DJNZ(offset) => {
                let value = self.get_register_value(Register::B);
                let result = value.wrapping_sub(1);
                self.set_register_value(Register::B, result);

                if result != 0 {
                    self.state.pc = ((self.state.pc as i16) + (offset as i16)) as u16;
                }
            },
            Instruction::EI => {
                self.state.interrupts_enabled = true;
            },
            Instruction::EXX => {
                for i in 0..6 {
                    let (normal, shadow) = (self.state.reg[i], self.state.shadow_reg[i]);
                    self.state.reg[i] = shadow;
                    self.state.shadow_reg[i] = normal;
                }
            },
            Instruction::EXafaf => {
                for i in 6..8 {
                    let (normal, shadow) = (self.state.reg[i], self.state.shadow_reg[i]);
                    self.state.reg[i] = shadow;
                    self.state.shadow_reg[i] = normal;
                }
            },
            Instruction::EXhlde => {
                let (hl, de) = (self.get_register_pair_value(RegisterPair::HL), self.get_register_pair_value(RegisterPair::DE));
                self.set_register_pair_value(RegisterPair::DE, hl);
                self.set_register_pair_value(RegisterPair::HL, de);
            },
            Instruction::EXsp(regpair) => {
                let reg_value = self.get_register_pair_value(regpair);
                let sp = self.get_register_pair_value(RegisterPair::SP);
                let sp_value = self.port.read_leu16(sp as Address)?;
                self.set_register_pair_value(regpair, sp_value);
                self.port.write_leu16(sp as Address, reg_value)?;
            },
            Instruction::HALT => {
                self.state.status = Status::Halted;
            },
            Instruction::IM(mode) => {
                self.state.interrupt_mode = mode;
            },
            Instruction::INC16(regpair) => {
                let value = self.get_register_pair_value(regpair);

                let (result, _, _, _) = add_words(value, 1);

                self.set_register_pair_value(regpair, result);
            },
            Instruction::INC8(target) => {
                let value = self.get_target_value(target)?;

                let (result, _, overflow, _) = add_bytes(value, 1);
                let carry = self.get_flag(Flags::Carry);        // Preserve the carry bit, according to Z80 reference
                self.set_arithmetic_op_flags(result as u16, Size::Byte, false, carry, overflow, (result & 0x10) != 0);

                self.set_target_value(target, result)?;
            },
            //Instruction::IND => {
            //},
            //Instruction::INDR => {
            //},
            //Instruction::INI => {
            //},
            //Instruction::INIR => {
            //},
            //Instruction::INic(reg) => {
            //},
            //Instruction::INx(u8) => {
            //},
            Instruction::JP(addr) => {
                self.state.pc = addr;
            },
            Instruction::JPIndirect(regpair) => {
                let value = self.get_register_pair_value(regpair);
                //let addr = self.port.read_leu16(value as Address)?;
                self.state.pc = value;
            },
            Instruction::JPcc(cond, addr) => {
                if self.get_current_condition(cond) {
                    self.state.pc = addr;
                }
            },
            Instruction::JR(offset) => {
                self.state.pc = ((self.state.pc as i16) + (offset as i16)) as u16;
            },
            Instruction::JRcc(cond, offset) => {
                if self.get_current_condition(cond) {
                    self.state.pc = ((self.state.pc as i16) + (offset as i16)) as u16;
                }
            },
            Instruction::LD(dest, src) => {
                let src_value = self.get_load_target_value(src)?;
                self.set_load_target_value(dest, src_value)?;
            },
            Instruction::LDsr(special_reg, dir) => {
                let addr = match special_reg {
                    SpecialRegister::I => &mut self.state.i,
                    SpecialRegister::R => &mut self.state.r,
                };

                match dir {
                    Direction::FromAcc => { *addr = self.state.reg[Register::A as usize]; },
                    Direction::ToAcc => { self.state.reg[Register::A as usize] = *addr; },
                }
            }
            Instruction::LDD | Instruction::LDDR | Instruction::LDI | Instruction::LDIR => {
                let diff = if self.decoder.instruction == Instruction::LDI || self.decoder.instruction == Instruction::LDIR {
                    1
                } else {
                    -1
                };

                let src_value = self.get_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::HL))?;
                self.set_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::DE), src_value)?;
                self.add_to_regpair(RegisterPair::DE, diff);
                self.add_to_regpair(RegisterPair::HL, diff);
                let count = self.add_to_regpair(RegisterPair::BC, -1);
                let mask = (Flags::AddSubtract as u8) | (Flags::HalfCarry as u8) | (Flags::Parity as u8);
                let parity = if count != 0 { Flags::Parity as u8 } else { 0 };
                self.set_flags(mask, parity);

                if self.decoder.instruction == Instruction::LDIR || self.decoder.instruction == Instruction::LDDR {
                    if count != 0 {
                        self.state.pc -= 2;
                    }
                }
            },
            Instruction::NEG => {
                let acc = self.get_register_value(Register::A);

                let (result, carry, overflow) = sub_bytes(0, acc);
                self.set_arithmetic_op_flags(result as u16, Size::Byte, true, carry, overflow, (acc & 0x10) != 0 && (result & 0x10) == 0);

                self.set_register_value(Register::A, result);
            },
            Instruction::NOP => { },
            Instruction::OR(target) => {
                let acc = self.get_register_value(Register::A);
                let value = self.get_target_value(target)?;
                let result = acc | value;
                self.set_register_value(Register::A, result);
                self.set_logic_op_flags(result, false, false);
            },
            //Instruction::OTDR => {
            //},
            //Instruction::OTIR => {
            //},
            //Instruction::OUTD => {
            //},
            //Instruction::OUTI => {
            //},
            //Instruction::OUTic(reg) => {
            //},
            Instruction::OUTx(port) => {
                // TODO this needs to be fixed
                println!("OUT ({:x}), {:x} {}", port, self.state.reg[Register::A as usize], self.state.reg[Register::A as usize] as char);
            },
            Instruction::POP(regpair) => {
                let value = self.pop_word()?;
                self.set_register_pair_value(regpair, value);
            },
            Instruction::PUSH(regpair) => {
                let value = self.get_register_pair_value(regpair);
                self.push_word(value)?;
            },
            Instruction::RES(bit, target, opt_copy) => {
                let mut value = self.get_target_value(target)?;
                value = value & !(1 << bit);
                self.set_target_value(target, value)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::RET => {
                self.state.pc = self.pop_word()?;
            },
            //Instruction::RETI => {
            //},
            //Instruction::RETN => {
            //},
            Instruction::RETcc(cond) => {
                if self.get_current_condition(cond) {
                    self.state.pc = self.pop_word()?;
                }
            },
            Instruction::RL(target, opt_copy) => {
                let value = self.get_target_value(target)?;
                let (result, out_bit) = self.rotate_left(value, RotateType::Bit9);
                self.set_logic_op_flags(result, out_bit, false);
                self.set_target_value(target, result)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::RLA => {
                let value = self.get_register_value(Register::A);
                let (result, out_bit) = self.rotate_left(value, RotateType::Bit9);
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::HalfCarry, false);
                self.set_flag(Flags::Carry, out_bit);
                self.set_register_value(Register::A, result);
            },
            Instruction::RLC(target, opt_copy) => {
                let value = self.get_target_value(target)?;
                let (result, out_bit) = self.rotate_left(value, RotateType::Bit8);
                self.set_logic_op_flags(result, out_bit, false);
                self.set_target_value(target, result)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::RLCA => {
                let value = self.get_register_value(Register::A);
                let (result, out_bit) = self.rotate_left(value, RotateType::Bit8);
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::HalfCarry, false);
                self.set_flag(Flags::Carry, out_bit);
                self.set_register_value(Register::A, result);
            },
            //Instruction::RLD => {
            //},
            Instruction::RR(target, opt_copy) => {
                let value = self.get_target_value(target)?;
                let (result, out_bit) = self.rotate_right(value, RotateType::Bit9);
                self.set_logic_op_flags(result, out_bit, false);
                self.set_target_value(target, result)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::RRA => {
                let value = self.get_register_value(Register::A);
                let (result, out_bit) = self.rotate_right(value, RotateType::Bit9);
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::HalfCarry, false);
                self.set_flag(Flags::Carry, out_bit);
                self.set_register_value(Register::A, result);
            },
            Instruction::RRC(target, opt_copy) => {
                let value = self.get_target_value(target)?;
                let (result, out_bit) = self.rotate_right(value, RotateType::Bit8);
                self.set_logic_op_flags(result, out_bit, false);
                self.set_target_value(target, result)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::RRCA => {
                let value = self.get_register_value(Register::A);
                let (result, out_bit) = self.rotate_right(value, RotateType::Bit8);
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::HalfCarry, false);
                self.set_flag(Flags::Carry, out_bit);
                self.set_register_value(Register::A, result);
            },
            //Instruction::RRD => {
            //},
            Instruction::RST(addr) => {
                self.push_word(self.decoder.end)?;
                self.state.pc = addr as u16;
            },
            Instruction::SBCa(target) => {
                let src = self.get_target_value(target)?;
                let acc = self.get_register_value(Register::A);

                let (result1, carry1, overflow1) = sub_bytes(acc, self.get_flag(Flags::Carry) as u8);
                let (result2, carry2, overflow2) = sub_bytes(result1, src);
                self.set_arithmetic_op_flags(result2 as u16, Size::Byte, true, carry1 | carry2, overflow1 | overflow2, (result2 & 0x10) != 0);

                self.set_register_value(Register::A, result2);
            },
            Instruction::SBC16(dest_pair, src_pair) => {
                let src = self.get_register_pair_value(src_pair);
                let dest = self.get_register_pair_value(dest_pair);

                let (result1, carry1, overflow1) = sub_words(dest, self.get_flag(Flags::Carry) as u16);
                let (result2, carry2, overflow2) = sub_words(result1, src);
                self.set_arithmetic_op_flags(result2, Size::Word, true, carry1 | carry2, overflow1 | overflow2, (result2 & 0x10) != 0);

                self.set_register_pair_value(dest_pair, result2);
            },
            Instruction::SCF => {
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::HalfCarry, false);
                self.set_flag(Flags::Carry, true);
            },
            Instruction::SET(bit, target, opt_copy) => {
                let mut value = self.get_target_value(target)?;
                value = value | (1 << bit);
                self.set_target_value(target, value)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::SLA(target, opt_copy) => {
                let value = self.get_target_value(target)?;
                let out_bit = get_msb(value as u16, Size::Byte);
                let result = value << 1;
                self.set_logic_op_flags(result, out_bit, false);
                self.set_target_value(target, result)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::SLL(target, opt_copy) => {
                let value = self.get_target_value(target)?;
                let out_bit = get_msb(value as u16, Size::Byte);
                let result = (value << 1) | 0x01;
                self.set_logic_op_flags(result, out_bit, false);
                self.set_target_value(target, result)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::SRA(target, opt_copy) => {
                let value = self.get_target_value(target)?;
                let out_bit = (value & 0x01) != 0;
                let msb_mask = if get_msb(value as u16, Size::Byte) { 0x80 } else { 0 };
                let result = (value >> 1) | msb_mask;
                self.set_logic_op_flags(result, out_bit, false);
                self.set_target_value(target, result)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::SRL(target, opt_copy) => {
                let value = self.get_target_value(target)?;
                let out_bit = (value & 0x01) != 0;
                let result = value >> 1;
                self.set_logic_op_flags(result, out_bit, false);
                self.set_target_value(target, result)?;
                if let Some(target) = opt_copy {
                    self.set_target_value(target, value)?;
                }
            },
            Instruction::SUB(target) => {
                let src = self.get_target_value(target)?;
                let acc = self.get_register_value(Register::A);

                let (result, carry, overflow) = sub_bytes(acc, src);
                self.set_arithmetic_op_flags(result as u16, Size::Byte, true, carry, overflow, (result & 0x10) != 0);

                self.set_register_value(Register::A, result);
            },
            Instruction::XOR(target) => {
                let acc = self.get_register_value(Register::A);
                let value = self.get_target_value(target)?;
                let result = acc ^ value;
                self.set_register_value(Register::A, result);
                self.set_logic_op_flags(result, false, false);
            },
            _ => {
                return Err(Error::new(&format!("{}: unimplemented instruction: {:?}", DEV_NAME, self.decoder.instruction)));
            }
        }

        Ok(())
    }

    fn rotate_left(&mut self, mut value: u8, rtype: RotateType) -> (u8, bool) {
        let out_bit = get_msb(value as u16, Size::Byte);

        let in_bit = match rtype {
            RotateType::Bit9 => self.get_flag(Flags::Carry),
            RotateType::Bit8 => out_bit,
        };

        value <<= 1;
        value |= in_bit as u8;
        (value, out_bit)
    }

    fn rotate_right(&mut self, mut value: u8, rtype: RotateType) -> (u8, bool) {
        let out_bit = (value & 0x01) != 0;

        let in_bit = match rtype {
            RotateType::Bit9 => self.get_flag(Flags::Carry),
            RotateType::Bit8 => out_bit,
        };

        value >>= 1;
        value |= if in_bit { 0x80 } else { 0 };
        (value, out_bit)
    }

    fn add_to_regpair(&mut self, regpair: RegisterPair, value: i16) -> u16 {
        let addr = match regpair {
            RegisterPair::BC => &mut self.state.reg[0..2],
            RegisterPair::DE => &mut self.state.reg[2..4],
            RegisterPair::HL => &mut self.state.reg[4..6],
            RegisterPair::AF => &mut self.state.reg[6..8],
            _ => panic!("RegPair is not supported by inc/dec"),
        };

        let result = (read_beu16(addr) as i16).wrapping_add(value) as u16;
        write_beu16(addr, result);
        result
    }



    fn push_word(&mut self, value: u16) -> Result<(), Error> {
        self.state.sp = self.state.sp.wrapping_sub(1);
        self.port.write_u8(self.state.sp as Address, (value >> 8) as u8)?;
        self.state.sp = self.state.sp.wrapping_sub(1);
        self.port.write_u8(self.state.sp as Address, (value & 0x00FF) as u8)?;
        Ok(())
    }

    fn pop_word(&mut self) -> Result<u16, Error> {
        let mut value;
        value = self.port.read_u8(self.state.sp as Address)? as u16;
        self.state.sp = self.state.sp.wrapping_add(1);
        value |= (self.port.read_u8(self.state.sp as Address)? as u16) << 8;
        self.state.sp = self.state.sp.wrapping_add(1);
        Ok(value)
    }

    fn get_load_target_value(&mut self, target: LoadTarget) -> Result<u16, Error> {
        let value = match target {
            LoadTarget::DirectRegByte(reg) => self.get_register_value(reg) as u16,
            LoadTarget::DirectRegHalfByte(reg) => self.get_index_register_half_value(reg) as u16,
            LoadTarget::DirectRegWord(regpair) => self.get_register_pair_value(regpair),
            LoadTarget::IndirectRegByte(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.read_u8(addr as Address)? as u16
            },
            LoadTarget::IndirectOffsetByte(index_reg, offset) => {
                let addr = self.get_index_register_value(index_reg);
                self.port.read_u8(((addr as i16).wrapping_add(offset as i16)) as Address)? as u16
            },
            LoadTarget::IndirectRegWord(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.read_leu16(addr as Address)?
            },
            LoadTarget::IndirectByte(addr) => {
                self.port.read_u8(addr as Address)? as u16
            },
            LoadTarget::IndirectWord(addr) => {
                self.port.read_leu16(addr as Address)?
            },
            LoadTarget::ImmediateByte(data) => data as u16,
            LoadTarget::ImmediateWord(data) => data,
            _ => panic!("Unsupported LoadTarget for set"),
        };
        Ok(value)
    }

    fn set_load_target_value(&mut self, target: LoadTarget, value: u16) -> Result<(), Error> {
        match target {
            LoadTarget::DirectRegByte(reg) => self.set_register_value(reg, value as u8),
            LoadTarget::DirectRegHalfByte(reg) => self.set_index_register_half_value(reg, value as u8),
            LoadTarget::DirectRegWord(regpair) => self.set_register_pair_value(regpair, value),
            LoadTarget::IndirectRegByte(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.write_u8(addr as Address, value as u8)?;
            },
            LoadTarget::IndirectOffsetByte(index_reg, offset) => {
                let addr = self.get_index_register_value(index_reg);
                self.port.write_u8(((addr as i16).wrapping_add(offset as i16)) as Address, value as u8)?;
            },
            LoadTarget::IndirectRegWord(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.write_leu16(addr as Address, value)?;
            },
            LoadTarget::IndirectByte(addr) => {
                self.port.write_u8(addr as Address, value as u8)?;
            },
            LoadTarget::IndirectWord(addr) => {
                self.port.write_leu16(addr as Address, value)?;
            },
            _ => panic!("Unsupported LoadTarget for set: {:?}", target),
        }
        Ok(())
    }

    fn get_target_value(&mut self, target: Target) -> Result<u8, Error> {
        match target {
            Target::DirectReg(reg) => Ok(self.get_register_value(reg)),
            Target::DirectRegHalf(reg) => Ok(self.get_index_register_half_value(reg)),
            Target::IndirectReg(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                Ok(self.port.read_u8(addr as Address)?)
            },
            Target::IndirectOffset(reg, offset) => {
                let addr = (self.get_index_register_value(reg) as i16) + (offset as i16);
                Ok(self.port.read_u8(addr as Address)?)
            },
            Target::Immediate(data) => Ok(data),
        }
    }

    fn set_target_value(&mut self, target: Target, value: u8) -> Result<(), Error> {
        match target {
            Target::DirectReg(reg) => self.set_register_value(reg, value),
            Target::DirectRegHalf(reg) => self.set_index_register_half_value(reg, value),
            Target::IndirectReg(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.write_u8(addr as Address, value)?;
            },
            Target::IndirectOffset(reg, offset) => {
                let addr = (self.get_index_register_value(reg) as i16) + (offset as i16);
                self.port.write_u8(addr as Address, value)?;
            },
            _ => panic!("Unsupported LoadTarget for set"),
        }
        Ok(())
    }


    fn get_register_value(&mut self, reg: Register) -> u8 {
        self.state.reg[reg as usize]
    }

    fn set_register_value(&mut self, reg: Register, value: u8) {
        self.state.reg[reg as usize] = value;
    }

    fn get_index_register_half_value(&mut self, reg: IndexRegisterHalf) -> u8 {
        match reg {
            IndexRegisterHalf::IXH => (self.state.ix >> 8) as u8,
            IndexRegisterHalf::IXL => (self.state.ix & 0x00FF) as u8,
            IndexRegisterHalf::IYH => (self.state.iy >> 8) as u8,
            IndexRegisterHalf::IYL => (self.state.iy & 0x00FF) as u8,
        }
    }

    fn set_index_register_half_value(&mut self, reg: IndexRegisterHalf, value: u8) {
        match reg {
            IndexRegisterHalf::IXH => { self.state.ix |= (value as u16) << 8; },
            IndexRegisterHalf::IXL => { self.state.ix |= value as u16; },
            IndexRegisterHalf::IYH => { self.state.iy |= (value as u16) << 8; },
            IndexRegisterHalf::IYL => { self.state.iy |= value as u16; },
        }
    }

    fn get_register_pair_value(&mut self, regpair: RegisterPair) -> u16 {
        match regpair {
            RegisterPair::BC => read_beu16(&self.state.reg[0..2]),
            RegisterPair::DE => read_beu16(&self.state.reg[2..4]),
            RegisterPair::HL => read_beu16(&self.state.reg[4..6]),
            RegisterPair::AF => read_beu16(&self.state.reg[6..8]),
            RegisterPair::SP => self.state.sp,
            RegisterPair::IX => self.state.ix,
            RegisterPair::IY => self.state.iy,
        }
    }

    fn set_register_pair_value(&mut self, regpair: RegisterPair, value: u16) {
        match regpair {
            RegisterPair::BC => { write_beu16(&mut self.state.reg[0..2], value); },
            RegisterPair::DE => { write_beu16(&mut self.state.reg[2..4], value); },
            RegisterPair::HL => { write_beu16(&mut self.state.reg[4..6], value); },
            RegisterPair::AF => { write_beu16(&mut self.state.reg[6..8], value); },
            RegisterPair::SP => { self.state.sp = value; },
            RegisterPair::IX => { self.state.ix = value; },
            RegisterPair::IY => { self.state.iy = value; },
        }
    }

    fn get_index_register_value(&mut self, reg: IndexRegister) -> u16 {
        match reg {
            IndexRegister::IX => self.state.ix,
            IndexRegister::IY => self.state.iy,
        }
    }

    fn get_current_condition(&mut self, cond: Condition) -> bool {
        match cond {
            Condition::NotZero => !self.get_flag(Flags::Zero),
            Condition::Zero => self.get_flag(Flags::Zero),
            Condition::NotCarry => !self.get_flag(Flags::Carry),
            Condition::Carry => self.get_flag(Flags::Carry),
            Condition::ParityOdd => !self.get_flag(Flags::Parity),
            Condition::ParityEven => self.get_flag(Flags::Parity),
            Condition::Positive => !self.get_flag(Flags::Sign),
            Condition::Negative => self.get_flag(Flags::Sign),
        }
    }

    fn set_numeric_flags(&mut self, value: u16, size: Size) {
        let sign = if get_msb(value, size) { Flags::Sign as u8 } else { 0 };
        let zero = if value == 0 { Flags::Zero as u8 } else { 0 };
        self.set_flags(FLAGS_NUMERIC, sign | zero);
    }

    fn set_parity_flags(&mut self, value: u8) {
        let mask = (Flags::Parity as u8) | (Flags::AddSubtract as u8);
        let parity = if (value.count_ones() & 0x01) == 0 { Flags::Parity as u8 } else { 0 };
        self.set_flags(mask, parity);
    }

    fn set_arithmetic_op_flags(&mut self, value: u16, size: Size, addsub: bool, carry: bool, overflow: bool, half_carry: bool) {
        self.state.reg[Register::F as usize] = 0;
        self.set_numeric_flags(value, size);

        let addsub_flag = if addsub { Flags::AddSubtract as u8 } else { 0 };
        let overflow_flag = if overflow { Flags::Parity as u8 } else { 0 };
        let carry_flag = if carry { Flags::Carry as u8 } else { 0 };
        let half_carry_flag = if half_carry { Flags::HalfCarry as u8 } else { 0 };
        self.set_flags(FLAGS_ARITHMETIC, addsub_flag | overflow_flag | carry_flag | half_carry_flag);
    }

    fn set_logic_op_flags(&mut self, value: u8, carry: bool, half_carry: bool) {
        self.state.reg[Register::F as usize] = 0;
        self.set_numeric_flags(value as u16, Size::Byte);
        self.set_parity_flags(value);

        let carry_flag = if carry { Flags::Carry as u8 } else { 0 };
        let half_carry_flag = if half_carry { Flags::HalfCarry as u8 } else { 0 };
        self.set_flags(FLAGS_CARRY_HALF_CARRY, carry_flag | half_carry_flag);
    }

    #[inline(always)]
    fn get_flag(&self, flag: Flags) -> bool {
        self.get_flags() & (flag as u8) != 0
    }

    #[inline(always)]
    fn set_flag(&mut self, flag: Flags, value: bool) {
        self.state.reg[Register::F as usize] = self.state.reg[Register::F as usize] & !(flag as u8);
        if value {
            self.state.reg[Register::F as usize] |= flag as u8;
        }
    }

    #[inline(always)]
    fn get_flags(&self) -> u8 {
        self.state.reg[Register::F as usize]
    }

    #[inline(always)]
    fn set_flags(&mut self, mask: u8, values: u8) {
        self.state.reg[Register::F as usize] = (self.state.reg[Register::F as usize] & !mask) | values;
    }
}

fn add_bytes(operand1: u8, operand2: u8) -> (u8, bool, bool, bool) {
    let (result, carry) = operand1.overflowing_add(operand2);
    let (_, overflow) = (operand1 as i8).overflowing_add(operand2 as i8);
    let half_carry = (operand1 & 0x10) != 0 && (result & 0x10) == 0;
    (result, carry, overflow, half_carry)
}

fn add_words(operand1: u16, operand2: u16) -> (u16, bool, bool, bool) {
    let (result, carry) = operand1.overflowing_add(operand2);
    let (_, overflow) = ((operand1 as i8) as i16).overflowing_add((operand2 as i8) as i16);
    let half_carry = (operand1 & 0x10) != 0 && (result & 0x10) == 0;
    (result, carry, overflow, half_carry)
}

fn sub_bytes(operand1: u8, operand2: u8) -> (u8, bool, bool) {
    let (result, carry) = operand1.overflowing_sub(operand2);
    let (_, overflow) = (operand1 as i8).overflowing_sub(operand2 as i8);
    (result, carry, overflow)
}

fn sub_words(operand1: u16, operand2: u16) -> (u16, bool, bool) {
    let (result, carry) = operand1.overflowing_sub(operand2);
    let (_, overflow) = ((operand1 as i8) as i16).overflowing_sub((operand2 as i8) as i16);
    (result, carry, overflow)
}

#[inline(always)]
fn get_msb(value: u16, size: Size) -> bool {
    match size {
        Size::Byte => (value & 0x0080) != 0,
        Size::Word => (value & 0x8000) != 0,
    }
}

