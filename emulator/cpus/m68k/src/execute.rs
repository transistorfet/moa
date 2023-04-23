
use moa_core::debug;
use moa_core::{System, Error, ErrorType, ClockDuration, Address, Steppable, Interruptable, Addressable, Debuggable, Transmutable};

use crate::state::{M68k, M68kType, Status, Flags, Exceptions, InterruptPriority, FunctionCode, MemType, MemAccess};
use crate::instructions::{
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


const DEV_NAME: &str = "m68k-cpu";

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Used {
    Once,
    Twice,
}

impl Steppable for M68k {
    fn step(&mut self, system: &System) -> Result<ClockDuration, Error> {
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

    pub fn step_internal(&mut self, system: &System) -> Result<ClockDuration, Error> {
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
                        Ok(self.frequency.period_duration() * 4)
                    },
                    Err(err) => Err(err),
                }
            },
        }
    }

    pub fn init(&mut self) -> Result<ClockDuration, Error> {
        self.state.ssp = self.port.read_beu32(0)?;
        self.state.pc = self.port.read_beu32(4)?;
        self.state.status = Status::Running;
        Ok(self.frequency.period_duration() * 16)
    }

    pub fn cycle_one(&mut self, system: &System) -> Result<ClockDuration, Error> {
        self.timer.cycle.start();
        self.decode_next()?;
        self.execute_current()?;
        self.timer.cycle.end();
        //if (self.timer.cycle.events % 500) == 0 {
        //    println!("{}", self.timer);
        //}

        self.check_pending_interrupts(system)?;
        self.check_breakpoints(system);
        Ok(self.frequency.period_duration() * self.timing.calculate_clocks(false, 1) as u64)
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
                debug!("{} interrupt: {} @ {} ns", DEV_NAME, pending_ipl, system.clock.as_duration().as_nanos());
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
        let sr = self.state.sr;
        self.state.request.i_n_bit = true;

        // Changes to the flags must happen after the previous value has been pushed to the stack
        self.set_flag(Flags::Supervisor, true);
        self.set_flag(Flags::Tracing, false);
        if is_interrupt {
            self.state.sr = (self.state.sr & !(Flags::IntMask as u16)) | ((self.state.current_ipl as u16) << 8);
        }

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
            Instruction::ABCD(src, dest) => self.execute_abcd(src, dest),
            Instruction::ADD(src, dest, size) => self.execute_add(src, dest, size),
            Instruction::ADDA(src, dest, size) => self.execute_adda(src, dest, size),
            Instruction::ADDX(src, dest, size) => self.execute_addx(src, dest, size),
            Instruction::AND(src, dest, size) => self.execute_and(src, dest, size),
            Instruction::ANDtoCCR(value) => self.execute_and_to_ccr(value),
            Instruction::ANDtoSR(value) => self.execute_and_to_sr(value),
            Instruction::ASd(count, target, size, shift_dir) => self.execute_asd(count, target, size, shift_dir),
            Instruction::Bcc(cond, offset) => self.execute_bcc(cond, offset),
            Instruction::BRA(offset) => self.execute_bra(offset),
            Instruction::BSR(offset) => self.execute_bsr(offset),
            Instruction::BCHG(bitnum, target, size) => self.execute_bchg(bitnum, target, size),
            Instruction::BCLR(bitnum, target, size) => self.execute_bclr(bitnum, target, size),
            Instruction::BSET(bitnum, target, size) => self.execute_bset(bitnum, target, size),
            Instruction::BTST(bitnum, target, size) => self.execute_btst(bitnum, target, size),
            Instruction::BFCHG(target, offset, width) => self.execute_bfchg(target, offset, width),
            Instruction::BFCLR(target, offset, width) => self.execute_bfclr(target, offset, width),
            Instruction::BFEXTS(target, offset, width, reg) => self.execute_bfexts(target, offset, width, reg),
            Instruction::BFEXTU(target, offset, width, reg) => self.execute_bfextu(target, offset, width, reg),
            //Instruction::BFFFO(target, offset, width, reg) => {},
            //Instruction::BFINS(reg, target, offset, width) => {},
            Instruction::BFSET(target, offset, width) => self.execute_bfset(target, offset, width),
            Instruction::BFTST(target, offset, width) => self.execute_bftst(target, offset, width),
            //Instruction::BKPT(u8) => {},
            Instruction::CHK(target, reg, size) => self.execute_chk(target, reg, size),
            Instruction::CLR(target, size) => self.execute_clr(target, size),
            Instruction::CMP(src, dest, size) => self.execute_cmp(src, dest, size),
            Instruction::CMPA(src, reg, size) => self.execute_cmpa(src, reg, size),
            Instruction::DBcc(cond, reg, offset) => self.execute_dbcc(cond, reg, offset),
            Instruction::DIVW(src, dest, sign) => self.execute_divw(src, dest, sign),
            Instruction::DIVL(src, dest_h, dest_l, sign) => self.execute_divl(src, dest_h, dest_l, sign),
            Instruction::EOR(src, dest, size) => self.execute_eor(src, dest, size),
            Instruction::EORtoCCR(value) => self.execute_eor_to_ccr(value),
            Instruction::EORtoSR(value) => self.execute_eor_to_sr(value),
            Instruction::EXG(target1, target2) => self.execute_exg(target1, target2),
            Instruction::EXT(reg, from_size, to_size) => self.execute_ext(reg, from_size, to_size),
            Instruction::ILLEGAL => self.execute_illegal(),
            Instruction::JMP(target) => self.execute_jmp(target),
            Instruction::JSR(target) => self.execute_jsr(target),
            Instruction::LEA(target, reg) => self.execute_lea(target, reg),
            Instruction::LINK(reg, offset) => self.execute_link(reg, offset),
            Instruction::LSd(count, target, size, shift_dir) => self.execute_lsd(count, target, size, shift_dir),
            Instruction::MOVE(src, dest, size) => self.execute_move(src, dest, size),
            Instruction::MOVEA(src, reg, size) => self.execute_movea(src, reg, size),
            Instruction::MOVEfromSR(target) => self.execute_move_from_sr(target),
            Instruction::MOVEtoSR(target) => self.execute_move_to_sr(target),
            Instruction::MOVEtoCCR(target) => self.execute_move_to_ccr(target),
            Instruction::MOVEC(target, control_reg, dir) => self.execute_movec(target, control_reg, dir),
            Instruction::MOVEM(target, size, dir, mask) => self.execute_movem(target, size, dir, mask),
            Instruction::MOVEP(dreg, areg, offset, size, dir) => self.execute_movep(dreg, areg, offset, size, dir),
            Instruction::MOVEQ(data, reg) => self.execute_moveq(data, reg),
            Instruction::MOVEUSP(target, dir) => self.execute_moveusp(target, dir),
            Instruction::MULW(src, dest, sign) => self.execute_mulw(src, dest, sign),
            Instruction::MULL(src, dest_h, dest_l, sign) => self.execute_mull(src, dest_h, dest_l, sign),
            Instruction::NBCD(dest) => self.execute_nbcd(dest),
            Instruction::NEG(target, size) => self.execute_neg(target, size),
            Instruction::NEGX(dest, size) => self.execute_negx(dest, size),
            Instruction::NOP => Ok(()),
            Instruction::NOT(target, size) => self.execute_not(target, size),
            Instruction::OR(src, dest, size) => self.execute_or(src, dest, size),
            Instruction::ORtoCCR(value) => self.execute_or_to_ccr(value),
            Instruction::ORtoSR(value) => self.execute_or_to_sr(value),
            Instruction::PEA(target) => self.execute_pea(target),
            Instruction::RESET => self.execute_reset(),
            Instruction::ROd(count, target, size, shift_dir) => self.execute_rod(count, target, size, shift_dir),
            Instruction::ROXd(count, target, size, shift_dir) => self.execute_roxd(count, target, size, shift_dir),
            Instruction::RTE => self.execute_rte(),
            Instruction::RTR => self.execute_rtr(),
            Instruction::RTS => self.execute_rts(),
            //Instruction::RTD(i16) => {},
            Instruction::Scc(cond, target) => self.execute_scc(cond, target),
            Instruction::STOP(flags) => self.execute_stop(flags),
            Instruction::SBCD(src, dest) => self.execute_sbcd(src, dest),
            Instruction::SUB(src, dest, size) => self.execute_sub(src, dest, size),
            Instruction::SUBA(src, dest, size) => self.execute_suba(src, dest, size),
            Instruction::SUBX(src, dest, size) => self.execute_subx(src, dest, size),
            Instruction::SWAP(reg) => self.execute_swap(reg),
            Instruction::TAS(target) => self.execute_tas(target),
            Instruction::TST(target, size) => self.execute_tst(target, size),
            Instruction::TRAP(number) => self.execute_trap(number),
            Instruction::TRAPV => self.execute_trapv(),
            Instruction::UNLK(reg) => self.execute_unlk(reg),
            Instruction::UnimplementedA(value) => self.execute_unimplemented_a(value),
            Instruction::UnimplementedF(value) => self.execute_unimplemented_f(value),
            _ => { return Err(Error::new("Unsupported instruction")); },
        }?;

        self.timer.execute.end();
        Ok(())
    }

    #[inline]
    fn execute_abcd(&mut self, src: Target, dest: Target) -> Result<(), Error> {
        let src_val = self.get_target_value(src, Size::Byte, Used::Once)?;
        let dest_val = self.get_target_value(dest, Size::Byte, Used::Twice)?;

        let extend_flag = self.get_flag(Flags::Extend) as u32;
        let src_parts = get_nibbles_from_byte(src_val);
        let dest_parts = get_nibbles_from_byte(dest_val);

        let binary_result = src_val + dest_val + extend_flag;
        let mut result = src_parts.1 + dest_parts.1 + extend_flag;
        if result > 0x09 { result += 0x06 };
        result += src_parts.0 + dest_parts.0;
        if result > 0x99 { result += 0x60 };
        let carry = (result & 0xFFFFFF00) != 0;

        self.set_target_value(dest, result, Size::Byte, Used::Twice)?;
        self.set_flag(Flags::Negative, get_msb(result, Size::Byte));
        self.set_flag(Flags::Zero, result == 0);
        self.set_flag(Flags::Overflow, (!binary_result & result & 0x80) != 0);
        self.set_flag(Flags::Carry, carry);
        self.set_flag(Flags::Extend, carry);
        Ok(())
    }

    #[inline]
    fn execute_add(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let (result, carry) = overflowing_add_sized(dest_val, src_val, size);
        let overflow = get_add_overflow(dest_val, src_val, result, size);
        self.set_compare_flags(result, size, carry, overflow);
        self.set_flag(Flags::Extend, carry);
        self.set_target_value(dest, result, size, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_adda(&mut self, src: Target, dest: Register, size: Size) -> Result<(), Error> {
        let src_val = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
        let dest_val = *self.get_a_reg_mut(dest);
        let (result, _) = overflowing_add_sized(dest_val, src_val, Size::Long);
        *self.get_a_reg_mut(dest) = result;
        Ok(())
    }

    #[inline]
    fn execute_addx(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let extend = self.get_flag(Flags::Extend) as u32;
        let (result1, carry1) = overflowing_add_sized(dest_val, src_val, size);
        let (result2, carry2) = overflowing_add_sized(result1, extend, size);
        let overflow = get_add_overflow(dest_val, src_val, result2, size);

        // Handle flags
        let zero = self.get_flag(Flags::Zero);
        self.set_compare_flags(result2, size, carry1 || carry2, overflow);
        if self.get_flag(Flags::Zero) {
            // ADDX can only clear the zero flag, so if it's set, restore it to whatever it was before
            self.set_flag(Flags::Zero, zero);
        }
        self.set_flag(Flags::Extend, carry1 || carry2);

        self.set_target_value(dest, result2, size, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_and(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let result = get_value_sized(dest_val & src_val, size);
        self.set_target_value(dest, result, size, Used::Twice)?;
        self.set_logic_flags(result, size);
        Ok(())
    }

    #[inline]
    fn execute_and_to_ccr(&mut self, value: u8) -> Result<(), Error> {
        self.state.sr = (self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) & (value as u16));
        Ok(())
    }

    #[inline]
    fn execute_and_to_sr(&mut self, value: u16) -> Result<(), Error> {
        self.require_supervisor()?;
        self.set_sr(self.state.sr & value);
        Ok(())
    }

    #[inline]
    fn execute_asd(&mut self, count: Target, target: Target, size: Size, shift_dir: ShiftDirection) -> Result<(), Error> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let value = self.get_target_value(target, size, Used::Twice)?;

        let mut overflow = false;
        let mut pair = (value, false);
        let mut previous_msb = get_msb(pair.0, size);
        for _ in 0..count {
            pair = shift_operation(pair.0, size, shift_dir, true);
            if get_msb(pair.0, size) != previous_msb {
                overflow = true;
            }
            previous_msb = get_msb(pair.0, size);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;

        let carry = match shift_dir {
            ShiftDirection::Left => pair.1,
            ShiftDirection::Right => if count < size.in_bits() { pair.1 } else { false }
        };

        // Adjust flags
        self.set_logic_flags(pair.0, size);
        self.set_flag(Flags::Overflow, overflow);
        if count != 0 {
            self.set_flag(Flags::Extend, carry);
            self.set_flag(Flags::Carry, carry);
        } else {
            self.set_flag(Flags::Carry, false);
        }
        Ok(())
    }

    #[inline]
    fn execute_bcc(&mut self, cond: Condition, offset: i32) -> Result<(), Error> {
        let should_branch = self.get_current_condition(cond);
        if should_branch {
            if let Err(err) = self.set_pc((self.decoder.start + 2).wrapping_add(offset as u32)) {
                self.state.pc -= 2;
                return Err(err);
            }
        }
        Ok(())
    }

    #[inline]
    fn execute_bra(&mut self, offset: i32) -> Result<(), Error> {
        if let Err(err) = self.set_pc((self.decoder.start + 2).wrapping_add(offset as u32)) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    #[inline]
    fn execute_bsr(&mut self, offset: i32) -> Result<(), Error> {
        self.push_long(self.state.pc)?;
        let sp = *self.get_stack_pointer_mut();
        self.debugger.stack_tracer.push_return(sp);
        if let Err(err) = self.set_pc((self.decoder.start + 2).wrapping_add(offset as u32)) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    #[inline]
    fn execute_bchg(&mut self, bitnum: Target, target: Target, size: Size) -> Result<(), Error> {
        let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
        let mut src_val = self.get_target_value(target, size, Used::Twice)?;
        let mask = self.set_bit_test_flags(src_val, bitnum, size);
        src_val = (src_val & !mask) | (!(src_val & mask) & mask);
        self.set_target_value(target, src_val, size, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_bclr(&mut self, bitnum: Target, target: Target, size: Size) -> Result<(), Error> {
        let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
        let mut src_val = self.get_target_value(target, size, Used::Twice)?;
        let mask = self.set_bit_test_flags(src_val, bitnum, size);
        src_val &= !mask;
        self.set_target_value(target, src_val, size, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_bset(&mut self, bitnum: Target, target: Target, size: Size) -> Result<(), Error> {
        let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
        let mut value = self.get_target_value(target, size, Used::Twice)?;
        let mask = self.set_bit_test_flags(value, bitnum, size);
        value |= mask;
        self.set_target_value(target, value, size, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_btst(&mut self, bitnum: Target, target: Target, size: Size) -> Result<(), Error> {
        let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
        let value = self.get_target_value(target, size, Used::Once)?;
        self.set_bit_test_flags(value, bitnum, size);
        Ok(())
    }

    #[inline]
    fn execute_bfchg(&mut self, target: Target, offset: RegOrImmediate, width: RegOrImmediate) -> Result<(), Error> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Twice)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        self.set_target_value(target, (value & !mask) | (!field & mask), Size::Long, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_bfclr(&mut self, target: Target, offset: RegOrImmediate, width: RegOrImmediate) -> Result<(), Error> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Twice)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        self.set_target_value(target, value & !mask, Size::Long, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_bfexts(&mut self, target: Target, offset: RegOrImmediate, width: RegOrImmediate, reg: Register) -> Result<(), Error> {
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
        Ok(())
    }

    #[inline]
    fn execute_bfextu(&mut self, target: Target, offset: RegOrImmediate, width: RegOrImmediate, reg: Register) -> Result<(), Error> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Once)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        self.state.d_reg[reg as usize] = field >> (32 - offset - width);
        Ok(())
    }

    #[inline]
    fn execute_bfset(&mut self, target: Target, offset: RegOrImmediate, width: RegOrImmediate) -> Result<(), Error> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Twice)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        self.set_target_value(target, value | mask, Size::Long, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_bftst(&mut self, target: Target, offset: RegOrImmediate, width: RegOrImmediate) -> Result<(), Error> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Once)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        Ok(())
    }

    #[inline]
    fn execute_chk(&mut self, target: Target, reg: Register, size: Size) -> Result<(), Error> {
        let upper_bound = sign_extend_to_long(self.get_target_value(target, size, Used::Once)?, size);
        let dreg = sign_extend_to_long(self.state.d_reg[reg as usize], size);

        self.set_sr(self.state.sr & 0xFFF0);
        if dreg < 0 || dreg > upper_bound {
            if dreg < 0 {
                self.set_flag(Flags::Negative, true);
            } else if dreg > upper_bound {
                self.set_flag(Flags::Negative, false);
            }
            self.exception(Exceptions::ChkInstruction as u8, false)?;
        }
        Ok(())
    }

    #[inline]
    fn execute_clr(&mut self, target: Target, size: Size) -> Result<(), Error> {
        if self.cputype == M68kType::MC68000 {
            self.get_target_value(target, size, Used::Twice)?;
            self.set_target_value(target, 0, size, Used::Twice)?;
        } else {
            self.set_target_value(target, 0, size, Used::Once)?;
        }
        // Clear flags except Zero flag
        self.state.sr = (self.state.sr & 0xFFF0) | (Flags::Zero as u16);
        Ok(())
    }

    #[inline]
    fn execute_cmp(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Once)?;
        let (result, carry) = overflowing_sub_sized(dest_val, src_val, size);
        let overflow = get_sub_overflow(dest_val, src_val, result, size);
        self.set_compare_flags(result, size, carry, overflow);
        Ok(())
    }

    #[inline]
    fn execute_cmpa(&mut self, src: Target, reg: Register, size: Size) -> Result<(), Error> {
        let src_val = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
        let dest_val = *self.get_a_reg_mut(reg);
        let (result, carry) = overflowing_sub_sized(dest_val, src_val, Size::Long);
        let overflow = get_sub_overflow(dest_val, src_val, result, Size::Long);
        self.set_compare_flags(result, Size::Long, carry, overflow);
        Ok(())
    }

    #[inline]
    fn execute_dbcc(&mut self, cond: Condition, reg: Register, offset: i16) -> Result<(), Error> {
        let condition_true = self.get_current_condition(cond);
        if !condition_true {
            let next = ((get_value_sized(self.state.d_reg[reg as usize], Size::Word) as u16) as i16).wrapping_sub(1);
            set_value_sized(&mut self.state.d_reg[reg as usize], next as u32, Size::Word);
            if next != -1 {
                if let Err(err) = self.set_pc((self.decoder.start + 2).wrapping_add(offset as u32)) {
                    self.state.pc -= 2;
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    #[inline]
    fn execute_divw(&mut self, src: Target, dest: Register, sign: Sign) -> Result<(), Error> {
        let src_val = self.get_target_value(src, Size::Word, Used::Once)?;
        if src_val == 0 {
            self.exception(Exceptions::ZeroDivide as u8, false)?;
            return Ok(());
        }

        let dest_val = get_value_sized(self.state.d_reg[dest as usize], Size::Long);
        let (remainder, quotient, overflow) = match sign {
            Sign::Signed => {
                let dest_val = dest_val as i32;
                let src_val = sign_extend_to_long(src_val, Size::Word);
                let quotient = dest_val / src_val;
                (
                    (dest_val % src_val) as u32,
                    quotient as u32,
                    quotient > i16::MAX as i32 || quotient < i16::MIN as i32
                )
            },
            Sign::Unsigned => {
                let quotient = dest_val / src_val;
                (
                    dest_val % src_val,
                    quotient,
                    (quotient & 0xFFFF0000) != 0
                )
            },
        };

        // Only update the register if the quotient was large than a 16-bit number
        if !overflow {
            self.set_compare_flags(quotient, Size::Word, false, false);
            self.state.d_reg[dest as usize] = (remainder << 16) | (0xFFFF & quotient);
        } else {
            self.set_flag(Flags::Carry, false);
            self.set_flag(Flags::Overflow, true);
        }
        Ok(())
    }

    #[inline]
    fn execute_divl(&mut self, src: Target, dest_h: Option<Register>, dest_l: Register, sign: Sign) -> Result<(), Error> {
        let src_val = self.get_target_value(src, Size::Long, Used::Once)?;
        if src_val == 0 {
            self.exception(Exceptions::ZeroDivide as u8, false)?;
            return Ok(());
        }

        let existing_l = self.state.d_reg[dest_l as usize];
        let (remainder, quotient) = match sign {
            Sign::Signed => {
                let src_val = (src_val as i32) as i64;
                let dest_val = match dest_h {
                    Some(reg) => (((self.state.d_reg[reg as usize] as u64) << 32) | (existing_l as u64)) as i64,
                    None => (existing_l as i32) as i64,
                };
                ((dest_val % src_val) as u64, (dest_val / src_val) as u64)
            },
            Sign::Unsigned => {
                let src_val = src_val as u64;
                let existing_h = dest_h.map(|reg| self.state.d_reg[reg as usize]).unwrap_or(0);
                let dest_val = ((existing_h as u64) << 32) | (existing_l as u64);
                (dest_val % src_val, dest_val / src_val)
            },
        };

        self.set_compare_flags(quotient as u32, Size::Long, false, (quotient & 0xFFFFFFFF00000000) != 0);
        if let Some(dest_h) = dest_h {
            self.state.d_reg[dest_h as usize] = remainder as u32;
        }
        self.state.d_reg[dest_l as usize] = quotient as u32;
        Ok(())
    }

    #[inline]
    fn execute_eor(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let result = get_value_sized(dest_val ^ src_val, size);
        self.set_target_value(dest, result, size, Used::Twice)?;
        self.set_logic_flags(result, size);
        Ok(())
    }

    #[inline]
    fn execute_eor_to_ccr(&mut self, value: u8) -> Result<(), Error> {
        self.set_sr((self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) ^ (value as u16)));
        Ok(())
    }

    #[inline]
    fn execute_eor_to_sr(&mut self, value: u16) -> Result<(), Error> {
        self.require_supervisor()?;
        self.set_sr(self.state.sr ^ value);
        Ok(())
    }

    #[inline]
    fn execute_exg(&mut self, target1: Target, target2: Target) -> Result<(), Error> {
        let value1 = self.get_target_value(target1, Size::Long, Used::Twice)?;
        let value2 = self.get_target_value(target2, Size::Long, Used::Twice)?;
        self.set_target_value(target1, value2, Size::Long, Used::Twice)?;
        self.set_target_value(target2, value1, Size::Long, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_ext(&mut self, reg: Register, from_size: Size, to_size: Size) -> Result<(), Error> {
        let input = get_value_sized(self.state.d_reg[reg as usize], from_size);
        let result = match (from_size, to_size) {
            (Size::Byte, Size::Word) => ((((input as u8) as i8) as i16) as u16) as u32,
            (Size::Word, Size::Long) => (((input as u16) as i16) as i32) as u32,
            (Size::Byte, Size::Long) => (((input as u8) as i8) as i32) as u32,
            _ => panic!("Unsupported size for EXT instruction"),
        };
        set_value_sized(&mut self.state.d_reg[reg as usize], result, to_size);
        self.set_logic_flags(result, to_size);
        Ok(())
    }

    #[inline]
    fn execute_illegal(&mut self) -> Result<(), Error> {
        self.exception(Exceptions::IllegalInstruction as u8, false)?;
        Ok(())
    }

    #[inline]
    fn execute_jmp(&mut self, target: Target) -> Result<(), Error> {
        let addr = self.get_target_address(target)?;
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    #[inline]
    fn execute_jsr(&mut self, target: Target) -> Result<(), Error> {
        let previous_pc = self.state.pc;
        let addr = self.get_target_address(target)?;
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }

        // If the address is good, then push the old PC onto the stack
        self.push_long(previous_pc)?;
        let sp = *self.get_stack_pointer_mut();
        self.debugger.stack_tracer.push_return(sp);
        Ok(())
    }

    #[inline]
    fn execute_lea(&mut self, target: Target, reg: Register) -> Result<(), Error> {
        let value = self.get_target_address(target)?;
        let addr = self.get_a_reg_mut(reg);
        *addr = value;
        Ok(())
    }

    #[inline]
    fn execute_link(&mut self, reg: Register, offset: i32) -> Result<(), Error> {
        *self.get_stack_pointer_mut() -= 4;
        let sp = *self.get_stack_pointer_mut();
        let value = *self.get_a_reg_mut(reg);
        self.set_address_sized(sp as Address, value, Size::Long)?;
        *self.get_a_reg_mut(reg) = sp;
        *self.get_stack_pointer_mut() = (sp as i32).wrapping_add(offset) as u32;
        Ok(())
    }

    #[inline]
    fn execute_lsd(&mut self, count: Target, target: Target, size: Size, shift_dir: ShiftDirection) -> Result<(), Error> {
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
        Ok(())
    }

    #[inline]
    fn execute_move(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        self.set_logic_flags(src_val, size);
        self.set_target_value(dest, src_val, size, Used::Once)?;
        Ok(())
    }

    #[inline]
    fn execute_movea(&mut self, src: Target, reg: Register, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let src_val = sign_extend_to_long(src_val, size) as u32;
        let addr = self.get_a_reg_mut(reg);
        *addr = src_val;
        Ok(())
    }

    #[inline]
    fn execute_move_from_sr(&mut self, target: Target) -> Result<(), Error> {
        self.require_supervisor()?;
        self.set_target_value(target, self.state.sr as u32, Size::Word, Used::Once)?;
        Ok(())
    }

    #[inline]
    fn execute_move_to_sr(&mut self, target: Target) -> Result<(), Error> {
        self.require_supervisor()?;
        let value = self.get_target_value(target, Size::Word, Used::Once)? as u16;
        self.set_sr(value);
        Ok(())
    }

    #[inline]
    fn execute_move_to_ccr(&mut self, target: Target) -> Result<(), Error> {
        let value = self.get_target_value(target, Size::Word, Used::Once)? as u16;
        self.set_sr((self.state.sr & 0xFF00) | (value & 0x00FF));
        Ok(())
    }

    #[inline]
    fn execute_movec(&mut self, target: Target, control_reg: ControlRegister, dir: Direction) -> Result<(), Error> {
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
        Ok(())
    }

    #[inline]
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

    #[inline]
    fn move_memory_to_registers(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, Error> {
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

    #[inline]
    fn move_registers_to_memory(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, Error> {
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

    #[inline]
    fn move_registers_to_memory_reverse(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, Error> {
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

    #[inline]
    fn execute_movep(&mut self, dreg: Register, areg: Register, offset: i16, size: Size, dir: Direction) -> Result<(), Error> {
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
        Ok(())
    }

    #[inline]
    fn execute_moveq(&mut self, data: u8, reg: Register) -> Result<(), Error> {
        let value = sign_extend_to_long(data as u32, Size::Byte) as u32;
        self.state.d_reg[reg as usize] = value;
        self.set_logic_flags(value, Size::Long);
        Ok(())
    }

    #[inline]
    fn execute_moveusp(&mut self, target: Target, dir: Direction) -> Result<(), Error> {
        self.require_supervisor()?;
        match dir {
            Direction::ToTarget => self.set_target_value(target, self.state.usp, Size::Long, Used::Once)?,
            Direction::FromTarget => { self.state.usp = self.get_target_value(target, Size::Long, Used::Once)?; },
        }
        Ok(())
    }

    #[inline]
    fn execute_mulw(&mut self, src: Target, dest: Register, sign: Sign) -> Result<(), Error> {
        let src_val = self.get_target_value(src, Size::Word, Used::Once)?;
        let dest_val = get_value_sized(self.state.d_reg[dest as usize], Size::Word);
        let result = match sign {
            Sign::Signed => ((((dest_val as u16) as i16) as i64) * (((src_val as u16) as i16) as i64)) as u64,
            Sign::Unsigned => dest_val as u64 * src_val as u64,
        };

        self.set_compare_flags(result as u32, Size::Long, false, false);
        self.state.d_reg[dest as usize] = result as u32;
        Ok(())
    }

    #[inline]
    fn execute_mull(&mut self, src: Target, dest_h: Option<Register>, dest_l: Register, sign: Sign) -> Result<(), Error> {
        let src_val = self.get_target_value(src, Size::Long, Used::Once)?;
        let dest_val = get_value_sized(self.state.d_reg[dest_l as usize], Size::Long);
        let result = match sign {
            Sign::Signed => (((dest_val as i32) as i64) * ((src_val as i32) as i64)) as u64,
            Sign::Unsigned => dest_val as u64 * src_val as u64,
        };

        self.set_compare_flags(result as u32, Size::Long, false, false);
        if let Some(dest_h) = dest_h {
            self.state.d_reg[dest_h as usize] = (result >> 32) as u32;
        }
        self.state.d_reg[dest_l as usize] = (result & 0x00000000FFFFFFFF) as u32;
        Ok(())
    }

    #[inline]
    fn execute_nbcd(&mut self, dest: Target) -> Result<(), Error> {
        let dest_val = self.get_target_value(dest, Size::Byte, Used::Twice)?;
        let result = self.execute_sbcd_val(dest_val, 0)?;
        self.set_target_value(dest, result, Size::Byte, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_neg(&mut self, target: Target, size: Size) -> Result<(), Error> {
        let original = self.get_target_value(target, size, Used::Twice)?;
        let (result, overflow) = overflowing_sub_signed_sized(0, original, size);
        let carry = result != 0;
        self.set_target_value(target, result, size, Used::Twice)?;
        self.set_compare_flags(result, size, carry, overflow);
        self.set_flag(Flags::Extend, carry);
        Ok(())
    }

    #[inline]
    fn execute_negx(&mut self, dest: Target, size: Size) -> Result<(), Error> {
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let extend = self.get_flag(Flags::Extend) as u32;
        let (result1, carry1) = overflowing_sub_sized(0, dest_val, size);
        let (result2, carry2) = overflowing_sub_sized(result1, extend, size);
        let overflow = get_sub_overflow(0, dest_val, result2, size);

        // Handle flags
        let zero = self.get_flag(Flags::Zero);
        self.set_compare_flags(result2, size, carry1 || carry2, overflow);
        if self.get_flag(Flags::Zero) {
            // NEGX can only clear the zero flag, so if it's set, restore it to whatever it was before
            self.set_flag(Flags::Zero, zero);
        }
        self.set_flag(Flags::Extend, carry1 || carry2);

        self.set_target_value(dest, result2, size, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_not(&mut self, target: Target, size: Size) -> Result<(), Error> {
        let mut value = self.get_target_value(target, size, Used::Twice)?;
        value = get_value_sized(!value, size);
        self.set_target_value(target, value, size, Used::Twice)?;
        self.set_logic_flags(value, size);
        Ok(())
    }

    #[inline]
    fn execute_or(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let result = get_value_sized(dest_val | src_val, size);
        self.set_target_value(dest, result, size, Used::Twice)?;
        self.set_logic_flags(result, size);
        Ok(())
    }

    #[inline]
    fn execute_or_to_ccr(&mut self, value: u8) -> Result<(), Error> {
        self.set_sr((self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) | (value as u16)));
        Ok(())
    }

    #[inline]
    fn execute_or_to_sr(&mut self, value: u16) -> Result<(), Error> {
        self.require_supervisor()?;
        self.set_sr(self.state.sr | value);
        Ok(())
    }

    #[inline]
    fn execute_pea(&mut self, target: Target) -> Result<(), Error> {
        let value = self.get_target_address(target)?;
        self.push_long(value)?;
        Ok(())
    }

    #[inline]
    fn execute_reset(&mut self) -> Result<(), Error> {
        self.require_supervisor()?;
        // TODO this only resets external devices and not internal ones
        Ok(())
    }

    #[inline]
    fn execute_rod(&mut self, count: Target, target: Target, size: Size, shift_dir: ShiftDirection) -> Result<(), Error> {
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
        Ok(())
    }

    #[inline]
    fn execute_roxd(&mut self, count: Target, target: Target, size: Size, shift_dir: ShiftDirection) -> Result<(), Error> {
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
        Ok(())
    }

    #[inline]
    fn execute_rte(&mut self) -> Result<(), Error> {
        self.require_supervisor()?;
        let sr = self.pop_word()?;
        let addr = self.pop_long()?;

        if self.cputype >= M68kType::MC68010 {
            let _ = self.pop_word()?;
        }

        self.set_sr(sr);
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    #[inline]
    fn execute_rtr(&mut self) -> Result<(), Error> {
        let ccr = self.pop_word()?;
        let addr = self.pop_long()?;
        self.set_sr((self.state.sr & 0xFF00) | (ccr & 0x00FF));
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    #[inline]
    fn execute_rts(&mut self) -> Result<(), Error> {
        self.debugger.stack_tracer.pop_return();
        let addr = self.pop_long()?;
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    #[inline]
    fn execute_scc(&mut self, cond: Condition, target: Target) -> Result<(), Error> {
        let condition_true = self.get_current_condition(cond);
        if condition_true {
            self.set_target_value(target, 0xFF, Size::Byte, Used::Once)?;
        } else {
            self.set_target_value(target, 0x00, Size::Byte, Used::Once)?;
        }
        Ok(())
    }

    #[inline]
    fn execute_stop(&mut self, flags: u16) -> Result<(), Error> {
        self.require_supervisor()?;
        self.set_sr(flags);
        self.state.status = Status::Stopped;
        Ok(())
    }

    #[inline]
    fn execute_sbcd(&mut self, src: Target, dest: Target) -> Result<(), Error> {
        let src_val = self.get_target_value(src, Size::Byte, Used::Once)?;
        let dest_val = self.get_target_value(dest, Size::Byte, Used::Twice)?;
        let result = self.execute_sbcd_val(src_val, dest_val)?;
        self.set_target_value(dest, result, Size::Byte, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_sbcd_val(&mut self, src_val: u32, dest_val: u32) -> Result<u32, Error> {
        let extend_flag = self.get_flag(Flags::Extend) as u32;
        let src_parts = get_nibbles_from_byte(src_val);
        let dest_parts = get_nibbles_from_byte(dest_val);

        let binary_result = dest_val.wrapping_sub(src_val).wrapping_sub(extend_flag);
        let mut result = dest_parts.1.wrapping_sub(src_parts.1).wrapping_sub(extend_flag);
        if (result & 0x1F) > 0x09 { result -= 0x06 };
        result = result.wrapping_add(dest_parts.0.wrapping_sub(src_parts.0));
        let carry = (result & 0x1FF) > 0x99;
        if carry { result -= 0x60 };

        self.set_flag(Flags::Negative, get_msb(result, Size::Byte));
        self.set_flag(Flags::Zero, (result & 0xFF) == 0);
        self.set_flag(Flags::Overflow, (binary_result & !result & 0x80) != 0);
        self.set_flag(Flags::Carry, carry);
        self.set_flag(Flags::Extend, carry);

        Ok(result)
    }

    #[inline]
    fn execute_sub(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let (result, carry) = overflowing_sub_sized(dest_val, src_val, size);
        let overflow = get_sub_overflow(dest_val, src_val, result, size);
        self.set_compare_flags(result, size, carry, overflow);
        self.set_flag(Flags::Extend, carry);
        self.set_target_value(dest, result, size, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_suba(&mut self, src: Target, dest: Register, size: Size) -> Result<(), Error> {
        let src_val = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
        let dest_val = *self.get_a_reg_mut(dest);
        let (result, _) = overflowing_sub_sized(dest_val, src_val, Size::Long);
        *self.get_a_reg_mut(dest) = result;
        Ok(())
    }

    #[inline]
    fn execute_subx(&mut self, src: Target, dest: Target, size: Size) -> Result<(), Error> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let extend = self.get_flag(Flags::Extend) as u32;
        let (result1, carry1) = overflowing_sub_sized(dest_val, src_val, size);
        let (result2, carry2) = overflowing_sub_sized(result1, extend, size);
        let overflow = get_sub_overflow(dest_val, src_val, result2, size);

        // Handle flags
        let zero = self.get_flag(Flags::Zero);
        self.set_compare_flags(result2, size, carry1 || carry2, overflow);
        if self.get_flag(Flags::Zero) {
            // SUBX can only clear the zero flag, so if it's set, restore it to whatever it was before
            self.set_flag(Flags::Zero, zero);
        }
        self.set_flag(Flags::Extend, carry1 || carry2);

        self.set_target_value(dest, result2, size, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_swap(&mut self, reg: Register) -> Result<(), Error> {
        let value = self.state.d_reg[reg as usize];
        self.state.d_reg[reg as usize] = ((value & 0x0000FFFF) << 16) | ((value & 0xFFFF0000) >> 16);
        self.set_logic_flags(self.state.d_reg[reg as usize], Size::Long);
        Ok(())
    }

    #[inline]
    fn execute_tas(&mut self, target: Target) -> Result<(), Error> {
        let value = self.get_target_value(target, Size::Byte, Used::Twice)?;
        self.set_flag(Flags::Negative, (value & 0x80) != 0);
        self.set_flag(Flags::Zero, value == 0);
        self.set_flag(Flags::Overflow, false);
        self.set_flag(Flags::Carry, false);
        self.set_target_value(target, value | 0x80, Size::Byte, Used::Twice)?;
        Ok(())
    }

    #[inline]
    fn execute_tst(&mut self, target: Target, size: Size) -> Result<(), Error> {
        let value = self.get_target_value(target, size, Used::Once)?;
        self.set_logic_flags(value, size);
        Ok(())
    }

    #[inline]
    fn execute_trap(&mut self, number: u8) -> Result<(), Error> {
        self.exception(32 + number, false)?;
        Ok(())
    }

    #[inline]
    fn execute_trapv(&mut self) -> Result<(), Error> {
        if self.get_flag(Flags::Overflow) {
            self.exception(Exceptions::TrapvInstruction as u8, false)?;
        }
        Ok(())
    }

    #[inline]
    fn execute_unlk(&mut self, reg: Register) -> Result<(), Error> {
        let value = *self.get_a_reg_mut(reg);
        *self.get_stack_pointer_mut() = value;
        let new_value = self.pop_long()?;
        let addr = self.get_a_reg_mut(reg);
        *addr = new_value;
        Ok(())
    }

    #[inline]
    fn execute_unimplemented_a(&mut self, _: u16) -> Result<(), Error> {
        self.state.pc -= 2;
        self.exception(Exceptions::LineAEmulator as u8, false)?;
        Ok(())
    }

    #[inline]
    fn execute_unimplemented_f(&mut self, _: u16) -> Result<(), Error> {
        self.state.pc -= 2;
        self.exception(Exceptions::LineFEmulator as u8, false)?;
        Ok(())
    }


    pub(super) fn get_target_value(&mut self, target: Target, size: Size, used: Used) -> Result<u32, Error> {
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

    pub(super) fn set_target_value(&mut self, target: Target, value: u32, size: Size, used: Used) -> Result<(), Error> {
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

    fn get_target_address(&mut self, target: Target) -> Result<u32, Error> {
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

    fn post_increment_areg_target(&mut self, reg: Register, mut size: Size, used: Used) -> u32 {
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

    fn pre_decrement_areg_target(&mut self, reg: Register, mut size: Size, used: Used) -> u32 {
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

    fn get_address_sized(&mut self, addr: Address, size: Size) -> Result<u32, Error> {
        self.start_request(addr as u32, size, MemAccess::Read, MemType::Data, false)?;
        match size {
            Size::Byte => self.port.read_u8(addr).map(|value| value as u32),
            Size::Word => self.port.read_beu16(addr).map(|value| value as u32),
            Size::Long => self.port.read_beu32(addr),
        }
    }

    fn set_address_sized(&mut self, addr: Address, value: u32, size: Size) -> Result<(), Error> {
        self.start_request(addr as u32, size, MemAccess::Write, MemType::Data, false)?;
        match size {
            Size::Byte => self.port.write_u8(addr, value as u8),
            Size::Word => self.port.write_beu16(addr, value as u16),
            Size::Long => self.port.write_beu32(addr, value),
        }
    }

    fn start_instruction_request(&mut self, addr: u32) -> Result<u32, Error> {
        self.state.request.i_n_bit = false;
        self.state.request.code = FunctionCode::program(self.state.sr);
        self.state.request.access = MemAccess::Read;
        self.state.request.address = addr;

        validate_address(addr)
    }

    fn start_request(&mut self, addr: u32, size: Size, access: MemAccess, mtype: MemType, i_n_bit: bool) -> Result<u32, Error> {
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
                Size::Long => (value << 1, bit),
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
                Size::Long => (mask | value << 1, bit),
            }
        },
        ShiftDirection::Right => {
            let bit = (value & 0x01) != 0;
            let mask = if use_extend.unwrap_or(bit) { get_msb_mask(0xffffffff, size) } else { 0x0 };
            ((value >> 1) | mask, bit)
        },
    }
}

fn get_nibbles_from_byte(value: u32) -> (u32, u32) {
    (value & 0xF0, value & 0x0F)
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


