#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Direction {
    ToAcc,
    FromAcc,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Size {
    Byte,
    Word,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Condition {
    NotZero,
    Zero,
    NotCarry,
    Carry,
    ParityOdd,
    ParityEven,
    Positive,
    Negative,
}

#[repr(u8)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Register {
    B = 0,
    C = 1,
    D = 2,
    E = 3,
    H = 4,
    L = 5,
    A = 6,
    F = 7,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RegisterPair {
    BC,
    DE,
    HL,
    AF,
    SP,
    IX,
    IY,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IndexRegister {
    IX,
    IY,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum IndexRegisterHalf {
    IXH,
    IXL,
    IYH,
    IYL,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SpecialRegister {
    I,
    R,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum InterruptMode {
    Mode0,
    Mode1,
    Mode2,
    Unknown(u8),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Target {
    DirectReg(Register),
    DirectRegHalf(IndexRegisterHalf),
    IndirectReg(RegisterPair),
    IndirectOffset(IndexRegister, i8),
    Immediate(u8),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum LoadTarget {
    DirectRegByte(Register),
    DirectRegHalfByte(IndexRegisterHalf),
    DirectRegWord(RegisterPair),
    IndirectRegByte(RegisterPair),
    IndirectRegWord(RegisterPair),
    IndirectOffsetByte(IndexRegister, i8),
    DirectAltRegByte(Register),
    IndirectByte(u16),
    IndirectWord(u16),
    ImmediateByte(u8),
    ImmediateWord(u16),
}

pub type UndocumentedCopy = Option<Target>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Instruction {
    ADCa(Target),
    ADC16(RegisterPair, RegisterPair),
    ADDa(Target),
    ADD16(RegisterPair, RegisterPair),
    AND(Target),
    BIT(u8, Target),
    CALL(u16),
    CALLcc(Condition, u16),
    CCF,
    CP(Target),
    CPD,
    CPDR,
    CPI,
    CPIR,
    CPL,
    DAA,
    DEC16(RegisterPair),
    DEC8(Target),
    DI,
    DJNZ(i8),
    EI,
    EXX,
    EXafaf,
    EXhlde,
    EXsp(RegisterPair),
    HALT,
    IM(InterruptMode),
    INC16(RegisterPair),
    INC8(Target),
    IND,
    INDR,
    INI,
    INIR,
    INic(Register),
    INicz,
    INx(u8),
    JP(u16),
    JPIndirect(RegisterPair),
    JPcc(Condition, u16),
    JR(i8),
    JRcc(Condition, i8),
    LD(LoadTarget, LoadTarget),
    LDsr(SpecialRegister, Direction),
    LDD,
    LDDR,
    LDI,
    LDIR,
    NEG,
    NOP,
    OR(Target),
    OTDR,
    OTIR,
    OUTD,
    OUTI,
    OUTic(Register),
    OUTicz,
    OUTx(u8),
    POP(RegisterPair),
    PUSH(RegisterPair),
    RES(u8, Target, UndocumentedCopy),
    RET,
    RETI,
    RETN,
    RETcc(Condition),
    RL(Target, UndocumentedCopy),
    RLA,
    RLC(Target, UndocumentedCopy),
    RLCA,
    RLD,
    RR(Target, UndocumentedCopy),
    RRA,
    RRC(Target, UndocumentedCopy),
    RRCA,
    RRD,
    RST(u8),
    SBCa(Target),
    SBC16(RegisterPair, RegisterPair),
    SCF,
    SET(u8, Target, UndocumentedCopy),
    SLA(Target, UndocumentedCopy),
    SLL(Target, UndocumentedCopy),
    SRA(Target, UndocumentedCopy),
    SRL(Target, UndocumentedCopy),
    SUB(Target),
    XOR(Target),
}

impl From<u8> for InterruptMode {
    fn from(im: u8) -> Self {
        match im {
            0 => InterruptMode::Mode0,
            1 => InterruptMode::Mode1,
            2 => InterruptMode::Mode2,
            _ => InterruptMode::Unknown(im),
        }
    }
}

impl RegisterPair {
    pub(crate) fn is_index_reg(&self) -> bool {
        matches!(self, RegisterPair::IX | RegisterPair::IY)
    }
}
