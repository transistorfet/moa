use femtos::{Instant, Duration};

use moa_core::{System, Error, Address, Steppable, Addressable, Interruptable, Debuggable, Transmutable, read_beu16, write_beu16};

use crate::instructions::{
    Condition, Instruction, LoadTarget, Target, Register, InterruptMode, RegisterPair, IndexRegister, SpecialRegister,
    IndexRegisterHalf, Size, Direction, UndocumentedCopy,
};
use crate::state::{Z80, Z80Error, Status, Flags};
use crate::timing::Z80InstructionCycles;


const FLAGS_NUMERIC: u8 = 0xC0;
const FLAGS_ARITHMETIC: u8 = 0x17;
const FLAGS_CARRY_HALF_CARRY: u8 = 0x11;


enum RotateType {
    Bit8,
    Bit9,
}

impl Steppable for Z80 {
    fn step(&mut self, system: &System) -> Result<Duration, Error> {
        let clocks = if self.reset.get() {
            self.reset()?
        } else if self.bus_request.get() {
            4
        } else {
            self.step_internal(system)?
        };

        Ok(self.frequency.period_duration() * clocks as u64)
    }

    fn on_error(&mut self, system: &System) {
        self.dump_state(system.clock);
    }
}

impl Interruptable for Z80 {}


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

impl From<Z80Error> for Error {
    fn from(err: Z80Error) -> Self {
        match err {
            Z80Error::Halted => Self::Other("cpu halted".to_string()),
            Z80Error::Breakpoint => Self::Breakpoint("breakpoint".to_string()),
            Z80Error::Unimplemented(instruction) => Self::new(format!("unimplemented instruction {:?}", instruction)),
            Z80Error::BusError(msg) => Self::Other(msg),
        }
    }
}

impl From<Error> for Z80Error {
    fn from(err: Error) -> Self {
        match err {
            Error::Processor(ex) => Z80Error::BusError(format!("processor error {}", ex)),
            Error::Breakpoint(_) => Z80Error::Breakpoint,
            Error::Other(msg) | Error::Assertion(msg) | Error::Emulator(_, msg) => Z80Error::BusError(msg),
        }
    }
}
#[derive(Clone)]
pub struct Z80Executor {
    pub current_clock: Instant,
    pub took_branch: bool,
}

impl Z80Executor {
    pub fn at_time(current_clock: Instant) -> Self {
        Self {
            current_clock,
            took_branch: false,
        }
    }
}

impl Z80 {
    pub fn step_internal(&mut self, system: &System) -> Result<u16, Z80Error> {
        self.executor = Z80Executor::at_time(system.clock);
        match self.state.status {
            Status::Init => self.init(),
            Status::Halted => Err(Z80Error::Halted),
            Status::Running => match self.cycle_one() {
                Ok(clocks) => Ok(clocks),
                Err(err) => Err(err),
            },
        }
    }

    pub fn init(&mut self) -> Result<u16, Z80Error> {
        self.state.pc = 0;
        self.state.status = Status::Running;
        Ok(16)
    }

    pub fn reset(&mut self) -> Result<u16, Z80Error> {
        self.clear_state();
        Ok(16)
    }

    pub fn cycle_one(&mut self) -> Result<u16, Z80Error> {
        self.check_breakpoints()?;

        self.decode_next()?;
        self.execute_current()?;
        Ok(
            Z80InstructionCycles::from_instruction(&self.decoder.instruction, self.decoder.extra_instruction_bytes)?
                .calculate_cycles(self.executor.took_branch),
        )
    }

    pub fn decode_next(&mut self) -> Result<(), Z80Error> {
        self.decoder
            .decode_at(&mut self.port, self.executor.current_clock, self.state.pc)?;
        self.increment_refresh(self.decoder.end.saturating_sub(self.decoder.start) as u8);
        self.state.pc = self.decoder.end;
        Ok(())
    }

    pub fn execute_current(&mut self) -> Result<(), Z80Error> {
        match self.decoder.instruction {
            Instruction::ADCa(target) => self.execute_adca(target),
            Instruction::ADC16(dest_pair, src_pair) => self.execute_adc16(dest_pair, src_pair),
            Instruction::ADDa(target) => self.execute_adda(target),
            Instruction::ADD16(dest_pair, src_pair) => self.execute_add16(dest_pair, src_pair),
            Instruction::AND(target) => self.execute_and(target),
            Instruction::BIT(bit, target) => self.execute_bit(bit, target),
            Instruction::CALL(addr) => self.execute_call(addr),
            Instruction::CALLcc(cond, addr) => self.execute_callcc(cond, addr),
            Instruction::CCF => self.execute_ccf(),
            Instruction::CP(target) => self.execute_cp(target),
            //Instruction::CPD => {
            //},
            //Instruction::CPDR => {
            //},
            //Instruction::CPI => {
            //},
            //Instruction::CPIR => {
            //},
            Instruction::CPL => self.execute_cpl(),
            Instruction::DAA => self.execute_daa(),
            Instruction::DEC16(regpair) => self.execute_dec16(regpair),
            Instruction::DEC8(target) => self.execute_dec8(target),
            Instruction::DI => self.execute_di(),
            Instruction::DJNZ(offset) => self.execute_djnz(offset),
            Instruction::EI => self.execute_ei(),
            Instruction::EXX => self.execute_exx(),
            Instruction::EXafaf => self.execute_ex_af_af(),
            Instruction::EXhlde => self.execute_ex_hl_de(),
            Instruction::EXsp(regpair) => self.execute_ex_sp(regpair),
            Instruction::HALT => self.execute_halt(),
            Instruction::IM(mode) => self.execute_im(mode),
            Instruction::INC16(regpair) => self.execute_inc16(regpair),
            Instruction::INC8(target) => self.execute_inc8(target),
            //Instruction::IND => {
            //},
            //Instruction::INDR => {
            //},
            Instruction::INI => self.execute_ini(),
            //Instruction::INIR => {
            //},
            Instruction::INic(reg) => self.execute_inic(reg),
            //Instruction::INicz => {
            //},
            Instruction::INx(n) => self.execute_inx(n),
            Instruction::JP(addr) => self.execute_jp(addr),
            Instruction::JPIndirect(regpair) => self.execute_jp_indirect(regpair),
            Instruction::JPcc(cond, addr) => self.execute_jpcc(cond, addr),
            Instruction::JR(offset) => self.execute_jr(offset),
            Instruction::JRcc(cond, offset) => self.execute_jrcc(cond, offset),
            Instruction::LD(dest, src) => self.execute_ld(dest, src),
            Instruction::LDsr(special_reg, dir) => self.execute_ldsr(special_reg, dir),
            Instruction::LDD | Instruction::LDDR | Instruction::LDI | Instruction::LDIR => self.execute_ldx(),
            Instruction::NEG => self.execute_neg(),
            Instruction::NOP => Ok(()),
            Instruction::OR(target) => self.execute_or(target),
            //Instruction::OTDR => {
            //},
            //Instruction::OTIR => {
            //},
            //Instruction::OUTD => {
            //},
            //Instruction::OUTI => {
            //},
            Instruction::OUTic(reg) => self.execute_outic(reg),
            //Instruction::OUTicz => {
            //},
            Instruction::OUTx(n) => self.execute_outx(n),
            Instruction::POP(regpair) => self.execute_pop(regpair),
            Instruction::PUSH(regpair) => self.execute_push(regpair),
            Instruction::RES(bit, target, opt_copy) => self.execute_res(bit, target, opt_copy),
            Instruction::RET => self.execute_ret(),
            Instruction::RETI => self.execute_reti(),
            Instruction::RETN => self.execute_retn(),
            Instruction::RETcc(cond) => self.execute_retcc(cond),
            Instruction::RL(target, opt_copy) => self.execute_rl(target, opt_copy),
            Instruction::RLA => self.execute_rla(),
            Instruction::RLC(target, opt_copy) => self.execute_rlc(target, opt_copy),
            Instruction::RLCA => self.execute_rlca(),
            Instruction::RLD => self.execute_rld(),
            Instruction::RR(target, opt_copy) => self.execute_rr(target, opt_copy),
            Instruction::RRA => self.execute_rra(),
            Instruction::RRC(target, opt_copy) => self.execute_rrc(target, opt_copy),
            Instruction::RRCA => self.execute_rrca(),
            Instruction::RRD => self.execute_rrd(),
            Instruction::RST(addr) => self.execute_rst(addr),
            Instruction::SBCa(target) => self.execute_sbca(target),
            Instruction::SBC16(dest_pair, src_pair) => self.execute_sbc16(dest_pair, src_pair),
            Instruction::SCF => self.execute_scf(),
            Instruction::SET(bit, target, opt_copy) => self.execute_set(bit, target, opt_copy),
            Instruction::SLA(target, opt_copy) => self.execute_sla(target, opt_copy),
            Instruction::SLL(target, opt_copy) => self.execute_sll(target, opt_copy),
            Instruction::SRA(target, opt_copy) => self.execute_sra(target, opt_copy),
            Instruction::SRL(target, opt_copy) => self.execute_srl(target, opt_copy),
            Instruction::SUB(target) => self.execute_sub(target),
            Instruction::XOR(target) => self.execute_xor(target),
            _ => Err(Z80Error::Unimplemented(self.decoder.instruction.clone())),
        }
    }

    fn execute_adca(&mut self, target: Target) -> Result<(), Z80Error> {
        let src = self.get_target_value(target)?;
        let acc = self.get_register_value(Register::A);

        let (result1, carry1, overflow1, half_carry1) = add_bytes(acc, self.get_flag(Flags::Carry) as u8);
        let (result2, carry2, overflow2, half_carry2) = add_bytes(result1, src);
        self.set_arithmetic_op_flags(
            result2 as u16,
            Size::Byte,
            false,
            carry1 | carry2,
            overflow1 ^ overflow2,
            half_carry1 | half_carry2,
        );

        self.set_register_value(Register::A, result2);
        Ok(())
    }

    fn execute_adc16(&mut self, dest_pair: RegisterPair, src_pair: RegisterPair) -> Result<(), Z80Error> {
        let src = self.get_register_pair_value(src_pair);
        let dest = self.get_register_pair_value(dest_pair);

        let (result1, carry1, overflow1, half_carry1) = add_words(dest, src);
        let (result2, carry2, overflow2, half_carry2) = add_words(result1, self.get_flag(Flags::Carry) as u16);
        self.set_arithmetic_op_flags(result2, Size::Word, false, carry1 | carry2, overflow1 ^ overflow2, half_carry1 | half_carry2);

        self.set_register_pair_value(dest_pair, result2);
        Ok(())
    }

    fn execute_adda(&mut self, target: Target) -> Result<(), Z80Error> {
        let src = self.get_target_value(target)?;
        let acc = self.get_register_value(Register::A);

        let (result, carry, overflow, half_carry) = add_bytes(acc, src);
        self.set_arithmetic_op_flags(result as u16, Size::Byte, false, carry, overflow, half_carry);

        self.set_register_value(Register::A, result);
        Ok(())
    }

    fn execute_add16(&mut self, dest_pair: RegisterPair, src_pair: RegisterPair) -> Result<(), Z80Error> {
        let src = self.get_register_pair_value(src_pair);
        let dest = self.get_register_pair_value(dest_pair);

        let (result, carry, _, half_carry) = add_words(dest, src);
        self.set_flag(Flags::AddSubtract, false);
        self.set_flag(Flags::Carry, carry);
        self.set_flag(Flags::HalfCarry, half_carry);

        self.set_register_pair_value(dest_pair, result);
        Ok(())
    }

    fn execute_and(&mut self, target: Target) -> Result<(), Z80Error> {
        let acc = self.get_register_value(Register::A);
        let value = self.get_target_value(target)?;
        let result = acc & value;
        self.set_register_value(Register::A, result);
        self.set_logic_op_flags(result, false, true);
        Ok(())
    }

    fn execute_bit(&mut self, bit: u8, target: Target) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let result = value & (1 << bit);
        self.set_flag(Flags::Zero, result == 0);
        self.set_flag(Flags::Sign, bit == 7 && result != 0);
        self.set_flag(Flags::Parity, result == 0);
        self.set_flag(Flags::AddSubtract, false);
        self.set_flag(Flags::HalfCarry, true);
        Ok(())
    }

    fn execute_call(&mut self, addr: u16) -> Result<(), Z80Error> {
        self.push_word(self.decoder.end)?;
        self.state.pc = addr;
        Ok(())
    }

    fn execute_callcc(&mut self, cond: Condition, addr: u16) -> Result<(), Z80Error> {
        if self.get_current_condition(cond) {
            self.executor.took_branch = true;
            self.push_word(self.decoder.end)?;
            self.state.pc = addr;
        }
        Ok(())
    }

    fn execute_ccf(&mut self) -> Result<(), Z80Error> {
        self.set_flag(Flags::AddSubtract, false);
        self.set_flag(Flags::HalfCarry, self.get_flag(Flags::Carry));
        self.set_flag(Flags::Carry, !self.get_flag(Flags::Carry));
        Ok(())
    }

    fn execute_cp(&mut self, target: Target) -> Result<(), Z80Error> {
        let src = self.get_target_value(target)?;
        let acc = self.get_register_value(Register::A);

        let (result, carry, overflow, half_carry) = sub_bytes(acc, src);
        self.set_arithmetic_op_flags(result as u16, Size::Byte, true, carry, overflow, half_carry);
        Ok(())
    }

    //Instruction::CPD => {
    //}
    //Instruction::CPDR => {
    //}
    //Instruction::CPI => {
    //}
    //Instruction::CPIR => {
    //}

    fn execute_cpl(&mut self) -> Result<(), Z80Error> {
        let value = self.get_register_value(Register::A);
        self.set_register_value(Register::A, !value);
        self.set_flag(Flags::HalfCarry, true);
        self.set_flag(Flags::AddSubtract, true);
        Ok(())
    }

    fn execute_daa(&mut self) -> Result<(), Z80Error> {
        // From <http://z80-heaven.wikidot.com/instructions-set:daa>
        // if the least significant four bits of A contain a non-BCD digit (i. e. it is
        // greater than 9) or the H flag is set, then $06 is added to the register. Then
        // the four most significant bits are checked. If this more significant digit
        // also happens to be greater than 9 or the C flag is set, then $60 is added.

        // From <http://www.z80.info/zip/z80-documented.pdf>
        //
        // CF |  high  | HF |  low   | diff
        //    | nibble |    | nibble |
        //----------------------------------
        //  0 |    0-9 |  0 |    0-9 |  00
        //  0 |    0-9 |  1 |    0-9 |  06
        //  0 |    0-8 |  * |    a-f |  06
        //  0 |    a-f |  0 |    0-9 |  60
        //  1 |      * |  0 |    0-9 |  60
        //  1 |      * |  1 |    0-9 |  66
        //  1 |      * |  * |    a-f |  66
        //  0 |    9-f |  * |    a-f |  66
        //  0 |    a-f |  1 |    0-9 |  66

        let mut value = self.get_register_value(Register::A);
        let mut carry = false;
        let mut half_carry = false;
        if (value & 0x0F) > 9 || self.get_flag(Flags::HalfCarry) {
            let (result, _, _, half_carry1) = add_bytes(value, 6);
            value = result;
            half_carry = half_carry1;
        }
        if (value & 0xF0) > 0x90 || self.get_flag(Flags::Carry) {
            let (result, _, _, half_carry2) = add_bytes(value, 0x60);
            value = result;
            half_carry |= half_carry2;
            carry = true;
        }
        self.set_register_value(Register::A, value);

        self.set_numeric_flags(value as u16, Size::Byte);
        self.set_parity_flags(value);
        self.set_flag(Flags::HalfCarry, half_carry);
        self.set_flag(Flags::Carry, carry);
        Ok(())
    }

    fn execute_dec16(&mut self, regpair: RegisterPair) -> Result<(), Z80Error> {
        let value = self.get_register_pair_value(regpair);

        let (result, _, _, _) = sub_words(value, 1);

        self.set_register_pair_value(regpair, result);
        Ok(())
    }

    fn execute_dec8(&mut self, target: Target) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;

        let (result, _, overflow, half_carry) = sub_bytes(value, 1);
        let carry = self.get_flag(Flags::Carry); // Preserve the carry bit, according to Z80 reference
        self.set_arithmetic_op_flags(result as u16, Size::Byte, true, carry, overflow, half_carry);

        self.set_target_value(target, result)?;
        Ok(())
    }

    fn execute_di(&mut self) -> Result<(), Z80Error> {
        self.state.iff1 = false;
        self.state.iff2 = false;
        Ok(())
    }

    fn execute_djnz(&mut self, offset: i8) -> Result<(), Z80Error> {
        let value = self.get_register_value(Register::B);
        let result = value.wrapping_sub(1);
        self.set_register_value(Register::B, result);

        if result != 0 {
            self.executor.took_branch = true;
            self.state.pc = self.state.pc.wrapping_add_signed(offset as i16);
        }
        Ok(())
    }

    fn execute_ei(&mut self) -> Result<(), Z80Error> {
        self.state.iff1 = true;
        self.state.iff2 = true;
        Ok(())
    }

    fn execute_exx(&mut self) -> Result<(), Z80Error> {
        for i in 0..6 {
            let (normal, shadow) = (self.state.reg[i], self.state.shadow_reg[i]);
            self.state.reg[i] = shadow;
            self.state.shadow_reg[i] = normal;
        }
        Ok(())
    }

    fn execute_ex_af_af(&mut self) -> Result<(), Z80Error> {
        for i in 6..8 {
            let (normal, shadow) = (self.state.reg[i], self.state.shadow_reg[i]);
            self.state.reg[i] = shadow;
            self.state.shadow_reg[i] = normal;
        }
        Ok(())
    }

    fn execute_ex_hl_de(&mut self) -> Result<(), Z80Error> {
        let (hl, de) = (self.get_register_pair_value(RegisterPair::HL), self.get_register_pair_value(RegisterPair::DE));
        self.set_register_pair_value(RegisterPair::DE, hl);
        self.set_register_pair_value(RegisterPair::HL, de);
        Ok(())
    }

    fn execute_ex_sp(&mut self, regpair: RegisterPair) -> Result<(), Z80Error> {
        let reg_value = self.get_register_pair_value(regpair);
        let sp = self.get_register_pair_value(RegisterPair::SP);
        let sp_value = self.read_port_u16(sp)?;
        self.set_register_pair_value(regpair, sp_value);
        self.write_port_u16(sp, reg_value)?;
        Ok(())
    }

    fn execute_halt(&mut self) -> Result<(), Z80Error> {
        self.state.status = Status::Halted;
        self.state.pc -= 1;
        Ok(())
    }

    fn execute_im(&mut self, mode: InterruptMode) -> Result<(), Z80Error> {
        self.state.im = mode;
        Ok(())
    }

    fn execute_inc16(&mut self, regpair: RegisterPair) -> Result<(), Z80Error> {
        let value = self.get_register_pair_value(regpair);

        let (result, _, _, _) = add_words(value, 1);

        self.set_register_pair_value(regpair, result);
        Ok(())
    }

    fn execute_inc8(&mut self, target: Target) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let (result, _, overflow, half_carry) = add_bytes(value, 1);
        let carry = self.get_flag(Flags::Carry); // Preserve the carry bit, according to Z80 reference
        self.set_arithmetic_op_flags(result as u16, Size::Byte, false, carry, overflow, half_carry);

        self.set_target_value(target, result)?;
        Ok(())
    }

    //Instruction::IND => {
    //},
    //Instruction::INDR => {
    //}

    fn execute_ini(&mut self) -> Result<(), Z80Error> {
        let b = self.get_register_value(Register::B);
        let c = self.get_register_value(Register::C);
        let value = self.read_ioport_value(b, c)?;

        self.set_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::HL), value as u16)?;
        let hl = self.get_register_pair_value(RegisterPair::HL).wrapping_add(1);
        self.set_register_pair_value(RegisterPair::HL, hl);
        let b = self.get_register_value(Register::B).wrapping_sub(1);
        self.set_register_value(Register::B, b);
        Ok(())
    }

    //Instruction::INIR => {
    //}

    fn execute_inic(&mut self, reg: Register) -> Result<(), Z80Error> {
        let b = self.get_register_value(Register::B);
        let c = self.get_register_value(Register::C);
        let value = self.read_ioport_value(b, c)?;

        self.set_register_value(reg, value);
        self.set_numeric_flags(value as u16, Size::Byte);
        self.set_parity_flags(value);
        self.set_flag(Flags::HalfCarry, false);
        self.set_flag(Flags::AddSubtract, false);
        Ok(())
    }

    //Instruction::INicz => {
    //}

    fn execute_inx(&mut self, n: u8) -> Result<(), Z80Error> {
        let a = self.get_register_value(Register::A);
        let value = self.read_ioport_value(a, n)?;
        self.set_register_value(Register::A, value);
        Ok(())
    }

    fn execute_jp(&mut self, addr: u16) -> Result<(), Z80Error> {
        self.state.pc = addr;
        Ok(())
    }

    fn execute_jp_indirect(&mut self, regpair: RegisterPair) -> Result<(), Z80Error> {
        let value = self.get_register_pair_value(regpair);
        self.state.pc = value;
        Ok(())
    }

    fn execute_jpcc(&mut self, cond: Condition, addr: u16) -> Result<(), Z80Error> {
        if self.get_current_condition(cond) {
            self.executor.took_branch = true;
            self.state.pc = addr;
        }
        Ok(())
    }

    fn execute_jr(&mut self, offset: i8) -> Result<(), Z80Error> {
        self.state.pc = self.state.pc.wrapping_add_signed(offset as i16);
        Ok(())
    }

    fn execute_jrcc(&mut self, cond: Condition, offset: i8) -> Result<(), Z80Error> {
        if self.get_current_condition(cond) {
            self.executor.took_branch = true;
            self.state.pc = self.state.pc.wrapping_add_signed(offset as i16);
        }
        Ok(())
    }

    fn execute_ld(&mut self, dest: LoadTarget, src: LoadTarget) -> Result<(), Z80Error> {
        let src_value = self.get_load_target_value(src)?;
        self.set_load_target_value(dest, src_value)?;
        Ok(())
    }

    fn execute_ldsr(&mut self, special_reg: SpecialRegister, dir: Direction) -> Result<(), Z80Error> {
        let addr = match special_reg {
            SpecialRegister::I => &mut self.state.i,
            SpecialRegister::R => &mut self.state.r,
        };

        match dir {
            Direction::FromAcc => {
                *addr = self.state.reg[Register::A as usize];
            },
            Direction::ToAcc => {
                self.state.reg[Register::A as usize] = *addr;
                let value = self.state.reg[Register::A as usize];
                self.set_numeric_flags(value as u16, Size::Byte);
                self.set_flag(Flags::Parity, self.state.iff2);
                self.set_flag(Flags::AddSubtract, false);
                self.set_flag(Flags::HalfCarry, false);
            },
        }

        Ok(())
    }

    fn execute_ldx(&mut self) -> Result<(), Z80Error> {
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

        if (self.decoder.instruction == Instruction::LDIR || self.decoder.instruction == Instruction::LDDR) && count != 0 {
            self.executor.took_branch = true;
            self.state.pc -= 2;
        }
        Ok(())
    }

    fn execute_neg(&mut self) -> Result<(), Z80Error> {
        let acc = self.get_register_value(Register::A);

        let (result, carry, overflow, half_carry) = sub_bytes(0, acc);
        self.set_arithmetic_op_flags(result as u16, Size::Byte, true, carry, overflow, half_carry);

        self.set_register_value(Register::A, result);
        Ok(())
    }

    fn execute_or(&mut self, target: Target) -> Result<(), Z80Error> {
        let acc = self.get_register_value(Register::A);
        let value = self.get_target_value(target)?;
        let result = acc | value;
        self.set_register_value(Register::A, result);
        self.set_logic_op_flags(result, false, false);
        Ok(())
    }

    //Instruction::OTDR => {
    //},
    //Instruction::OTIR => {
    //},
    //Instruction::OUTD => {
    //},
    //Instruction::OUTI => {
    //}

    fn execute_outic(&mut self, reg: Register) -> Result<(), Z80Error> {
        let b = self.get_register_value(Register::B);
        let c = self.get_register_value(Register::C);
        let value = self.get_register_value(reg);
        self.write_ioport_value(b, c, value)?;
        Ok(())
    }

    //Instruction::OUTicz => {
    //}

    fn execute_outx(&mut self, n: u8) -> Result<(), Z80Error> {
        let a = self.get_register_value(Register::A);
        let value = self.get_register_value(Register::A);
        self.write_ioport_value(a, n, value)?;
        Ok(())
    }

    fn execute_pop(&mut self, regpair: RegisterPair) -> Result<(), Z80Error> {
        let value = self.pop_word()?;
        self.set_register_pair_value(regpair, value);
        Ok(())
    }

    fn execute_push(&mut self, regpair: RegisterPair) -> Result<(), Z80Error> {
        let value = self.get_register_pair_value(regpair);
        self.push_word(value)?;
        Ok(())
    }

    fn execute_res(&mut self, bit: u8, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let mut value = self.get_target_value(target)?;
        value &= !(1 << bit);
        self.set_target_value(target, value)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, value)?;
        }
        Ok(())
    }

    fn execute_ret(&mut self) -> Result<(), Z80Error> {
        self.state.pc = self.pop_word()?;
        Ok(())
    }

    fn execute_reti(&mut self) -> Result<(), Z80Error> {
        self.state.pc = self.pop_word()?;
        self.state.iff1 = self.state.iff2;
        Ok(())
    }

    fn execute_retn(&mut self) -> Result<(), Z80Error> {
        self.state.pc = self.pop_word()?;
        self.state.iff1 = self.state.iff2;
        Ok(())
    }

    fn execute_retcc(&mut self, cond: Condition) -> Result<(), Z80Error> {
        if self.get_current_condition(cond) {
            self.executor.took_branch = true;
            self.state.pc = self.pop_word()?;
        }
        Ok(())
    }

    fn execute_rl(&mut self, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let (result, out_bit) = self.rotate_left(value, RotateType::Bit9);
        self.set_logic_op_flags(result, out_bit, false);
        self.set_target_value(target, result)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, result)?;
        }
        Ok(())
    }

    fn execute_rla(&mut self) -> Result<(), Z80Error> {
        let value = self.get_register_value(Register::A);
        let (result, out_bit) = self.rotate_left(value, RotateType::Bit9);
        self.set_flag(Flags::AddSubtract, false);
        self.set_flag(Flags::HalfCarry, false);
        self.set_flag(Flags::Carry, out_bit);
        self.set_register_value(Register::A, result);
        Ok(())
    }

    fn execute_rlc(&mut self, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let (result, out_bit) = self.rotate_left(value, RotateType::Bit8);
        self.set_logic_op_flags(result, out_bit, false);
        self.set_target_value(target, result)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, result)?;
        }
        Ok(())
    }

    fn execute_rlca(&mut self) -> Result<(), Z80Error> {
        let value = self.get_register_value(Register::A);
        let (result, out_bit) = self.rotate_left(value, RotateType::Bit8);
        self.set_flag(Flags::AddSubtract, false);
        self.set_flag(Flags::HalfCarry, false);
        self.set_flag(Flags::Carry, out_bit);
        self.set_register_value(Register::A, result);
        Ok(())
    }

    fn execute_rld(&mut self) -> Result<(), Z80Error> {
        let a = self.get_register_value(Register::A);
        let mem = self.get_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::HL))? as u8;

        let lower_a = a & 0x0F;
        let a = (a & 0xF0) | (mem >> 4);
        let mem = (mem << 4) | lower_a;

        self.set_register_value(Register::A, a);
        self.set_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::HL), mem as u16)?;

        self.set_numeric_flags(a as u16, Size::Byte);
        self.set_parity_flags(a);
        self.set_flag(Flags::HalfCarry, false);
        self.set_flag(Flags::AddSubtract, false);
        Ok(())
    }

    fn execute_rr(&mut self, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let (result, out_bit) = self.rotate_right(value, RotateType::Bit9);
        self.set_logic_op_flags(result, out_bit, false);
        self.set_target_value(target, result)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, result)?;
        }
        Ok(())
    }

    fn execute_rra(&mut self) -> Result<(), Z80Error> {
        let value = self.get_register_value(Register::A);
        let (result, out_bit) = self.rotate_right(value, RotateType::Bit9);
        self.set_flag(Flags::AddSubtract, false);
        self.set_flag(Flags::HalfCarry, false);
        self.set_flag(Flags::Carry, out_bit);
        self.set_register_value(Register::A, result);
        Ok(())
    }

    fn execute_rrc(&mut self, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let (result, out_bit) = self.rotate_right(value, RotateType::Bit8);
        self.set_logic_op_flags(result, out_bit, false);
        self.set_target_value(target, result)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, result)?;
        }
        Ok(())
    }

    fn execute_rrca(&mut self) -> Result<(), Z80Error> {
        let value = self.get_register_value(Register::A);
        let (result, out_bit) = self.rotate_right(value, RotateType::Bit8);
        self.set_flag(Flags::AddSubtract, false);
        self.set_flag(Flags::HalfCarry, false);
        self.set_flag(Flags::Carry, out_bit);
        self.set_register_value(Register::A, result);
        Ok(())
    }

    fn execute_rrd(&mut self) -> Result<(), Z80Error> {
        let a = self.get_register_value(Register::A);
        let mem = self.get_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::HL))? as u8;

        let lower_mem = mem & 0x0F;
        let mem = (a << 4) | (mem >> 4);
        let a = (a & 0xF0) | lower_mem;

        self.set_register_value(Register::A, a);
        self.set_load_target_value(LoadTarget::IndirectRegByte(RegisterPair::HL), mem as u16)?;

        self.set_numeric_flags(a as u16, Size::Byte);
        self.set_parity_flags(a);
        self.set_flag(Flags::HalfCarry, false);
        self.set_flag(Flags::AddSubtract, false);
        Ok(())
    }

    fn execute_rst(&mut self, addr: u8) -> Result<(), Z80Error> {
        self.push_word(self.decoder.end)?;
        self.state.pc = addr as u16;
        Ok(())
    }

    fn execute_sbca(&mut self, target: Target) -> Result<(), Z80Error> {
        let src = self.get_target_value(target)?;
        let acc = self.get_register_value(Register::A);

        let (result1, carry1, overflow1, half_carry1) = sub_bytes(acc, src);
        let (result2, carry2, overflow2, half_carry2) = sub_bytes(result1, self.get_flag(Flags::Carry) as u8);
        self.set_arithmetic_op_flags(
            result2 as u16,
            Size::Byte,
            true,
            carry1 | carry2,
            overflow1 ^ overflow2,
            half_carry1 | half_carry2,
        );

        self.set_register_value(Register::A, result2);
        Ok(())
    }

    fn execute_sbc16(&mut self, dest_pair: RegisterPair, src_pair: RegisterPair) -> Result<(), Z80Error> {
        let src = self.get_register_pair_value(src_pair);
        let dest = self.get_register_pair_value(dest_pair);

        let (result1, carry1, overflow1, half_carry1) = sub_words(dest, self.get_flag(Flags::Carry) as u16);
        let (result2, carry2, overflow2, half_carry2) = sub_words(result1, src);
        self.set_arithmetic_op_flags(result2, Size::Word, true, carry1 | carry2, overflow1 ^ overflow2, half_carry1 | half_carry2);

        self.set_register_pair_value(dest_pair, result2);
        Ok(())
    }

    fn execute_scf(&mut self) -> Result<(), Z80Error> {
        self.set_flag(Flags::AddSubtract, false);
        self.set_flag(Flags::HalfCarry, false);
        self.set_flag(Flags::Carry, true);
        Ok(())
    }

    fn execute_set(&mut self, bit: u8, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let mut value = self.get_target_value(target)?;
        value |= 1 << bit;
        self.set_target_value(target, value)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, value)?;
        }
        Ok(())
    }

    fn execute_sla(&mut self, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let out_bit = get_msb(value as u16, Size::Byte);
        let result = value << 1;
        self.set_logic_op_flags(result, out_bit, false);
        self.set_target_value(target, result)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, result)?;
        }
        Ok(())
    }

    fn execute_sll(&mut self, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let out_bit = get_msb(value as u16, Size::Byte);
        let result = (value << 1) | 0x01;
        self.set_logic_op_flags(result, out_bit, false);
        self.set_target_value(target, result)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, result)?;
        }
        Ok(())
    }

    fn execute_sra(&mut self, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let out_bit = (value & 0x01) != 0;
        let msb_mask = if get_msb(value as u16, Size::Byte) { 0x80 } else { 0 };
        let result = (value >> 1) | msb_mask;
        self.set_logic_op_flags(result, out_bit, false);
        self.set_target_value(target, result)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, result)?;
        }
        Ok(())
    }

    fn execute_srl(&mut self, target: Target, opt_copy: UndocumentedCopy) -> Result<(), Z80Error> {
        let value = self.get_target_value(target)?;
        let out_bit = (value & 0x01) != 0;
        let result = value >> 1;
        self.set_logic_op_flags(result, out_bit, false);
        self.set_target_value(target, result)?;
        if let Some(target) = opt_copy {
            self.set_target_value(target, result)?;
        }
        Ok(())
    }

    fn execute_sub(&mut self, target: Target) -> Result<(), Z80Error> {
        let src = self.get_target_value(target)?;
        let acc = self.get_register_value(Register::A);

        let (result, carry, overflow, half_carry) = sub_bytes(acc, src);
        self.set_arithmetic_op_flags(result as u16, Size::Byte, true, carry, overflow, half_carry);

        self.set_register_value(Register::A, result);
        Ok(())
    }

    fn execute_xor(&mut self, target: Target) -> Result<(), Z80Error> {
        let acc = self.get_register_value(Register::A);
        let value = self.get_target_value(target)?;
        let result = acc ^ value;
        self.set_register_value(Register::A, result);
        self.set_logic_op_flags(result, false, false);
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



    fn push_word(&mut self, value: u16) -> Result<(), Z80Error> {
        self.state.sp = self.state.sp.wrapping_sub(1);
        self.write_port_u8(self.state.sp, (value >> 8) as u8)?;
        self.state.sp = self.state.sp.wrapping_sub(1);
        self.write_port_u8(self.state.sp, (value & 0x00FF) as u8)?;
        Ok(())
    }

    fn pop_word(&mut self) -> Result<u16, Z80Error> {
        let mut value;
        value = self.read_port_u8(self.state.sp)? as u16;
        self.state.sp = self.state.sp.wrapping_add(1);
        value |= (self.read_port_u8(self.state.sp)? as u16) << 8;
        self.state.sp = self.state.sp.wrapping_add(1);
        Ok(value)
    }

    fn get_load_target_value(&mut self, target: LoadTarget) -> Result<u16, Z80Error> {
        let value = match target {
            LoadTarget::DirectRegByte(reg) => self.get_register_value(reg) as u16,
            LoadTarget::DirectRegHalfByte(reg) => self.get_index_register_half_value(reg) as u16,
            LoadTarget::DirectRegWord(regpair) => self.get_register_pair_value(regpair),
            LoadTarget::IndirectRegByte(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.read_port_u8(addr)? as u16
            },
            LoadTarget::IndirectOffsetByte(index_reg, offset) => {
                let addr = self.get_index_register_value(index_reg);
                self.read_port_u8((addr as i16).wrapping_add(offset as i16) as u16)? as u16
            },
            LoadTarget::IndirectRegWord(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.read_port_u16(addr)?
            },
            LoadTarget::IndirectByte(addr) => self.read_port_u8(addr)? as u16,
            LoadTarget::IndirectWord(addr) => self.read_port_u16(addr)?,
            LoadTarget::ImmediateByte(data) => data as u16,
            LoadTarget::ImmediateWord(data) => data,
            _ => panic!("Unsupported LoadTarget for set"),
        };
        Ok(value)
    }

    fn set_load_target_value(&mut self, target: LoadTarget, value: u16) -> Result<(), Z80Error> {
        match target {
            LoadTarget::DirectRegByte(reg) => self.set_register_value(reg, value as u8),
            LoadTarget::DirectRegHalfByte(reg) => self.set_index_register_half_value(reg, value as u8),
            LoadTarget::DirectRegWord(regpair) => self.set_register_pair_value(regpair, value),
            LoadTarget::IndirectRegByte(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.write_port_u8(addr, value as u8)?;
            },
            LoadTarget::IndirectOffsetByte(index_reg, offset) => {
                let addr = self.get_index_register_value(index_reg);
                self.write_port_u8((addr as i16).wrapping_add(offset as i16) as u16, value as u8)?;
            },
            LoadTarget::IndirectRegWord(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.write_port_u16(addr, value)?;
            },
            LoadTarget::IndirectByte(addr) => {
                self.write_port_u8(addr, value as u8)?;
            },
            LoadTarget::IndirectWord(addr) => {
                self.write_port_u16(addr, value)?;
            },
            _ => panic!("Unsupported LoadTarget for set: {:?}", target),
        }
        Ok(())
    }

    fn get_target_value(&mut self, target: Target) -> Result<u8, Z80Error> {
        match target {
            Target::DirectReg(reg) => Ok(self.get_register_value(reg)),
            Target::DirectRegHalf(reg) => Ok(self.get_index_register_half_value(reg)),
            Target::IndirectReg(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                Ok(self.read_port_u8(addr)?)
            },
            Target::IndirectOffset(reg, offset) => {
                let addr = self.get_index_register_value(reg).wrapping_add_signed(offset as i16);
                Ok(self.read_port_u8(addr)?)
            },
            Target::Immediate(data) => Ok(data),
        }
    }

    fn set_target_value(&mut self, target: Target, value: u8) -> Result<(), Z80Error> {
        match target {
            Target::DirectReg(reg) => self.set_register_value(reg, value),
            Target::DirectRegHalf(reg) => self.set_index_register_half_value(reg, value),
            Target::IndirectReg(regpair) => {
                let addr = self.get_register_pair_value(regpair);
                self.write_port_u8(addr, value)?;
            },
            Target::IndirectOffset(reg, offset) => {
                let addr = self.get_index_register_value(reg).wrapping_add_signed(offset as i16);
                self.write_port_u8(addr, value)?;
            },
            _ => panic!("Unsupported LoadTarget for set"),
        }
        Ok(())
    }

    fn increment_refresh(&mut self, count: u8) {
        self.state.r = (self.state.r & 0x80) | ((self.state.r + count) & 0x7F);
    }

    fn read_port_u8(&mut self, addr: u16) -> Result<u8, Z80Error> {
        self.increment_refresh(1);
        Ok(self.port.read_u8(self.executor.current_clock, addr as Address)?)
    }

    fn write_port_u8(&mut self, addr: u16, value: u8) -> Result<(), Z80Error> {
        self.increment_refresh(1);
        Ok(self.port.write_u8(self.executor.current_clock, addr as Address, value)?)
    }

    fn read_port_u16(&mut self, addr: u16) -> Result<u16, Z80Error> {
        self.increment_refresh(2);
        Ok(self.port.read_leu16(self.executor.current_clock, addr as Address)?)
    }

    fn write_port_u16(&mut self, addr: u16, value: u16) -> Result<(), Z80Error> {
        self.increment_refresh(2);
        Ok(self.port.write_leu16(self.executor.current_clock, addr as Address, value)?)
    }

    fn read_ioport_value(&mut self, upper: u8, lower: u8) -> Result<u8, Z80Error> {
        let addr = ((upper as Address) << 8) | (lower as Address);
        if let Some(io) = self.ioport.as_mut() {
            Ok(io.read_u8(self.executor.current_clock, addr)?)
        } else {
            Ok(0)
        }
    }

    fn write_ioport_value(&mut self, upper: u8, lower: u8, value: u8) -> Result<(), Z80Error> {
        let addr = ((upper as Address) << 8) | (lower as Address);
        if let Some(io) = self.ioport.as_mut() {
            io.write_u8(self.executor.current_clock, addr, value)?
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
            IndexRegisterHalf::IXH => {
                self.state.ix = (self.state.ix & 0x00FF) | (value as u16) << 8;
            },
            IndexRegisterHalf::IXL => {
                self.state.ix = (self.state.ix & 0xFF00) | value as u16;
            },
            IndexRegisterHalf::IYH => {
                self.state.iy = (self.state.iy & 0x00FF) | (value as u16) << 8;
            },
            IndexRegisterHalf::IYL => {
                self.state.iy = (self.state.iy & 0xFF00) | value as u16;
            },
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
            RegisterPair::BC => {
                write_beu16(&mut self.state.reg[0..2], value);
            },
            RegisterPair::DE => {
                write_beu16(&mut self.state.reg[2..4], value);
            },
            RegisterPair::HL => {
                write_beu16(&mut self.state.reg[4..6], value);
            },
            RegisterPair::AF => {
                write_beu16(&mut self.state.reg[6..8], value);
            },
            RegisterPair::SP => {
                self.state.sp = value;
            },
            RegisterPair::IX => {
                self.state.ix = value;
            },
            RegisterPair::IY => {
                self.state.iy = value;
            },
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
        let parity = if (value.count_ones() & 0x01) == 0 {
            Flags::Parity as u8
        } else {
            0
        };
        self.set_flags(Flags::Parity as u8, parity);
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

    fn get_flag(&self, flag: Flags) -> bool {
        self.get_flags() & (flag as u8) != 0
    }

    fn set_flag(&mut self, flag: Flags, value: bool) {
        self.state.reg[Register::F as usize] &= !(flag as u8);
        if value {
            self.state.reg[Register::F as usize] |= flag as u8;
        }
    }

    fn get_flags(&self) -> u8 {
        self.state.reg[Register::F as usize]
    }

    fn set_flags(&mut self, mask: u8, values: u8) {
        self.state.reg[Register::F as usize] = (self.state.reg[Register::F as usize] & !mask) | values;
    }
}

fn add_bytes(operand1: u8, operand2: u8) -> (u8, bool, bool, bool) {
    let (result, carry) = operand1.overflowing_add(operand2);
    let overflow = (operand1 & 0x80) == (operand2 & 0x80) && (operand1 & 0x80) != (result & 0x80);
    let half_carry = (operand1 & 0x0F) + (operand2 & 0x0F) >= 0x10;
    (result, carry, overflow, half_carry)
}

fn add_words(operand1: u16, operand2: u16) -> (u16, bool, bool, bool) {
    let (result, carry) = operand1.overflowing_add(operand2);
    let overflow = (operand1 & 0x8000) == (operand2 & 0x8000) && (operand1 & 0x8000) != (result & 0x8000);
    let half_carry = (operand1 & 0x0FFF) + (operand2 & 0x0FFF) >= 0x1000;
    (result, carry, overflow, half_carry)
}

fn sub_bytes(operand1: u8, operand2: u8) -> (u8, bool, bool, bool) {
    let (result, carry) = operand1.overflowing_sub(operand2);
    let overflow = (operand1 & 0x80) != (operand2 & 0x80) && (operand1 & 0x80) != (result & 0x80);
    let half_carry = (operand1 & 0x0F) < (operand2 & 0x0F);
    (result, carry, overflow, half_carry)
}

fn sub_words(operand1: u16, operand2: u16) -> (u16, bool, bool, bool) {
    let (result, carry) = operand1.overflowing_sub(operand2);
    let overflow = (operand1 & 0x8000) != (operand2 & 0x8000) && (operand1 & 0x8000) != (result & 0x8000);
    let half_carry = (operand1 & 0x0FFF) < (operand2 & 0x0FFF);
    (result, carry, overflow, half_carry)
}

fn get_msb(value: u16, size: Size) -> bool {
    match size {
        Size::Byte => (value & 0x0080) != 0,
        Size::Word => (value & 0x8000) != 0,
    }
}
