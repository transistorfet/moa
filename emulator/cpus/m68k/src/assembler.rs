use std::collections::HashMap;

use moa_parsing::{self as parser, AssemblyLine, AssemblyOperand, AssemblyParser, ParserError};

use super::state::M68kType;
use super::instructions::Size;

#[derive(Clone, Debug)]
pub struct Error(String);

impl Error {
    pub fn new(msg: String) -> Self {
        Self(msg)
    }
}

impl From<ParserError> for Error {
    fn from(err: ParserError) -> Self {
        Self(err.0)
    }
}


#[repr(usize)]
#[derive(Copy, Clone)]
#[rustfmt::skip]
pub enum Disallow {
    None                        = 0x0000,
    NoDReg                      = 0x0001,
    NoAReg                      = 0x0002,
    NoIndirect                  = 0x0004,
    NoIndirectPre               = 0x0008,
    NoIndirectPost              = 0x0010,
    NoIndirectOffset            = 0x0020,
    NoIndirectIndexReg          = 0x0040,
    NoIndirectImmediate         = 0x0080,
    NoImmediate                 = 0x0100,
    NoPCRelative                = 0x0200,
    NoPCRelativeIndex           = 0x0400,

    NoRegs                      = 0x0003,
    NoRegsOrImmediate           = 0x0103,
    NoRegsImmediateOrPC         = 0x0703,
    NoARegImmediateOrPC         = 0x0702,
    NoRegsPrePostOrImmediate    = 0x011B,
    NoImmediateOrPC             = 0x0700,
    OnlyAReg                    = 0x07FD,
}

impl Disallow {
    pub fn check(&self, lineno: usize, disallow: Disallow) -> Result<(), Error> {
        if (*self as usize) & (disallow as usize) == 0 {
            Ok(())
        } else {
            Err(Error::new(format!("error at line {}: invalid addressing mode for the instruction", lineno)))
        }
    }
}

pub enum RelocationType {
    Displacement,
    Word(String),
    Long(String),
}

pub struct Relocation {
    pub rtype: RelocationType,
    pub label: String,
    pub index: usize,
    pub from_origin: usize,
}

impl Relocation {
    pub fn new(rtype: RelocationType, label: String, index: usize, from_origin: usize) -> Self {
        Self {
            rtype,
            label,
            index,
            from_origin,
        }
    }
}

pub struct M68kAssembler {
    pub cputype: M68kType,
    pub labels: HashMap<String, usize>,
    pub output: Vec<u16>,
    pub relocations: Vec<Relocation>,
    pub current_origin: usize,
}

impl M68kAssembler {
    pub fn new(cputype: M68kType) -> Self {
        Self {
            cputype,
            labels: HashMap::new(),
            output: vec![],
            relocations: vec![],
            current_origin: 0,
        }
    }

    pub fn assemble(&mut self, text: &str) -> Result<Vec<u8>, Error> {
        self.assemble_in_place(text)?;
        Ok(self.output.iter().fold(vec![], |mut acc, item| {
            acc.push((*item >> 8) as u8);
            acc.push(*item as u8);
            acc
        }))
    }

    pub fn assemble_words(&mut self, text: &str) -> Result<Vec<u16>, Error> {
        self.assemble_in_place(text)?;
        Ok(self.output.clone())
    }

    pub fn assemble_in_place(&mut self, text: &str) -> Result<(), Error> {
        let lines = self.parse(text)?;

        for (lineno, line) in lines {
            self.convert(lineno, &line)?;
        }

        self.apply_relocations()?;

        Ok(())
    }

    fn parse(&mut self, text: &str) -> Result<Vec<(usize, AssemblyLine)>, Error> {
        let mut parser = AssemblyParser::new(text);
        Ok(parser.parse()?)
    }

    fn apply_relocations(&mut self) -> Result<(), Error> {
        for reloc in self.relocations.iter() {
            match reloc.rtype {
                RelocationType::Displacement => {
                    // TODO this doesn't yet take into accound the origin
                    let location = *self
                        .labels
                        .get(&reloc.label)
                        .ok_or_else(|| Error::new(format!("error during relocation, label undefined {:?}", reloc.label)))?;
                    self.output[reloc.index] |= ((self.output[reloc.index] as i8 * 2 + 2) - (location as i8 * 2)) as u16 & 0x00ff;
                },
                _ => panic!("relocation type unimplemented"),
            }
        }
        Ok(())
    }

    fn convert(&mut self, lineno: usize, line: &AssemblyLine) -> Result<(), Error> {
        match line {
            AssemblyLine::Directive(name, list) => {
                println!("skipping directive {} ({:?})", name, list);
            },
            AssemblyLine::Label(label) => {
                println!("label {}", label);
                self.labels.insert(label.clone(), self.output.len() - 1);
            },
            AssemblyLine::Instruction(name, list) => {
                self.convert_instruction(lineno, name, list)?;
            },
        }
        Ok(())
    }

    fn convert_instruction(&mut self, lineno: usize, mneumonic: &str, args: &[AssemblyOperand]) -> Result<(), Error> {
        match mneumonic {
            "bra" => {
                let label = parser::expect_label(lineno, args)?;
                self.output.push(0x6000);
                self.relocations.push(Relocation::new(
                    RelocationType::Displacement,
                    label,
                    self.output.len() - 1,
                    self.current_origin,
                ));
            },
            "bsr" => {
                let label = parser::expect_label(lineno, args)?;
                self.output.push(0x6100);
                self.relocations.push(Relocation::new(
                    RelocationType::Displacement,
                    label,
                    self.output.len() - 1,
                    self.current_origin,
                ));
            },
            "illegal" => {
                self.output.push(0x4AFC);
            },

            "lea" => {
                parser::expect_args(lineno, args, 2)?;
                let reg = expect_address_register(lineno, &args[0])?;
                let (effective_address, additional_words) =
                    convert_target(lineno, &args[1], Size::Long, Disallow::NoRegsPrePostOrImmediate)?;
                self.output.push(0x41C0 | (reg << 9) | effective_address);
                self.output.extend(additional_words);
            },
            "nop" => {
                self.output.push(0x4E71);
            },
            "rts" => {
                self.output.push(0x4E75);
            },
            "rte" => {
                self.output.push(0x4E73);
            },
            "rtr" => {
                self.output.push(0x4E77);
            },
            "stop" => {
                let immediate = parser::expect_immediate(lineno, &args[0])?;
                self.output.push(0x4E72);
                self.output.extend(convert_immediate(lineno, immediate, Size::Word)?);
            },
            "trapv" => {
                self.output.push(0x4E76);
            },

            _ => {
                self.convert_sized_instruction(lineno, mneumonic, args)?;
            },
        }
        Ok(())
    }

    fn convert_sized_instruction(&mut self, lineno: usize, mneumonic: &str, args: &[AssemblyOperand]) -> Result<(), Error> {
        let operation_size = get_size_from_mneumonic(mneumonic)
            .ok_or_else(|| Error::new(format!("error at line {}: expected a size specifier (b/w/l)", lineno)));
        match &mneumonic[..mneumonic.len() - 1] {
            "addi" => {
                self.convert_common_immediate_instruction(lineno, 0x0600, args, operation_size?, Disallow::NoARegImmediateOrPC)?;
            },
            "addai" => {
                self.convert_common_immediate_instruction(lineno, 0x0600, args, operation_size?, Disallow::OnlyAReg)?;
            },
            "add" => {
                self.convert_common_dreg_instruction(lineno, 0xD000, args, operation_size?, Disallow::None)?;
            },
            "adda" => {
                self.convert_common_areg_instruction(lineno, 0xD000, args, operation_size?, Disallow::None)?;
            },
            "andi" => {
                if !self.check_convert_flags_instruction(lineno, 0x23C, 0x27C, args)? {
                    self.convert_common_immediate_instruction(
                        lineno,
                        0x0200,
                        args,
                        operation_size?,
                        Disallow::NoARegImmediateOrPC,
                    )?;
                }
            },
            "and" => {
                self.convert_common_dreg_instruction(lineno, 0xC000, args, operation_size?, Disallow::None)?;
            },
            "asr" | "asl" => {
                self.convert_common_shift_instruction(lineno, mneumonic, 0xE000, args, operation_size?)?;
            },

            "clr" => {
                self.convert_common_single_operand_instruction(
                    lineno,
                    0x4200,
                    args,
                    operation_size?,
                    Disallow::NoARegImmediateOrPC,
                )?;
            },
            "cmpi" => {
                self.convert_common_immediate_instruction(lineno, 0x0C00, args, operation_size?, Disallow::NoARegImmediateOrPC)?;
            },
            "cmp" => {
                self.convert_common_dreg_instruction(lineno, 0xB000, args, operation_size?, Disallow::None)?;
            },

            "eori" => {
                if !self.check_convert_flags_instruction(lineno, 0x0A3C, 0x0A7C, args)? {
                    self.convert_common_immediate_instruction(
                        lineno,
                        0x0A00,
                        args,
                        operation_size?,
                        Disallow::NoARegImmediateOrPC,
                    )?;
                }
            },
            "eor" => {
                self.convert_common_dreg_instruction(lineno, 0xB000, args, operation_size?, Disallow::NoARegImmediateOrPC)?;
            },

            "lsr" | "lsl" => {
                self.convert_common_shift_instruction(lineno, mneumonic, 0xE008, args, operation_size?)?;
            },

            "move" | "movea" => {
                let operation_size = operation_size?;
                parser::expect_args(lineno, args, 2)?;
                let (effective_address_left, additional_words_left) =
                    convert_target(lineno, &args[0], operation_size, Disallow::None)?;
                let (effective_address_right, additional_words_right) =
                    convert_target(lineno, &args[1], operation_size, Disallow::None)?;
                let effective_address_left = (effective_address_left >> 3) | (effective_address_left << 3);
                self.output
                    .push(encode_size_for_move(operation_size) | effective_address_left | effective_address_right);
                self.output.extend(additional_words_left);
                self.output.extend(additional_words_right);
            },

            "neg" => {
                self.convert_common_single_operand_instruction(
                    lineno,
                    0x4400,
                    args,
                    operation_size?,
                    Disallow::NoARegImmediateOrPC,
                )?;
            },
            "negx" => {
                self.convert_common_single_operand_instruction(
                    lineno,
                    0x4000,
                    args,
                    operation_size?,
                    Disallow::NoARegImmediateOrPC,
                )?;
            },
            "not" => {
                self.convert_common_single_operand_instruction(
                    lineno,
                    0x4600,
                    args,
                    operation_size?,
                    Disallow::NoARegImmediateOrPC,
                )?;
            },

            "ori" => {
                if !self.check_convert_flags_instruction(lineno, 0x003C, 0x007C, args)? {
                    self.convert_common_immediate_instruction(
                        lineno,
                        0x0000,
                        args,
                        operation_size?,
                        Disallow::NoARegImmediateOrPC,
                    )?;
                }
            },
            "or" => {
                self.convert_common_dreg_instruction(lineno, 0x8000, args, operation_size?, Disallow::NoARegImmediateOrPC)?;
            },

            "ror" | "rol" => {
                self.convert_common_shift_instruction(lineno, mneumonic, 0xE018, args, operation_size?)?;
            },

            "roxr" | "roxl" => {
                self.convert_common_shift_instruction(lineno, mneumonic, 0xE010, args, operation_size?)?;
            },

            "subi" => {
                self.convert_common_immediate_instruction(lineno, 0x0400, args, operation_size?, Disallow::NoARegImmediateOrPC)?;
            },
            "subai" => {
                self.convert_common_immediate_instruction(lineno, 0x0400, args, operation_size?, Disallow::OnlyAReg)?;
            },
            "sub" => {
                self.convert_common_dreg_instruction(lineno, 0x9000, args, operation_size?, Disallow::None)?;
            },
            "suba" => {
                self.convert_common_areg_instruction(lineno, 0x9000, args, operation_size?, Disallow::None)?;
            },

            // TODO complete remaining instructions
            _ => return Err(Error::new(format!("unrecognized instruction at line {}: {:?}", lineno, mneumonic))),
        }
        Ok(())
    }

    fn convert_common_immediate_instruction(
        &mut self,
        lineno: usize,
        opcode: u16,
        args: &[AssemblyOperand],
        operation_size: Size,
        disallow: Disallow,
    ) -> Result<(), Error> {
        parser::expect_args(lineno, args, 2)?;
        let immediate = parser::expect_immediate(lineno, &args[0])?;
        let (effective_address, additional_words) = convert_target(lineno, &args[1], operation_size, disallow)?;
        self.output.push(opcode | encode_size(operation_size) | effective_address);
        self.output.extend(convert_immediate(lineno, immediate, operation_size)?);
        self.output.extend(additional_words);
        Ok(())
    }

    fn convert_common_dreg_instruction(
        &mut self,
        lineno: usize,
        opcode: u16,
        args: &[AssemblyOperand],
        operation_size: Size,
        disallow: Disallow,
    ) -> Result<(), Error> {
        parser::expect_args(lineno, args, 2)?;
        let (direction, reg, operand) = convert_reg_and_other(lineno, args, Disallow::NoAReg)?;
        let (effective_address, additional_words) = convert_target(lineno, operand, operation_size, disallow)?;
        self.output
            .push(opcode | encode_size(operation_size) | direction | (reg << 9) | effective_address);
        self.output.extend(additional_words);
        Ok(())
    }

    fn convert_common_areg_instruction(
        &mut self,
        lineno: usize,
        opcode: u16,
        args: &[AssemblyOperand],
        operation_size: Size,
        disallow: Disallow,
    ) -> Result<(), Error> {
        let size_bit = expect_a_instruction_size(lineno, operation_size)?;
        parser::expect_args(lineno, args, 2)?;
        //let (_direction, reg, operand) = convert_reg_and_other(lineno, args, Disallow::NoDReg)?;
        let reg = expect_address_register(lineno, &args[1])?;
        let (effective_address, additional_words) = convert_target(lineno, &args[0], operation_size, disallow)?;
        self.output
            .push(opcode | size_bit | (0b11 << 6) | (reg << 9) | effective_address);
        self.output.extend(additional_words);
        Ok(())
    }

    fn convert_common_single_operand_instruction(
        &mut self,
        lineno: usize,
        opcode: u16,
        args: &[AssemblyOperand],
        operation_size: Size,
        disallow: Disallow,
    ) -> Result<(), Error> {
        parser::expect_args(lineno, args, 1)?;
        let (effective_address, additional_words) = convert_target(lineno, &args[0], operation_size, disallow)?;
        self.output.push(opcode | encode_size(operation_size) | effective_address);
        self.output.extend(additional_words);
        Ok(())
    }

    fn check_convert_flags_instruction(
        &mut self,
        lineno: usize,
        opcode_ccr: u16,
        opcode_sr: u16,
        args: &[AssemblyOperand],
    ) -> Result<bool, Error> {
        if let AssemblyOperand::Register(name) = &args[1] {
            let opcode = match name.as_str() {
                "ccr" => Some(opcode_ccr),
                "sr" => Some(opcode_sr),
                _ => None,
            };

            if let Some(opcode) = opcode {
                parser::expect_args(lineno, args, 2)?;
                let immediate = parser::expect_immediate(lineno, &args[0])?;
                self.output.push(opcode);
                self.output.extend(convert_immediate(lineno, immediate, Size::Word)?);
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn convert_common_shift_instruction(
        &mut self,
        lineno: usize,
        mneumonic: &str,
        opcode: u16,
        args: &[AssemblyOperand],
        operation_size: Size,
    ) -> Result<(), Error> {
        let dirstr = &mneumonic[mneumonic.len() - 2..mneumonic.len() - 1];
        let direction = if dirstr == "r" {
            0
        } else if dirstr == "l" {
            1 << 8
        } else {
            return Err(Error::new(format!(
                "error at line {}: expected direction of (l)eft or (r)ight, but found {:?}",
                lineno, dirstr
            )));
        };

        match &args {
            [AssemblyOperand::Immediate(_), AssemblyOperand::Register(_)] => {
                let mut immediate = parser::expect_immediate(lineno, &args[0])?;
                if !(1..=8).contains(&immediate) {
                    return Err(Error::new(format!(
                        "error at line {}: immediate value must be between 1 and 8, found {:?}",
                        lineno, args
                    )));
                } else if immediate == 8 {
                    immediate = 0;
                }

                let reg = expect_data_register(lineno, &args[1])?;
                self.output
                    .push(opcode | ((immediate as u16) << 9) | direction | encode_size(operation_size) /*(0b0 << 5)*/ | reg);
            },
            [AssemblyOperand::Register(_), AssemblyOperand::Register(_)] => {
                let bit_reg = expect_data_register(lineno, &args[0])?;
                let reg = expect_data_register(lineno, &args[1])?;
                self.output
                    .push(opcode | (bit_reg << 9) | direction | encode_size(operation_size) | (0b1 << 5) | reg);
            },
            //[_] => {
            //    let (effective_address, additional_words) = convert_target(lineno, &args[0], Size::Word, Disallow::NoRegsImmediateOrPC)?;
            //    self.output.push(opcode | effective_address);
            //    self.output.extend(additional_words);
            //},
            _ => return Err(Error::new(format!("error at line {}: unexpected addressing mode, found {:?}", lineno, args))),
        }
        Ok(())
    }
}

fn convert_target(lineno: usize, operand: &AssemblyOperand, size: Size, disallow: Disallow) -> Result<(u16, Vec<u16>), Error> {
    match operand {
        AssemblyOperand::Register(name) => convert_register(lineno, name, disallow),
        AssemblyOperand::Immediate(value) => {
            disallow.check(lineno, Disallow::NoImmediate)?;
            Ok((0b111100, convert_immediate(lineno, *value, size)?))
        },
        AssemblyOperand::Indirect(args) => convert_indirect(lineno, args, disallow),
        AssemblyOperand::IndirectPost(args, operator) => {
            disallow.check(lineno, Disallow::NoIndirectPost)?;
            if args.len() == 1 && operator == "+" {
                if let AssemblyOperand::Register(name) = &args[0] {
                    if name.starts_with('a') {
                        let reg = expect_reg_num(lineno, name)?;
                        return Ok(((0b011 << 3) | reg, vec![]));
                    }
                }
            }
            Err(Error::new(format!(
                "error at line {}: post-increment operator can only be used with a single address register",
                lineno
            )))
        },
        AssemblyOperand::IndirectPre(operator, args) => {
            disallow.check(lineno, Disallow::NoIndirectPre)?;
            if args.len() == 1 && operator == "-" {
                if let AssemblyOperand::Register(name) = &args[0] {
                    if name.starts_with('a') {
                        let reg = expect_reg_num(lineno, name)?;
                        return Ok(((0b100 << 3) | reg, vec![]));
                    } else if name == "sp" {
                        return Ok((0b100111, vec![]));
                    }
                }
            }
            Err(Error::new(format!(
                "error at line {}: pre-decrement operator can only be used with a single address register",
                lineno
            )))
        },
        _ => Err(Error::new(format!("not implemented: {:?}", operand))),
    }
}

fn convert_register(lineno: usize, name: &str, disallow: Disallow) -> Result<(u16, Vec<u16>), Error> {
    match name {
        name if name.starts_with('d') => {
            disallow.check(lineno, Disallow::NoDReg)?;
            let reg = expect_reg_num(lineno, name)?;
            Ok((/*(0b000 << 3)*/ reg, vec![]))
        },
        name if name.starts_with('a') => {
            disallow.check(lineno, Disallow::NoAReg)?;
            let reg = expect_reg_num(lineno, name)?;
            Ok(((0b001 << 3) | reg, vec![]))
        },
        "sp" => {
            disallow.check(lineno, Disallow::NoAReg)?;
            Ok(((0b001 << 3) | 7, vec![]))
        },
        _ => Err(Error::new(format!("error at line {}: invalid register {:?}", lineno, name))),
    }
}

fn convert_indirect(lineno: usize, args: &[AssemblyOperand], disallow: Disallow) -> Result<(u16, Vec<u16>), Error> {
    match &args {
        [AssemblyOperand::Register(name)] => {
            disallow.check(lineno, Disallow::NoIndirect)?;
            let reg = expect_address_reg_num(lineno, name)?;
            Ok(((0b010 << 3) | reg, vec![]))
        },
        [AssemblyOperand::Immediate(address)] => {
            disallow.check(lineno, Disallow::NoIndirectImmediate)?;
            if *address < u16::MAX as usize {
                Ok((0b111000, convert_immediate(lineno, *address, Size::Word)?))
            } else {
                Ok((0b111001, convert_immediate(lineno, *address, Size::Long)?))
            }
        },
        [AssemblyOperand::Immediate(offset), AssemblyOperand::Register(name)] => {
            if name == "pc" {
                disallow.check(lineno, Disallow::NoPCRelative)?;
                Ok((0b111010, convert_immediate(lineno, *offset, Size::Word)?))
            } else {
                disallow.check(lineno, Disallow::NoIndirectOffset)?;
                let reg = expect_address_reg_num(lineno, name)?;
                Ok(((0b101 << 3) | reg, convert_immediate(lineno, *offset, Size::Word)?))
            }
        },
        [AssemblyOperand::Immediate(offset), AssemblyOperand::Register(name), AssemblyOperand::Register(index)] => {
            let index_reg = expect_reg_num(lineno, index)?;
            let da_select = if index.starts_with('a') { 1 << 15 } else { 0 };
            if name == "pc" {
                disallow.check(lineno, Disallow::NoPCRelativeIndex)?;
                Ok((0b111011, vec![da_select | (index_reg << 12) | ((*offset as u16) & 0xff)]))
            } else {
                disallow.check(lineno, Disallow::NoIndirectIndexReg)?;
                let reg = expect_address_reg_num(lineno, name)?;
                Ok(((0b110 << 3) | reg, vec![da_select | (index_reg << 12) | ((*offset as u16) & 0xff)]))
            }
        },
        // TODO add the MC68020 address options
        _ => Err(Error::new(format!(
            "error at line {}: expected valid indirect addressing mode, but found {:?}",
            lineno, args
        ))),
    }
}

fn convert_reg_and_other(
    lineno: usize,
    args: &[AssemblyOperand],
    _disallow: Disallow,
) -> Result<(u16, u16, &AssemblyOperand), Error> {
    match &args {
        [AssemblyOperand::Register(reg), effective_address] => Ok(((0b1 << 8), expect_reg_num(lineno, reg)?, effective_address)),
        [effective_address, AssemblyOperand::Register(reg)] => Ok(((0b0 << 8), expect_reg_num(lineno, reg)?, effective_address)),
        _ => Err(Error::new(format!(
            "error at line {}: expected register and effective address, but found {:?}",
            lineno, args
        ))),
    }
}

fn convert_immediate(lineno: usize, value: usize, size: Size) -> Result<Vec<u16>, Error> {
    match size {
        Size::Byte => {
            if value <= u8::MAX as usize {
                Ok(vec![value as u16])
            } else {
                Err(Error::new(format!(
                    "error at line {}: immediate number is out of range; must be less than {}, but number is {:?}",
                    lineno,
                    u8::MAX,
                    value
                )))
            }
        },
        Size::Word => {
            if value <= u16::MAX as usize {
                Ok(vec![value as u16])
            } else {
                Err(Error::new(format!(
                    "error at line {}: immediate number is out of range; must be less than {}, but number is {:?}",
                    lineno,
                    u16::MAX,
                    value
                )))
            }
        },
        Size::Long => Ok(vec![(value >> 16) as u16, value as u16]),
    }
}

fn expect_data_register(lineno: usize, operand: &AssemblyOperand) -> Result<u16, Error> {
    if let AssemblyOperand::Register(name) = operand {
        if name.starts_with('d') {
            return expect_reg_num(lineno, name);
        }
    }
    Err(Error::new(format!("error at line {}: expected a data register, but found {:?}", lineno, operand)))
}

fn expect_address_register(lineno: usize, operand: &AssemblyOperand) -> Result<u16, Error> {
    if let AssemblyOperand::Register(name) = operand {
        if name.starts_with('a') {
            return expect_reg_num(lineno, name);
        }
    }
    Err(Error::new(format!(
        "error at line {}: expected an address register, but found {:?}",
        lineno, operand
    )))
}

fn expect_address_reg_num(lineno: usize, name: &str) -> Result<u16, Error> {
    if name.starts_with('a') {
        return expect_reg_num(lineno, name);
    }
    Err(Error::new(format!("error at line {}: expected an address register, but found {:?}", lineno, name)))
}

fn expect_reg_num(lineno: usize, name: &str) -> Result<u16, Error> {
    if let Ok(number) = str::parse::<u16>(&name[1..]) {
        if number <= 7 {
            return Ok(number);
        }
    }
    Err(Error::new(format!("error at line {}: no such register {:?}", lineno, name)))
}

fn expect_a_instruction_size(lineno: usize, size: Size) -> Result<u16, Error> {
    match size {
        Size::Word => Ok(0),
        Size::Long => Ok(0b1 << 8),
        _ => Err(Error::new(format!("error at line {}: address instructions can only be word or long size", lineno))),
    }
}


fn get_size_from_mneumonic(s: &str) -> Option<Size> {
    let size_ch = s.chars().last()?;
    match size_ch {
        'b' => Some(Size::Byte),
        'w' => Some(Size::Word),
        'l' => Some(Size::Long),
        _ => None,
    }
}

fn encode_size(size: Size) -> u16 {
    match size {
        Size::Byte => 0b00 << 6,
        Size::Word => 0b01 << 6,
        Size::Long => 0b10 << 6,
    }
}

fn encode_size_for_move(size: Size) -> u16 {
    match size {
        Size::Byte => 0b01 << 12,
        Size::Word => 0b11 << 12,
        Size::Long => 0b10 << 12,
    }
}

#[allow(dead_code)]
fn encode_size_bit(size: Size) -> Result<u16, Error> {
    match size {
        Size::Word => Ok(0b01 << 6),
        Size::Long => Ok(0b10 << 6),
        _ => Err(Error::new(format!("invalid size for this operation: {:?}", size))),
    }
}
