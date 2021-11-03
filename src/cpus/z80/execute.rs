
use crate::system::System;
use crate::error::{ErrorType, Error};
use crate::devices::{ClockElapsed, Address, Steppable, Addressable, Interruptable, Debuggable, Transmutable, read_beu16, write_beu16};

use super::decode::{Condition, Instruction, LoadTarget, Target, RegisterPair};
use super::state::{Z80, Status, Flags, Register};

impl Steppable for Z80 {
    fn step(&mut self, system: &System) -> Result<ClockElapsed, Error> {
        self.step_internal(system)?;
        Ok((1_000_000_000 / self.frequency as u64) * 4)
    }

    fn on_error(&mut self, system: &System) {
        //self.dump_state(system);
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

    //fn as_debuggable(&mut self) -> Option<&mut dyn Debuggable> {
    //    Some(self)
    //}
}



impl Z80 {
    pub fn step_internal(&mut self, system: &System) -> Result<(), Error> {
        match self.state.status {
            Status::Init => self.init(system),
            Status::Halted => Err(Error::new("CPU stopped")),
            Status::Running => {
                match self.cycle_one(system) {
                    Ok(()) => Ok(()),
                    //Err(Error { err: ErrorType::Processor, native, .. }) => {
                    Err(Error { err: ErrorType::Processor, native, .. }) => {
                        //self.exception(system, native as u8, false)?;
                        Ok(())
                    },
                    Err(err) => Err(err),
                }
            },
        }
    }

    pub fn init(&mut self, system: &System) -> Result<(), Error> {
        //self.state.msp = self.port.read_beu32(0)?;
        //self.state.pc = self.port.read_beu32(4)?;
        self.state.status = Status::Running;
        Ok(())
    }

    pub fn cycle_one(&mut self, system: &System) -> Result<(), Error> {
        //self.timer.cycle.start();
        self.decode_next(system)?;
        self.execute_current(system)?;
        //self.timer.cycle.end();

        //if (self.timer.cycle.events % 500) == 0 {
        //    println!("{}", self.timer);
        //}

        //self.check_pending_interrupts(system)?;
        Ok(())
    }

    pub fn decode_next(&mut self, system: &System) -> Result<(), Error> {
        //self.check_breakpoints(system);

        //self.timer.decode.start();
        self.decoder.decode_at(&mut self.port, self.state.pc)?;
        //self.timer.decode.end();

        //if self.debugger.use_tracing {
            self.decoder.dump_decoded(&mut self.port);
        //}

        self.state.pc = self.decoder.end;
        Ok(())
    }

    pub fn execute_current(&mut self, system: &System) -> Result<(), Error> {
        match self.decoder.instruction {
            //Instruction::ADCa(target) => {
            //},
            //Instruction::ADChl(regpair) => {
            //},
            //Instruction::ADDa(target) => {
            //},
            //Instruction::ADDhl(regpair) => {
            //},
            //Instruction::AND(target) => {
            //},
            //Instruction::CP(target) => {
            //},
            //Instruction::NEG => {
            //},
            Instruction::OR(target) => {
                let acc = self.get_register_value(Register::A);
                let value = self.get_target_value(target)?;
                let result = acc | value;
                self.set_register_value(Register::A, result);
                self.set_op_flags(result, false);
            },
            //Instruction::SBCa(target) => {
            //},
            //Instruction::SBChl(regpair) => {
            //},
            //Instruction::SUB(target) => {
            //},
            //Instruction::XOR(target) => {
            //},

            //Instruction::BIT(u8, target) => {
            //},
            //Instruction::RES(u8, target) => {
            //},
            //Instruction::RL(target) => {
            //},
            //Instruction::RLC(target) => {
            //},
            //Instruction::RR(target) => {
            //},
            //Instruction::RRC(target) => {
            //},
            //Instruction::SET(u8, target) => {
            //},
            //Instruction::SLA(target) => {
            //},
            //Instruction::SLL(target) => {
            //},
            //Instruction::SRA(target) => {
            //},
            //Instruction::SRL(target) => {
            //},

            Instruction::DEC8(target) => {
                let (result, overflow) = self.get_target_value(target)?.overflowing_sub(1);
                self.set_op_flags(result, false);
                self.set_target_value(target, result);
            },
            //Instruction::DEC16(regpair) => {
            //},
            //Instruction::INC8(target) => {
            //},
            //Instruction::INC16(regpair) => {
            //},

            //Instruction::EXX => {
            //},
            //Instruction::EXafaf => {
            //},
            //Instruction::EXhlsp => {
            //},
            //Instruction::EXhlde => {
            //},
            Instruction::LD(dest, src) => {
                let src_value = self.get_load_target_value(src)?;
                self.set_load_target_value(dest, src_value)?;
            },
            Instruction::POP(regpair) => {
                let value = self.pop_word()?;
                self.set_register_pair_value(regpair, value);
            },
            Instruction::PUSH(regpair) => {
                let value = self.get_register_pair_value(regpair);
                self.push_word(value)?;
            },

            //Instruction::INx(u8) => {
            //},
            //Instruction::INic(reg) => {
            //},
            Instruction::OUTx(port) => {
                // TODO this needs to be fixed
                println!("OUT: {:x}", self.state.reg[Register::A as usize]);
            },
            //Instruction::OUTic(reg) => {
            //},

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
            //Instruction::DJNZ(i8) => {
            //},
            Instruction::JP(addr) => {
                self.state.pc = addr;
            },
            //Instruction::JPIndirectHL => {
            //},
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
            Instruction::RET => {
                self.state.pc = self.pop_word()?;
            },
            //Instruction::RETI => {
            //},
            //Instruction::RETN => {
            //},
            //Instruction::RETcc(cond) => {
            //},

            //Instruction::DI => {
            //},
            //Instruction::EI => {
            //},
            //Instruction::IM(u8) => {
            //},
            Instruction::NOP => { },
            //Instruction::HALT => {
            //},
            //Instruction::RST(u8) => {
            //},

            //Instruction::CCF => {
            //},
            //Instruction::CPL => {
            //},
            //Instruction::DAA => {
            //},
            //Instruction::RLA => {
            //},
            //Instruction::RLCA => {
            //},
            //Instruction::RRA => {
            //},
            //Instruction::RRCA => {
            //},
            //Instruction::RRD => {
            //},
            //Instruction::RLD => {
            //},
            //Instruction::SCF => {
            //},

            //Instruction::CPD => {
            //},
            //Instruction::CPDR => {
            //},
            //Instruction::CPI => {
            //},
            //Instruction::CPIR => {
            //},
            //Instruction::IND => {
            //},
            //Instruction::INDR => {
            //},
            //Instruction::INI => {
            //},
            //Instruction::INIR => {
            //},
            //Instruction::LDD => {
            //},
            //Instruction::LDDR => {
            //},
            //Instruction::LDI => {
            //},
            Instruction::LDIR => {
                let src_value = self.get_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::HL))?;
                self.set_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::DE), src_value)?;
                self.register_pair_add_value(RegisterPair::DE, 1);
                self.register_pair_add_value(RegisterPair::HL, 1);
                let count = self.register_pair_add_value(RegisterPair::BC, -1);
                if count != 0 {
                    self.state.pc -= 2;
                }
            },
            //Instruction::OTDR => {
            //},
            //Instruction::OTIR => {
            //},
            //Instruction::OUTD => {
            //},
            //Instruction::OUTI => {
            //},

            _ => {
                panic!("unimplemented");
            }
        }

        Ok(())
    }

    fn push_word(&mut self, value: u16) -> Result<(), Error> {
        self.state.sp -= 1;
        self.port.write_u8(self.state.sp as Address, (value >> 8) as u8)?;
        self.state.sp -= 1;
        self.port.write_u8(self.state.sp as Address, (value & 0x00FF) as u8)?;
        Ok(())
    }

    fn pop_word(&mut self) -> Result<u16, Error> {
        let mut value = 0;
        value = self.port.read_u8(self.state.sp as Address)? as u16;
        self.state.sp += 1;
        value |= (self.port.read_u8(self.state.sp as Address)? as u16) << 8;
        self.state.sp += 1;
        Ok(value)
    }

    fn get_load_target_value(&mut self, target: LoadTarget) -> Result<u16, Error> {
        let value = match target {
            LoadTarget::DirectRegByte(reg) => self.get_register_value(reg) as u16,
            LoadTarget::DirectRegWord(regpair) => self.get_register_pair_value(regpair),
            LoadTarget::IndirectRegByte(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.read_u8(addr as Address)? as u16
            },
            LoadTarget::IndirectRegWord(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.read_leu16(addr as Address)?
            },
            //LoadTarget::DirectAltRegByte(reg),
            //LoadTarget::DirectSpecialRegByte(reg),
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
            LoadTarget::DirectRegWord(regpair) => self.set_register_pair_value(regpair, value),
            LoadTarget::IndirectRegByte(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.write_u8(addr as Address, value as u8)?;
            },
            LoadTarget::IndirectRegWord(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.write_leu16(addr as Address, value)?;
            },
            //LoadTarget::DirectAltRegByte(reg),
            //LoadTarget::DirectSpecialRegByte(reg),
            LoadTarget::IndirectByte(addr) => {
                self.port.write_u8(addr as Address, value as u8)?;
            },
            LoadTarget::IndirectWord(addr) => {
                self.port.write_leu16(addr as Address, value)?;
            },
            _ => panic!("Unsupported LoadTarget for set"),
        }
        Ok(())
    }

    fn get_target_value(&mut self, target: Target) -> Result<u8, Error> {
        match target {
            Target::DirectReg(reg) => Ok(self.get_register_value(reg)),
            Target::IndirectReg(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                Ok(self.port.read_u8(addr as Address)?)
            },
            Target::Immediate(data) => Ok(data),
        }
    }

    fn set_target_value(&mut self, target: Target, value: u8) -> Result<(), Error> {
        match target {
            Target::DirectReg(reg) => self.set_register_value(reg, value),
            Target::IndirectReg(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.port.write_u8(addr as Address, value)?;
            },
            _ => panic!("Unsupported LoadTarget for set"),
        }
        Ok(())
    }

    fn get_register_value(&mut self, reg: Register) -> u8 {
        let i = (reg as u8) as usize;
        self.state.reg[i]
    }

    fn set_register_value(&mut self, reg: Register, value: u8) {
        let i = (reg as u8) as usize;
        self.state.reg[i] = value;
    }

    fn get_register_pair_value(&mut self, regpair: RegisterPair) -> u16 {
        match regpair {
            RegisterPair::BC => read_beu16(&self.state.reg[0..2]),
            RegisterPair::DE => read_beu16(&self.state.reg[2..4]),
            RegisterPair::HL => read_beu16(&self.state.reg[4..6]),
            RegisterPair::AF => read_beu16(&self.state.reg[6..8]),
            RegisterPair::SP => self.state.sp,
        }
    }

    fn set_register_pair_value(&mut self, regpair: RegisterPair, value: u16) {
        match regpair {
            RegisterPair::BC => { write_beu16(&mut self.state.reg[0..2], value); },
            RegisterPair::DE => { write_beu16(&mut self.state.reg[2..4], value); },
            RegisterPair::HL => { write_beu16(&mut self.state.reg[4..6], value); },
            RegisterPair::AF => { write_beu16(&mut self.state.reg[6..8], value); },
            RegisterPair::SP => { self.state.sp = value; },
        }
    }

    fn register_pair_add_value(&mut self, regpair: RegisterPair, value: i16) -> u16 {
        let addr = match regpair {
            RegisterPair::BC => &mut self.state.reg[0..2],
            RegisterPair::DE => &mut self.state.reg[2..4],
            RegisterPair::HL => &mut self.state.reg[4..6],
            RegisterPair::AF => &mut self.state.reg[6..8],
            RegisterPair::SP => panic!("SP is not supported by inc/dec"),
        };

        let result = (read_beu16(addr) as i16).wrapping_add(value) as u16;
        write_beu16(addr, result);
        result
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

    fn set_op_flags(&mut self, value: u8, carry: bool) {
        let mut flags = 0;

        if value == 0 {
            flags |= Flags::Zero as u8;
        }
        if (value as i8) < 0 {
            flags |= Flags::Sign as u8;
        }
        if carry {
            flags |= Flags::Carry as u8;
        }
        self.state.reg[Register::F as usize] = flags;
    }

    #[inline(always)]
    fn get_flags(&self) -> u8 {
        self.state.reg[Register::F as usize]
    }

    #[inline(always)]
    fn get_flag(&self, flag: Flags) -> bool {
        self.get_flags() & (flag as u8) != 0
    }

}

