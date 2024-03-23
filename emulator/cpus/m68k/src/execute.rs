// Instruction Execution

use emulator_hal::time;
use emulator_hal::step::Step;
use emulator_hal::bus::{self, BusAccess};

use crate::{M68k, M68kType, M68kError, M68kState};
use crate::state::{Status, Flags, Exceptions, InterruptPriority};
use crate::memory::{MemType, MemAccess, M68kBusPort, M68kAddress};
use crate::decode::M68kDecoder;
use crate::debugger::M68kDebugger;
use crate::timing::M68kInstructionTiming;
use crate::instructions::{
    Register, Size, Sign, Direction, XRegister, BaseRegister, IndexRegister, RegOrImmediate, ControlRegister, Condition, Target,
    Instruction, sign_extend_to_long,
};


const DEV_NAME: &str = "m68k-cpu";

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Used {
    Once,
    Twice,
}


#[derive(Clone, Debug)]
pub struct M68kCycle<Instant> {
    pub decoder: M68kDecoder<Instant>,
    pub timing: M68kInstructionTiming,
    pub memory: M68kBusPort<Instant>,
    pub current_clock: Instant,
}

impl<Instant> M68kCycle<Instant>
where
    Instant: time::Instant,
{
    #[inline]
    pub fn default(cputype: M68kType, data_width: u8) -> Self {
        Self {
            decoder: M68kDecoder::new(cputype, true, 0),
            timing: M68kInstructionTiming::new(cputype, data_width),
            memory: M68kBusPort::default(),
            current_clock: Instant::START,
        }
    }

    #[inline]
    pub fn new(cpu: &M68k<Instant>, clock: Instant) -> Self {
        let is_supervisor = cpu.state.sr & (Flags::Supervisor as u16) != 0;
        Self {
            decoder: M68kDecoder::new(cpu.info.chip, is_supervisor, cpu.state.pc),
            timing: M68kInstructionTiming::new(cpu.info.chip, cpu.info.data_width as u8),
            memory: M68kBusPort::from_info(&cpu.info, clock),
            current_clock: clock,
        }
    }

    #[inline]
    pub fn begin<Bus>(self, cpu: &mut M68k<Instant>, bus: Bus) -> M68kCycleExecutor<'_, Bus, Instant>
    where
        Bus: BusAccess<M68kAddress, Instant = Instant>,
    {
        cpu.stats.cycle_number = cpu.stats.cycle_number.wrapping_add(1);

        M68kCycleExecutor {
            state: &mut cpu.state,
            bus,
            debugger: &mut cpu.debugger,
            cycle: self,
        }
    }
}

impl<Bus, BusError, Instant> Step<M68kAddress, Bus> for M68k<Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant, Error = BusError>,
    BusError: bus::Error,
    Instant: time::Instant,
{
    type Error = M68kError<BusError>;

    fn is_running(&mut self) -> bool {
        self.state.status == Status::Running
    }

    fn reset(&mut self, _now: Instant, _bus: &mut Bus) -> Result<(), Self::Error> {
        Ok(())
    }

    fn step(&mut self, now: Instant, bus: &mut Bus) -> Result<Instant, Self::Error> {
        let cycle = M68kCycle::new(self, now);

        let mut executor = cycle.begin(self, &mut *bus);
        executor.check_breakpoints()?;
        executor.step()?;

        //let interrupt = system.get_interrupt_controller().check();
        //if let (priority, Some(ack)) = executor.check_pending_interrupts(interrupt)? {
        //    log::debug!("interrupt: {:?} @ {} ns", priority, system.clock.as_duration().as_nanos());
        //    system.get_interrupt_controller().acknowledge(priority as u8)?;
        //}

        self.cycle = Some(executor.end());
        Ok(now + self.last_cycle_duration())
    }
}

pub struct M68kCycleExecutor<'a, Bus, Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant>,
{
    pub state: &'a mut M68kState,
    pub bus: Bus,
    pub debugger: &'a mut M68kDebugger,
    pub cycle: M68kCycle<Instant>,
}

impl<'a, Bus, Instant> M68kCycleExecutor<'a, Bus, Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant>,
    Instant: Copy,
{
    pub fn end(self) -> M68kCycle<Instant> {
        self.cycle
    }
}

impl<'a, Bus, Instant> M68kCycleExecutor<'a, Bus, Instant>
where
    Bus: BusAccess<M68kAddress, Instant = Instant>,
    Instant: Copy,
{
    #[inline]
    pub fn step(&mut self) -> Result<(), M68kError<Bus::Error>> {
        match self.state.status {
            Status::Init => self.reset_cpu(),
            Status::Stopped => Err(M68kError::Halted),
            Status::Running => self.cycle_one(),
        }?;
        Ok(())
    }

    #[inline]
    pub fn reset_cpu(&mut self) -> Result<(), M68kError<Bus::Error>> {
        self.state.ssp = self.get_address_sized(0, Size::Long)?;
        self.state.pc = self.get_address_sized(4, Size::Long)?;
        self.state.status = Status::Running;
        self.cycle.timing.performed_reset();
        Ok(())
    }

    #[inline]
    pub fn cycle_one(&mut self) -> Result<(), M68kError<Bus::Error>> {
        self.check_breakpoints()?;

        let result = self.decode_and_execute();
        self.process_error(result)?;

        // TODO this is called by the step function directly, but should be integrated better
        //self.check_pending_interrupts(system)?;
        Ok(())
    }

    #[inline]
    pub fn check_pending_interrupts(
        &mut self,
        interrupt: (bool, u8, u8),
    ) -> Result<(InterruptPriority, Option<u8>), M68kError<Bus::Error>> {
        let ack_num;
        (self.state.pending_ipl, ack_num) = match interrupt {
            (true, priority, ack) => (InterruptPriority::from_u8(priority), ack),
            (false, _, ack) => (InterruptPriority::NoInterrupt, ack),
        };

        let current_ipl = self.state.current_ipl as u8;
        let pending_ipl = self.state.pending_ipl as u8;

        if self.state.pending_ipl != InterruptPriority::NoInterrupt {
            let priority_mask = ((self.state.sr & Flags::IntMask as u16) >> 8) as u8;

            if (pending_ipl > priority_mask || pending_ipl == 7) && pending_ipl >= current_ipl {
                //log::debug!("{} interrupt: {} @ {} ns", DEV_NAME, pending_ipl, system.clock.as_duration().as_nanos());
                self.state.current_ipl = self.state.pending_ipl;
                //let acknowledge = self.state.current_ipl;
                //let ack_num = system.get_interrupt_controller().acknowledge(self.state.current_ipl as u8)?;
                self.exception(ack_num, true)?;
                return Ok((self.state.current_ipl, Some(ack_num)));
            }
        }

        if pending_ipl < current_ipl {
            self.state.current_ipl = self.state.pending_ipl;
        }

        Ok((self.state.current_ipl, None))
    }

    pub fn exception(&mut self, number: u8, is_interrupt: bool) -> Result<(), M68kError<Bus::Error>> {
        log::debug!("{}: raising exception {}", DEV_NAME, number);

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

    fn setup_group0_exception(&mut self, number: u8) -> Result<(), M68kError<Bus::Error>> {
        let sr = self.state.sr;
        let ins_word = self.cycle.decoder.instruction_word;
        let extra_code = self.cycle.memory.request.get_type_code();
        let fault_size = self.cycle.memory.request.size.in_bytes();
        let fault_address = self.cycle.memory.request.address;

        // Changes to the flags must happen after the previous value has been pushed to the stack
        self.set_flag(Flags::Supervisor, true);
        self.set_flag(Flags::Tracing, false);

        let offset = (number as u16) << 2;
        if self.cycle.decoder.cputype >= M68kType::MC68010 {
            self.push_word(offset)?;
        }

        self.push_long(self.state.pc - fault_size)?;
        self.push_word(sr)?;
        self.push_word(ins_word)?;
        self.push_long(fault_address)?;
        self.push_word((ins_word & 0xFFF0) | extra_code)?;

        let vector = self.state.vbr + offset as u32;
        let addr = self.get_address_sized(vector, Size::Long)?;
        self.set_pc(addr)?;

        Ok(())
    }

    fn setup_normal_exception(&mut self, number: u8, is_interrupt: bool) -> Result<(), M68kError<Bus::Error>> {
        let sr = self.state.sr;
        self.cycle.memory.request.i_n_bit = true;

        // Changes to the flags must happen after the previous value has been pushed to the stack
        self.set_flag(Flags::Supervisor, true);
        self.set_flag(Flags::Tracing, false);
        if is_interrupt {
            self.state.sr = (self.state.sr & !(Flags::IntMask as u16)) | ((self.state.current_ipl as u16) << 8);
        }

        let offset = (number as u16) << 2;
        if self.cycle.decoder.cputype >= M68kType::MC68010 {
            self.push_word(offset)?;
        }
        self.push_long(self.state.pc)?;
        self.push_word(sr)?;

        let vector = self.state.vbr + offset as u32;
        let addr = self.get_address_sized(vector, Size::Long)?;
        self.set_pc(addr)?;

        Ok(())
    }

    #[inline]
    pub fn process_error(&mut self, result: Result<(), M68kError<Bus::Error>>) -> Result<(), M68kError<Bus::Error>> {
        match result {
            Ok(value) => Ok(value),
            Err(M68kError::Exception(ex)) => {
                self.exception(ex as u8, false)?;
                Ok(())
            },
            Err(M68kError::Interrupt(ex)) => {
                self.exception(ex, false)?;
                Ok(())
            },
            Err(err) => Err(err),
        }
    }

    #[inline]
    pub fn decode_and_execute(&mut self) -> Result<(), M68kError<Bus::Error>> {
        self.decode_next()?;
        self.execute_current()?;
        Ok(())
    }

    #[inline]
    pub fn decode_next(&mut self) -> Result<(), M68kError<Bus::Error>> {
        let is_supervisor = self.is_supervisor();
        self.cycle
            .decoder
            .decode_at(&mut self.bus, &mut self.cycle.memory, is_supervisor, self.state.pc)?;

        self.cycle.timing.add_instruction(&self.cycle.decoder.instruction);

        self.state.pc = self.cycle.decoder.end;

        Ok(())
    }

    #[inline]
    pub fn execute_current(&mut self) -> Result<(), M68kError<Bus::Error>> {
        match self.cycle.decoder.instruction {
            Instruction::ABCD(src, dest) => self.execute_abcd(src, dest),
            Instruction::ADD(src, dest, size) => self.execute_add(src, dest, size),
            Instruction::ADDA(src, dest, size) => self.execute_adda(src, dest, size),
            Instruction::ADDX(src, dest, size) => self.execute_addx(src, dest, size),
            Instruction::AND(src, dest, size) => self.execute_and(src, dest, size),
            Instruction::ANDtoCCR(value) => self.execute_and_to_ccr(value),
            Instruction::ANDtoSR(value) => self.execute_and_to_sr(value),
            Instruction::ASL(count, target, size) => self.execute_asl(count, target, size),
            Instruction::ASR(count, target, size) => self.execute_asr(count, target, size),
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
            Instruction::LSL(count, target, size) => self.execute_lsl(count, target, size),
            Instruction::LSR(count, target, size) => self.execute_lsr(count, target, size),
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
            Instruction::ROL(count, target, size) => self.execute_rol(count, target, size),
            Instruction::ROR(count, target, size) => self.execute_ror(count, target, size),
            Instruction::ROXL(count, target, size) => self.execute_roxl(count, target, size),
            Instruction::ROXR(count, target, size) => self.execute_roxr(count, target, size),
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
            _ => {
                return Err(M68kError::Other("Unsupported instruction".to_string()));
            },
        }?;

        Ok(())
    }

    fn execute_abcd(&mut self, src: Target, dest: Target) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, Size::Byte, Used::Once)?;
        let dest_val = self.get_target_value(dest, Size::Byte, Used::Twice)?;

        let extend_flag = self.get_flag(Flags::Extend) as u32;
        let src_parts = get_nibbles_from_byte(src_val);
        let dest_parts = get_nibbles_from_byte(dest_val);

        let binary_result = src_val.wrapping_add(dest_val).wrapping_add(extend_flag);
        let mut result = src_parts.1.wrapping_add(dest_parts.1).wrapping_add(extend_flag);
        if result > 0x09 {
            result = result.wrapping_add(0x06)
        };
        result += src_parts.0 + dest_parts.0;
        if result > 0x99 {
            result = result.wrapping_add(0x60)
        };
        let carry = (result & 0xFFFFFF00) != 0;

        self.set_target_value(dest, result, Size::Byte, Used::Twice)?;
        self.set_flag(Flags::Negative, get_msb(result, Size::Byte));
        self.set_flag(Flags::Zero, result == 0);
        self.set_flag(Flags::Overflow, (!binary_result & result & 0x80) != 0);
        self.set_flag(Flags::Carry, carry);
        self.set_flag(Flags::Extend, carry);
        Ok(())
    }

    fn execute_add(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let (result, carry) = overflowing_add_sized(dest_val, src_val, size);
        let overflow = get_add_overflow(dest_val, src_val, result, size);
        self.set_compare_flags(result, size, carry, overflow);
        self.set_flag(Flags::Extend, carry);
        self.set_target_value(dest, result, size, Used::Twice)?;
        Ok(())
    }

    fn execute_adda(&mut self, src: Target, dest: Register, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
        let dest_val = *self.get_a_reg_mut(dest);
        let (result, _) = overflowing_add_sized(dest_val, src_val, Size::Long);
        *self.get_a_reg_mut(dest) = result;
        Ok(())
    }

    fn execute_addx(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_and(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let result = get_value_sized(dest_val & src_val, size);
        self.set_target_value(dest, result, size, Used::Twice)?;
        self.set_logic_flags(result, size);
        Ok(())
    }

    fn execute_and_to_ccr(&mut self, value: u8) -> Result<(), M68kError<Bus::Error>> {
        self.state.sr = (self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) & (value as u16));
        Ok(())
    }

    fn execute_and_to_sr(&mut self, value: u16) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        self.set_sr(self.state.sr & value);
        Ok(())
    }

    fn execute_asl(&mut self, count: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let value = self.get_target_value(target, size, Used::Twice)?;

        let mut overflow = false;
        let mut pair = (value, false);
        let mut previous_msb = get_msb(pair.0, size);
        for _ in 0..count {
            pair = shift_left(pair.0, size);
            if get_msb(pair.0, size) != previous_msb {
                overflow = true;
            }
            previous_msb = get_msb(pair.0, size);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;

        self.set_arithmetic_shift_flags(pair.0, count, pair.1, overflow, size);
        Ok(())
    }

    fn execute_asr(&mut self, count: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let value = self.get_target_value(target, size, Used::Twice)?;

        let mut overflow = false;
        let mut pair = (value, false);
        let mut previous_msb = get_msb(pair.0, size);
        for _ in 0..count {
            pair = shift_right(pair.0, size, true);
            if get_msb(pair.0, size) != previous_msb {
                overflow = true;
            }
            previous_msb = get_msb(pair.0, size);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;

        let last_bit = if count < size.in_bits() { pair.1 } else { false };
        //let last_bit = if count < size.in_bits() { pair.1 } else { get_msb(value, size) };
        self.set_arithmetic_shift_flags(pair.0, count, last_bit, overflow, size);
        Ok(())
    }

    fn set_arithmetic_shift_flags(&mut self, result: u32, count: u32, last_bit_out: bool, overflow: bool, size: Size) {
        self.set_logic_flags(result, size);
        self.set_flag(Flags::Overflow, overflow);
        if count != 0 {
            self.set_flag(Flags::Extend, last_bit_out);
            self.set_flag(Flags::Carry, last_bit_out);
        } else {
            self.set_flag(Flags::Carry, false);
        }
    }

    fn execute_bcc(&mut self, cond: Condition, offset: i32) -> Result<(), M68kError<Bus::Error>> {
        let should_branch = self.get_current_condition(cond);
        if should_branch {
            if let Err(err) = self.set_pc(self.cycle.decoder.start.wrapping_add(2).wrapping_add(offset as u32)) {
                self.state.pc -= 2;
                return Err(err);
            }
        }
        Ok(())
    }

    fn execute_bra(&mut self, offset: i32) -> Result<(), M68kError<Bus::Error>> {
        if let Err(err) = self.set_pc(self.cycle.decoder.start.wrapping_add(2).wrapping_add(offset as u32)) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    fn execute_bsr(&mut self, offset: i32) -> Result<(), M68kError<Bus::Error>> {
        self.push_long(self.state.pc)?;
        let sp = *self.get_stack_pointer_mut();
        self.debugger.stack_tracer.push_return(sp);
        if let Err(err) = self.set_pc(self.cycle.decoder.start.wrapping_add(2).wrapping_add(offset as u32)) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    fn execute_bchg(&mut self, bitnum: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
        let mut src_val = self.get_target_value(target, size, Used::Twice)?;
        let mask = self.set_bit_test_flags(src_val, bitnum, size);
        src_val = (src_val & !mask) | (!(src_val & mask) & mask);
        self.set_target_value(target, src_val, size, Used::Twice)?;
        Ok(())
    }

    fn execute_bclr(&mut self, bitnum: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
        let mut src_val = self.get_target_value(target, size, Used::Twice)?;
        let mask = self.set_bit_test_flags(src_val, bitnum, size);
        src_val &= !mask;
        self.set_target_value(target, src_val, size, Used::Twice)?;
        Ok(())
    }

    fn execute_bset(&mut self, bitnum: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
        let mut value = self.get_target_value(target, size, Used::Twice)?;
        let mask = self.set_bit_test_flags(value, bitnum, size);
        value |= mask;
        self.set_target_value(target, value, size, Used::Twice)?;
        Ok(())
    }

    fn execute_btst(&mut self, bitnum: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let bitnum = self.get_target_value(bitnum, Size::Byte, Used::Once)?;
        let value = self.get_target_value(target, size, Used::Once)?;
        self.set_bit_test_flags(value, bitnum, size);
        Ok(())
    }

    fn execute_bfchg(
        &mut self,
        target: Target,
        offset: RegOrImmediate,
        width: RegOrImmediate,
    ) -> Result<(), M68kError<Bus::Error>> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Twice)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        self.set_target_value(target, (value & !mask) | (!field & mask), Size::Long, Used::Twice)?;
        Ok(())
    }

    fn execute_bfclr(
        &mut self,
        target: Target,
        offset: RegOrImmediate,
        width: RegOrImmediate,
    ) -> Result<(), M68kError<Bus::Error>> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Twice)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        self.set_target_value(target, value & !mask, Size::Long, Used::Twice)?;
        Ok(())
    }

    fn execute_bfexts(
        &mut self,
        target: Target,
        offset: RegOrImmediate,
        width: RegOrImmediate,
        reg: Register,
    ) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_bfextu(
        &mut self,
        target: Target,
        offset: RegOrImmediate,
        width: RegOrImmediate,
        reg: Register,
    ) -> Result<(), M68kError<Bus::Error>> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Once)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        self.state.d_reg[reg as usize] = field >> (32 - offset - width);
        Ok(())
    }

    fn execute_bfset(
        &mut self,
        target: Target,
        offset: RegOrImmediate,
        width: RegOrImmediate,
    ) -> Result<(), M68kError<Bus::Error>> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Twice)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        self.set_target_value(target, value | mask, Size::Long, Used::Twice)?;
        Ok(())
    }

    fn execute_bftst(
        &mut self,
        target: Target,
        offset: RegOrImmediate,
        width: RegOrImmediate,
    ) -> Result<(), M68kError<Bus::Error>> {
        let (offset, width) = self.get_bit_field_args(offset, width);
        let mask = get_bit_field_mask(offset, width);
        let value = self.get_target_value(target, Size::Long, Used::Once)?;
        let field = value & mask;
        self.set_bit_field_test_flags(field, get_bit_field_msb(offset));
        Ok(())
    }

    fn execute_chk(&mut self, target: Target, reg: Register, size: Size) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_clr(&mut self, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        if self.cycle.decoder.cputype == M68kType::MC68000 {
            self.get_target_value(target, size, Used::Twice)?;
            self.set_target_value(target, 0, size, Used::Twice)?;
        } else {
            self.set_target_value(target, 0, size, Used::Once)?;
        }
        // Clear flags except Zero flag
        self.state.sr = (self.state.sr & 0xFFF0) | (Flags::Zero as u16);
        Ok(())
    }

    fn execute_cmp(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Once)?;
        let (result, carry) = overflowing_sub_sized(dest_val, src_val, size);
        let overflow = get_sub_overflow(dest_val, src_val, result, size);
        self.set_compare_flags(result, size, carry, overflow);
        Ok(())
    }

    fn execute_cmpa(&mut self, src: Target, reg: Register, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
        let dest_val = *self.get_a_reg_mut(reg);
        let (result, carry) = overflowing_sub_sized(dest_val, src_val, Size::Long);
        let overflow = get_sub_overflow(dest_val, src_val, result, Size::Long);
        self.set_compare_flags(result, Size::Long, carry, overflow);
        Ok(())
    }

    fn execute_dbcc(&mut self, cond: Condition, reg: Register, offset: i16) -> Result<(), M68kError<Bus::Error>> {
        let condition_true = self.get_current_condition(cond);
        if !condition_true {
            let next = ((get_value_sized(self.state.d_reg[reg as usize], Size::Word) as u16) as i16).wrapping_sub(1);
            set_value_sized(&mut self.state.d_reg[reg as usize], next as u32, Size::Word);
            if next != -1 {
                if let Err(err) = self.set_pc(self.cycle.decoder.start.wrapping_add(2).wrapping_add(offset as u32)) {
                    self.state.pc -= 2;
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    fn execute_divw(&mut self, src: Target, dest: Register, sign: Sign) -> Result<(), M68kError<Bus::Error>> {
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
                    quotient > i16::MAX as i32 || quotient < i16::MIN as i32,
                )
            },
            Sign::Unsigned => {
                let quotient = dest_val / src_val;
                (dest_val % src_val, quotient, (quotient & 0xFFFF0000) != 0)
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

    fn execute_divl(
        &mut self,
        src: Target,
        dest_h: Option<Register>,
        dest_l: Register,
        sign: Sign,
    ) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_eor(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let result = get_value_sized(dest_val ^ src_val, size);
        self.set_target_value(dest, result, size, Used::Twice)?;
        self.set_logic_flags(result, size);
        Ok(())
    }

    fn execute_eor_to_ccr(&mut self, value: u8) -> Result<(), M68kError<Bus::Error>> {
        self.set_sr((self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) ^ (value as u16)));
        Ok(())
    }

    fn execute_eor_to_sr(&mut self, value: u16) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        self.set_sr(self.state.sr ^ value);
        Ok(())
    }

    fn execute_exg(&mut self, target1: Target, target2: Target) -> Result<(), M68kError<Bus::Error>> {
        let value1 = self.get_target_value(target1, Size::Long, Used::Twice)?;
        let value2 = self.get_target_value(target2, Size::Long, Used::Twice)?;
        self.set_target_value(target1, value2, Size::Long, Used::Twice)?;
        self.set_target_value(target2, value1, Size::Long, Used::Twice)?;
        Ok(())
    }

    fn execute_ext(&mut self, reg: Register, from_size: Size, to_size: Size) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_illegal(&mut self) -> Result<(), M68kError<Bus::Error>> {
        self.exception(Exceptions::IllegalInstruction as u8, false)?;
        Ok(())
    }

    fn execute_jmp(&mut self, target: Target) -> Result<(), M68kError<Bus::Error>> {
        let addr = self.get_target_address(target)?;
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    fn execute_jsr(&mut self, target: Target) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_lea(&mut self, target: Target, reg: Register) -> Result<(), M68kError<Bus::Error>> {
        let value = self.get_target_address(target)?;
        let addr = self.get_a_reg_mut(reg);
        *addr = value;
        Ok(())
    }

    fn execute_link(&mut self, reg: Register, offset: i32) -> Result<(), M68kError<Bus::Error>> {
        *self.get_stack_pointer_mut() -= 4;
        let sp = *self.get_stack_pointer_mut();
        let value = *self.get_a_reg_mut(reg);
        self.set_address_sized(sp, value, Size::Long)?;
        *self.get_a_reg_mut(reg) = sp;
        *self.get_stack_pointer_mut() = (sp as i32).wrapping_add(offset) as u32;
        Ok(())
    }

    fn execute_lsl(&mut self, count: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
        for _ in 0..count {
            pair = shift_left(pair.0, size);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;

        self.set_shift_flags(pair.0, count, pair.1, size);
        Ok(())
    }

    fn execute_lsr(&mut self, count: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
        for _ in 0..count {
            pair = shift_right(pair.0, size, false);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;

        self.set_shift_flags(pair.0, count, pair.1, size);
        Ok(())
    }

    fn set_shift_flags(&mut self, result: u32, count: u32, last_bit_out: bool, size: Size) {
        self.set_logic_flags(result, size);
        self.set_flag(Flags::Overflow, false);
        if count != 0 {
            self.set_flag(Flags::Extend, last_bit_out);
            self.set_flag(Flags::Carry, last_bit_out);
        } else {
            self.set_flag(Flags::Carry, false);
        }
    }

    fn execute_move(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        self.set_logic_flags(src_val, size);
        self.set_target_value(dest, src_val, size, Used::Once)?;
        Ok(())
    }

    fn execute_movea(&mut self, src: Target, reg: Register, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let src_val = sign_extend_to_long(src_val, size) as u32;
        let addr = self.get_a_reg_mut(reg);
        *addr = src_val;
        Ok(())
    }

    fn execute_move_from_sr(&mut self, target: Target) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        self.set_target_value(target, self.state.sr as u32, Size::Word, Used::Once)?;
        Ok(())
    }

    fn execute_move_to_sr(&mut self, target: Target) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        let value = self.get_target_value(target, Size::Word, Used::Once)? as u16;
        self.set_sr(value);
        Ok(())
    }

    fn execute_move_to_ccr(&mut self, target: Target) -> Result<(), M68kError<Bus::Error>> {
        let value = self.get_target_value(target, Size::Word, Used::Once)? as u16;
        self.set_sr((self.state.sr & 0xFF00) | (value & 0x00FF));
        Ok(())
    }

    fn execute_movec(&mut self, target: Target, control_reg: ControlRegister, dir: Direction) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_movem(&mut self, target: Target, size: Size, dir: Direction, mask: u16) -> Result<(), M68kError<Bus::Error>> {
        let addr = self.get_target_address(target)?;

        // If we're using a MC68020 or higher, and it was Post-Inc/Pre-Dec target, then update the value before it's stored
        if self.cycle.decoder.cputype >= M68kType::MC68020 {
            match target {
                Target::IndirectARegInc(reg) | Target::IndirectARegDec(reg) => {
                    let a_reg_mut = self.get_a_reg_mut(reg);
                    *a_reg_mut = addr + (mask.count_ones() * size.in_bytes());
                },
                _ => {},
            }
        }

        let post_addr = match target {
            Target::IndirectARegInc(_) => {
                if dir != Direction::FromTarget {
                    return Err(M68kError::Other(format!("Cannot use {:?} with {:?}", target, dir)));
                }
                self.move_memory_to_registers(addr, size, mask)?
            },
            Target::IndirectARegDec(_) => {
                if dir != Direction::ToTarget {
                    return Err(M68kError::Other(format!("Cannot use {:?} with {:?}", target, dir)));
                }
                self.move_registers_to_memory_reverse(addr, size, mask)?
            },
            _ => match dir {
                Direction::ToTarget => self.move_registers_to_memory(addr, size, mask)?,
                Direction::FromTarget => self.move_memory_to_registers(addr, size, mask)?,
            },
        };

        // If it was Post-Inc/Pre-Dec target, then update the value
        match target {
            Target::IndirectARegInc(reg) | Target::IndirectARegDec(reg) => {
                let a_reg_mut = self.get_a_reg_mut(reg);
                *a_reg_mut = post_addr;
            },
            _ => {},
        }

        Ok(())
    }

    fn move_memory_to_registers(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, M68kError<Bus::Error>> {
        for i in 0..8 {
            if (mask & 0x01) != 0 {
                self.state.d_reg[i] = sign_extend_to_long(self.get_address_sized(addr, size)?, size) as u32;
                (addr, _) = overflowing_add_sized(addr, size.in_bytes(), Size::Long);
            }
            mask >>= 1;
        }
        for i in 0..8 {
            if (mask & 0x01) != 0 {
                *self.get_a_reg_mut(i) = sign_extend_to_long(self.get_address_sized(addr, size)?, size) as u32;
                (addr, _) = overflowing_add_sized(addr, size.in_bytes(), Size::Long);
            }
            mask >>= 1;
        }
        Ok(addr)
    }

    fn move_registers_to_memory(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, M68kError<Bus::Error>> {
        for i in 0..8 {
            if (mask & 0x01) != 0 {
                self.set_address_sized(addr, self.state.d_reg[i], size)?;
                addr += size.in_bytes();
            }
            mask >>= 1;
        }
        for i in 0..8 {
            if (mask & 0x01) != 0 {
                let value = *self.get_a_reg_mut(i);
                self.set_address_sized(addr, value, size)?;
                addr += size.in_bytes();
            }
            mask >>= 1;
        }
        Ok(addr)
    }

    fn move_registers_to_memory_reverse(&mut self, mut addr: u32, size: Size, mut mask: u16) -> Result<u32, M68kError<Bus::Error>> {
        for i in (0..8).rev() {
            if (mask & 0x01) != 0 {
                let value = *self.get_a_reg_mut(i);
                addr -= size.in_bytes();
                self.set_address_sized(addr, value, size)?;
            }
            mask >>= 1;
        }
        for i in (0..8).rev() {
            if (mask & 0x01) != 0 {
                addr -= size.in_bytes();
                self.set_address_sized(addr, self.state.d_reg[i], size)?;
            }
            mask >>= 1;
        }
        Ok(addr)
    }

    fn execute_movep(
        &mut self,
        dreg: Register,
        areg: Register,
        offset: i16,
        size: Size,
        dir: Direction,
    ) -> Result<(), M68kError<Bus::Error>> {
        match dir {
            Direction::ToTarget => {
                let mut shift = (size.in_bits() as i32) - 8;
                let mut addr = (*self.get_a_reg_mut(areg)).wrapping_add_signed(offset as i32);
                while shift >= 0 {
                    let byte = self.state.d_reg[dreg as usize] >> shift;
                    self.set_address_sized(addr, byte, Size::Byte)?;
                    addr += 2;
                    shift -= 8;
                }
            },
            Direction::FromTarget => {
                let mut shift = (size.in_bits() as i32) - 8;
                let mut addr = (*self.get_a_reg_mut(areg)).wrapping_add_signed(offset as i32);
                while shift >= 0 {
                    let byte = self.get_address_sized(addr, Size::Byte)?;
                    self.state.d_reg[dreg as usize] |= byte << shift;
                    addr += 2;
                    shift -= 8;
                }
            },
        }
        Ok(())
    }

    fn execute_moveq(&mut self, data: u8, reg: Register) -> Result<(), M68kError<Bus::Error>> {
        let value = sign_extend_to_long(data as u32, Size::Byte) as u32;
        self.state.d_reg[reg as usize] = value;
        self.set_logic_flags(value, Size::Long);
        Ok(())
    }

    fn execute_moveusp(&mut self, target: Target, dir: Direction) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        match dir {
            Direction::ToTarget => self.set_target_value(target, self.state.usp, Size::Long, Used::Once)?,
            Direction::FromTarget => {
                self.state.usp = self.get_target_value(target, Size::Long, Used::Once)?;
            },
        }
        Ok(())
    }

    fn execute_mulw(&mut self, src: Target, dest: Register, sign: Sign) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_mull(
        &mut self,
        src: Target,
        dest_h: Option<Register>,
        dest_l: Register,
        sign: Sign,
    ) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_nbcd(&mut self, dest: Target) -> Result<(), M68kError<Bus::Error>> {
        let dest_val = self.get_target_value(dest, Size::Byte, Used::Twice)?;
        let result = self.execute_sbcd_val(dest_val, 0)?;
        self.set_target_value(dest, result, Size::Byte, Used::Twice)?;
        Ok(())
    }

    fn execute_neg(&mut self, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let original = self.get_target_value(target, size, Used::Twice)?;
        let (result, overflow) = overflowing_sub_signed_sized(0, original, size);
        let carry = result != 0;
        self.set_target_value(target, result, size, Used::Twice)?;
        self.set_compare_flags(result, size, carry, overflow);
        self.set_flag(Flags::Extend, carry);
        Ok(())
    }

    fn execute_negx(&mut self, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_not(&mut self, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let mut value = self.get_target_value(target, size, Used::Twice)?;
        value = get_value_sized(!value, size);
        self.set_target_value(target, value, size, Used::Twice)?;
        self.set_logic_flags(value, size);
        Ok(())
    }

    fn execute_or(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let result = get_value_sized(dest_val | src_val, size);
        self.set_target_value(dest, result, size, Used::Twice)?;
        self.set_logic_flags(result, size);
        Ok(())
    }

    fn execute_or_to_ccr(&mut self, value: u8) -> Result<(), M68kError<Bus::Error>> {
        self.set_sr((self.state.sr & 0xFF00) | ((self.state.sr & 0x00FF) | (value as u16)));
        Ok(())
    }

    fn execute_or_to_sr(&mut self, value: u16) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        self.set_sr(self.state.sr | value);
        Ok(())
    }

    fn execute_pea(&mut self, target: Target) -> Result<(), M68kError<Bus::Error>> {
        let value = self.get_target_address(target)?;
        self.push_long(value)?;
        Ok(())
    }

    fn execute_reset(&mut self) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        // TODO this only resets external devices and not internal ones
        Ok(())
    }

    fn execute_rol(&mut self, count: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
        for _ in 0..count {
            pair = rotate_left(pair.0, size, None);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;
        self.set_rotate_flags(pair.0, pair.1, size);
        Ok(())
    }

    fn execute_ror(&mut self, count: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
        for _ in 0..count {
            pair = rotate_right(pair.0, size, None);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;
        self.set_rotate_flags(pair.0, pair.1, size);
        Ok(())
    }

    fn execute_roxl(&mut self, count: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
        for _ in 0..count {
            pair = rotate_left(pair.0, size, Some(self.get_flag(Flags::Extend)));
            self.set_flag(Flags::Extend, pair.1);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;
        self.set_rotate_flags(pair.0, pair.1, size);
        Ok(())
    }

    fn execute_roxr(&mut self, count: Target, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let count = self.get_target_value(count, size, Used::Once)? % 64;
        let mut pair = (self.get_target_value(target, size, Used::Twice)?, false);
        for _ in 0..count {
            pair = rotate_right(pair.0, size, Some(self.get_flag(Flags::Extend)));
            self.set_flag(Flags::Extend, pair.1);
        }
        self.set_target_value(target, pair.0, size, Used::Twice)?;
        self.set_rotate_flags(pair.0, pair.1, size);
        Ok(())
    }

    fn set_rotate_flags(&mut self, result: u32, last_bit_out: bool, size: Size) {
        self.set_logic_flags(result, size);
        if last_bit_out {
            self.set_flag(Flags::Carry, true);
        }
    }

    fn execute_rte(&mut self) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        let sr = self.pop_word()?;
        let addr = self.pop_long()?;

        if self.cycle.decoder.cputype >= M68kType::MC68010 {
            let _ = self.pop_word()?;
        }

        self.set_sr(sr);
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    fn execute_rtr(&mut self) -> Result<(), M68kError<Bus::Error>> {
        let ccr = self.pop_word()?;
        let addr = self.pop_long()?;
        self.set_sr((self.state.sr & 0xFF00) | (ccr & 0x00FF));
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    fn execute_rts(&mut self) -> Result<(), M68kError<Bus::Error>> {
        self.debugger.stack_tracer.pop_return();
        let addr = self.pop_long()?;
        if let Err(err) = self.set_pc(addr) {
            self.state.pc -= 2;
            return Err(err);
        }
        Ok(())
    }

    fn execute_scc(&mut self, cond: Condition, target: Target) -> Result<(), M68kError<Bus::Error>> {
        let condition_true = self.get_current_condition(cond);
        if condition_true {
            self.set_target_value(target, 0xFF, Size::Byte, Used::Once)?;
        } else {
            self.set_target_value(target, 0x00, Size::Byte, Used::Once)?;
        }
        Ok(())
    }

    fn execute_stop(&mut self, flags: u16) -> Result<(), M68kError<Bus::Error>> {
        self.require_supervisor()?;
        self.set_sr(flags);
        self.state.status = Status::Stopped;
        Ok(())
    }

    fn execute_sbcd(&mut self, src: Target, dest: Target) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, Size::Byte, Used::Once)?;
        let dest_val = self.get_target_value(dest, Size::Byte, Used::Twice)?;
        let result = self.execute_sbcd_val(src_val, dest_val)?;
        self.set_target_value(dest, result, Size::Byte, Used::Twice)?;
        Ok(())
    }

    fn execute_sbcd_val(&mut self, src_val: u32, dest_val: u32) -> Result<u32, M68kError<Bus::Error>> {
        let extend_flag = self.get_flag(Flags::Extend) as u32;
        let src_parts = get_nibbles_from_byte(src_val);
        let dest_parts = get_nibbles_from_byte(dest_val);

        let binary_result = dest_val.wrapping_sub(src_val).wrapping_sub(extend_flag);
        let mut result = dest_parts.1.wrapping_sub(src_parts.1).wrapping_sub(extend_flag);
        if (result & 0x1F) > 0x09 {
            result -= 0x06
        };
        result = result.wrapping_add(dest_parts.0.wrapping_sub(src_parts.0));
        let carry = (result & 0x1FF) > 0x99;
        if carry {
            result -= 0x60
        };

        self.set_flag(Flags::Negative, get_msb(result, Size::Byte));
        self.set_flag(Flags::Zero, (result & 0xFF) == 0);
        self.set_flag(Flags::Overflow, (binary_result & !result & 0x80) != 0);
        self.set_flag(Flags::Carry, carry);
        self.set_flag(Flags::Extend, carry);

        Ok(result)
    }

    fn execute_sub(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = self.get_target_value(src, size, Used::Once)?;
        let dest_val = self.get_target_value(dest, size, Used::Twice)?;
        let (result, carry) = overflowing_sub_sized(dest_val, src_val, size);
        let overflow = get_sub_overflow(dest_val, src_val, result, size);
        self.set_compare_flags(result, size, carry, overflow);
        self.set_flag(Flags::Extend, carry);
        self.set_target_value(dest, result, size, Used::Twice)?;
        Ok(())
    }

    fn execute_suba(&mut self, src: Target, dest: Register, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let src_val = sign_extend_to_long(self.get_target_value(src, size, Used::Once)?, size) as u32;
        let dest_val = *self.get_a_reg_mut(dest);
        let (result, _) = overflowing_sub_sized(dest_val, src_val, Size::Long);
        *self.get_a_reg_mut(dest) = result;
        Ok(())
    }

    fn execute_subx(&mut self, src: Target, dest: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
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

    fn execute_swap(&mut self, reg: Register) -> Result<(), M68kError<Bus::Error>> {
        let value = self.state.d_reg[reg as usize];
        self.state.d_reg[reg as usize] = ((value & 0x0000FFFF) << 16) | ((value & 0xFFFF0000) >> 16);
        self.set_logic_flags(self.state.d_reg[reg as usize], Size::Long);
        Ok(())
    }

    fn execute_tas(&mut self, target: Target) -> Result<(), M68kError<Bus::Error>> {
        let value = self.get_target_value(target, Size::Byte, Used::Twice)?;
        self.set_flag(Flags::Negative, (value & 0x80) != 0);
        self.set_flag(Flags::Zero, value == 0);
        self.set_flag(Flags::Overflow, false);
        self.set_flag(Flags::Carry, false);
        self.set_target_value(target, value | 0x80, Size::Byte, Used::Twice)?;
        Ok(())
    }

    fn execute_tst(&mut self, target: Target, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let value = self.get_target_value(target, size, Used::Once)?;
        self.set_logic_flags(value, size);
        Ok(())
    }

    fn execute_trap(&mut self, number: u8) -> Result<(), M68kError<Bus::Error>> {
        self.exception(32 + number, false)?;
        Ok(())
    }

    fn execute_trapv(&mut self) -> Result<(), M68kError<Bus::Error>> {
        if self.get_flag(Flags::Overflow) {
            self.exception(Exceptions::TrapvInstruction as u8, false)?;
        }
        Ok(())
    }

    fn execute_unlk(&mut self, reg: Register) -> Result<(), M68kError<Bus::Error>> {
        let value = *self.get_a_reg_mut(reg);
        *self.get_stack_pointer_mut() = value;
        let new_value = self.pop_long()?;
        let addr = self.get_a_reg_mut(reg);
        *addr = new_value;
        Ok(())
    }

    fn execute_unimplemented_a(&mut self, _: u16) -> Result<(), M68kError<Bus::Error>> {
        self.state.pc -= 2;
        self.exception(Exceptions::LineAEmulator as u8, false)?;
        Ok(())
    }

    fn execute_unimplemented_f(&mut self, _: u16) -> Result<(), M68kError<Bus::Error>> {
        self.state.pc -= 2;
        self.exception(Exceptions::LineFEmulator as u8, false)?;
        Ok(())
    }


    pub(super) fn get_target_value(&mut self, target: Target, size: Size, used: Used) -> Result<u32, M68kError<Bus::Error>> {
        match target {
            Target::Immediate(value) => Ok(value),
            Target::DirectDReg(reg) => Ok(get_value_sized(self.state.d_reg[reg as usize], size)),
            Target::DirectAReg(reg) => Ok(get_value_sized(*self.get_a_reg_mut(reg), size)),
            Target::IndirectAReg(reg) => {
                let addr = *self.get_a_reg_mut(reg);
                self.get_address_sized(addr, size)
            },
            Target::IndirectARegInc(reg) => {
                let addr = self.post_increment_areg_target(reg, size, used);
                self.get_address_sized(addr, size)
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.pre_decrement_areg_target(reg, size, Used::Once);
                self.get_address_sized(addr, size)
            },
            Target::IndirectRegOffset(base_reg, index_reg, displacement) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                self.get_address_sized(base_value.wrapping_add(displacement as u32).wrapping_add(index_value as u32), size)
            },
            Target::IndirectMemoryPreindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate =
                    self.get_address_sized(base_value.wrapping_add(base_disp as u32).wrapping_add(index_value as u32), Size::Long)?;
                self.get_address_sized(intermediate.wrapping_add(outer_disp as u32), size)
            },
            Target::IndirectMemoryPostindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32), Size::Long)?;
                self.get_address_sized(intermediate.wrapping_add(index_value as u32).wrapping_add(outer_disp as u32), size)
            },
            Target::IndirectMemory(addr, _) => self.get_address_sized(addr, size),
        }
    }

    pub(super) fn set_target_value(
        &mut self,
        target: Target,
        value: u32,
        size: Size,
        used: Used,
    ) -> Result<(), M68kError<Bus::Error>> {
        match target {
            Target::DirectDReg(reg) => {
                set_value_sized(&mut self.state.d_reg[reg as usize], value, size);
            },
            Target::DirectAReg(reg) => {
                set_value_sized(self.get_a_reg_mut(reg), value, size);
            },
            Target::IndirectAReg(reg) => {
                let addr = *self.get_a_reg_mut(reg);
                self.set_address_sized(addr, value, size)?;
            },
            Target::IndirectARegInc(reg) => {
                let addr = self.post_increment_areg_target(reg, size, Used::Once);
                self.set_address_sized(addr, value, size)?;
            },
            Target::IndirectARegDec(reg) => {
                let addr = self.pre_decrement_areg_target(reg, size, used);
                self.set_address_sized(addr, value, size)?;
            },
            Target::IndirectRegOffset(base_reg, index_reg, displacement) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                self.set_address_sized(base_value.wrapping_add(displacement as u32).wrapping_add(index_value as u32), value, size)?;
            },
            Target::IndirectMemoryPreindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate =
                    self.get_address_sized(base_value.wrapping_add(base_disp as u32).wrapping_add(index_value as u32), Size::Long)?;
                self.set_address_sized(intermediate.wrapping_add(outer_disp as u32), value, size)?;
            },
            Target::IndirectMemoryPostindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32), Size::Long)?;
                self.set_address_sized(intermediate.wrapping_add(index_value as u32).wrapping_add(outer_disp as u32), value, size)?;
            },
            Target::IndirectMemory(addr, _) => {
                self.set_address_sized(addr, value, size)?;
            },
            Target::Immediate(_) => return Err(M68kError::InvalidTarget(target)),
        }
        Ok(())
    }

    fn get_target_address(&mut self, target: Target) -> Result<u32, M68kError<Bus::Error>> {
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
                let intermediate =
                    self.get_address_sized(base_value.wrapping_add(base_disp as u32).wrapping_add(index_value as u32), Size::Long)?;
                intermediate.wrapping_add(outer_disp as u32)
            },
            Target::IndirectMemoryPostindexed(base_reg, index_reg, base_disp, outer_disp) => {
                let base_value = self.get_base_reg_value(base_reg);
                let index_value = self.get_index_reg_value(&index_reg);
                let intermediate = self.get_address_sized(base_value.wrapping_add(base_disp as u32), Size::Long)?;
                intermediate.wrapping_add(index_value as u32).wrapping_add(outer_disp as u32)
            },
            Target::IndirectMemory(addr, _) => addr,
            _ => return Err(M68kError::InvalidTarget(target)),
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

    fn get_address_sized(&mut self, addr: M68kAddress, size: Size) -> Result<u32, M68kError<Bus::Error>> {
        let is_supervisor = self.is_supervisor();
        self.cycle.memory.read_data_sized(&mut self.bus, is_supervisor, addr, size)
    }

    fn set_address_sized(&mut self, addr: M68kAddress, value: u32, size: Size) -> Result<(), M68kError<Bus::Error>> {
        let is_supervisor = self.is_supervisor();
        self.cycle
            .memory
            .write_data_sized(&mut self.bus, is_supervisor, addr, size, value)
    }

    fn push_word(&mut self, value: u16) -> Result<(), M68kError<Bus::Error>> {
        let is_supervisor = self.is_supervisor();
        *self.get_stack_pointer_mut() -= 2;
        let addr = *self.get_stack_pointer_mut();
        self.cycle
            .memory
            .write_data_sized(&mut self.bus, is_supervisor, addr, Size::Word, value as u32)?;
        Ok(())
    }

    fn pop_word(&mut self) -> Result<u16, M68kError<Bus::Error>> {
        let is_supervisor = self.is_supervisor();
        let addr = *self.get_stack_pointer_mut();
        let value = self
            .cycle
            .memory
            .read_data_sized(&mut self.bus, is_supervisor, addr, Size::Word)?;
        *self.get_stack_pointer_mut() += 2;
        Ok(value as u16)
    }

    fn push_long(&mut self, value: u32) -> Result<(), M68kError<Bus::Error>> {
        let is_supervisor = self.is_supervisor();
        *self.get_stack_pointer_mut() -= 4;
        let addr = *self.get_stack_pointer_mut();
        self.cycle
            .memory
            .write_data_sized(&mut self.bus, is_supervisor, addr, Size::Long, value)?;
        Ok(())
    }

    fn pop_long(&mut self) -> Result<u32, M68kError<Bus::Error>> {
        let is_supervisor = self.is_supervisor();
        let addr = *self.get_stack_pointer_mut();
        let value = self
            .cycle
            .memory
            .read_data_sized(&mut self.bus, is_supervisor, addr, Size::Long)?;
        *self.get_stack_pointer_mut() += 4;
        Ok(value)
    }

    fn set_pc(&mut self, value: u32) -> Result<(), M68kError<Bus::Error>> {
        self.state.pc = value;
        self.cycle.memory.start_request(
            self.is_supervisor(),
            self.state.pc,
            Size::Word,
            MemAccess::Read,
            MemType::Program,
            true,
        )?;
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
            BaseRegister::PC => self.cycle.decoder.start + 2,
            BaseRegister::AReg(7) => {
                if self.is_supervisor() {
                    self.state.ssp
                } else {
                    self.state.usp
                }
            },
            BaseRegister::AReg(reg) => self.state.a_reg[reg as usize],
        }
    }

    fn get_index_reg_value(&self, index_reg: &Option<IndexRegister>) -> i32 {
        match index_reg {
            None => 0,
            Some(IndexRegister {
                xreg,
                scale,
                size,
            }) => sign_extend_to_long(self.get_x_reg_value(*xreg), *size) << scale,
        }
    }

    fn get_control_reg_mut(&mut self, control_reg: ControlRegister) -> &mut u32 {
        match control_reg {
            ControlRegister::VBR => &mut self.state.vbr,
        }
    }

    fn get_stack_pointer_mut(&mut self) -> &mut u32 {
        if self.is_supervisor() {
            &mut self.state.ssp
        } else {
            &mut self.state.usp
        }
    }

    fn get_a_reg(&self, reg: Register) -> u32 {
        if reg == 7 {
            if self.is_supervisor() {
                self.state.ssp
            } else {
                self.state.usp
            }
        } else {
            self.state.a_reg[reg as usize]
        }
    }

    fn get_a_reg_mut(&mut self, reg: Register) -> &mut u32 {
        if reg == 7 {
            if self.is_supervisor() {
                &mut self.state.ssp
            } else {
                &mut self.state.usp
            }
        } else {
            &mut self.state.a_reg[reg as usize]
        }
    }

    fn is_supervisor(&self) -> bool {
        self.state.sr & (Flags::Supervisor as u16) != 0
    }

    fn require_supervisor(&self) -> Result<(), M68kError<Bus::Error>> {
        if self.is_supervisor() {
            Ok(())
        } else {
            Err(M68kError::Exception(Exceptions::PrivilegeViolation))
        }
    }

    fn set_sr(&mut self, value: u16) {
        let mask = if self.cycle.decoder.cputype <= M68kType::MC68010 {
            0xA71F
        } else {
            0xF71F
        };
        self.state.sr = value & mask;
    }

    fn get_flag(&self, flag: Flags) -> bool {
        (self.state.sr & (flag as u16)) != 0
    }

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
            Condition::GreaterThanOrEqual => {
                (self.get_flag(Flags::Negative) && self.get_flag(Flags::Overflow))
                    || (!self.get_flag(Flags::Negative) && !self.get_flag(Flags::Overflow))
            },
            Condition::LessThan => {
                (self.get_flag(Flags::Negative) && !self.get_flag(Flags::Overflow))
                    || (!self.get_flag(Flags::Negative) && self.get_flag(Flags::Overflow))
            },
            Condition::GreaterThan => {
                (self.get_flag(Flags::Negative) && self.get_flag(Flags::Overflow) && !self.get_flag(Flags::Zero))
                    || (!self.get_flag(Flags::Negative) && !self.get_flag(Flags::Overflow) && !self.get_flag(Flags::Zero))
            },
            Condition::LessThanOrEqual => {
                self.get_flag(Flags::Zero)
                    || (self.get_flag(Flags::Negative) && !self.get_flag(Flags::Overflow))
                    || (!self.get_flag(Flags::Negative) && self.get_flag(Flags::Overflow))
            },
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

fn shift_left(value: u32, size: Size) -> (u32, bool) {
    let bit = get_msb(value, size);
    match size {
        Size::Byte => (((value as u8) << 1) as u32, bit),
        Size::Word => (((value as u16) << 1) as u32, bit),
        Size::Long => (value << 1, bit),
    }
}

fn shift_right(value: u32, size: Size, arithmetic: bool) -> (u32, bool) {
    let mask = if arithmetic { get_msb_mask(value, size) } else { 0 };
    ((value >> 1) | mask, (value & 0x1) != 0)
}

fn rotate_left(value: u32, size: Size, use_extend: Option<bool>) -> (u32, bool) {
    let bit = get_msb(value, size);
    let mask = if use_extend.unwrap_or(bit) { 0x01 } else { 0x00 };
    match size {
        Size::Byte => (mask | ((value as u8) << 1) as u32, bit),
        Size::Word => (mask | ((value as u16) << 1) as u32, bit),
        Size::Long => (mask | value << 1, bit),
    }
}

fn rotate_right(value: u32, size: Size, use_extend: Option<bool>) -> (u32, bool) {
    let bit = (value & 0x01) != 0;
    let mask = if use_extend.unwrap_or(bit) {
        get_msb_mask(0xffffffff, size)
    } else {
        0x0
    };
    ((value >> 1) | mask, bit)
}

fn get_nibbles_from_byte(value: u32) -> (u32, u32) {
    (value & 0xF0, value & 0x0F)
}

fn get_value_sized(value: u32, size: Size) -> u32 {
    match size {
        Size::Byte => 0x000000FF & value,
        Size::Word => 0x0000FFFF & value,
        Size::Long => value,
    }
}

fn set_value_sized(addr: &mut u32, value: u32, size: Size) {
    match size {
        Size::Byte => {
            *addr = (*addr & 0xFFFFFF00) | (0x000000FF & value);
        },
        Size::Word => {
            *addr = (*addr & 0xFFFF0000) | (0x0000FFFF & value);
        },
        Size::Long => {
            *addr = value;
        },
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
