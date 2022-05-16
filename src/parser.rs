
use std::str::Chars;
use std::iter::Peekable;

use crate::error::Error;


#[derive(Debug)]
pub enum AssemblyLine {
    Directive(String, Vec<String>),
    Label(String),
    Instruction(String, Vec<AssemblyOperand>),
}

#[derive(Debug)]
pub enum AssemblyOperand {
    Register(String),
    Indirect(Vec<AssemblyOperand>),
    IndirectPre(String, Vec<AssemblyOperand>),
    IndirectPost(Vec<AssemblyOperand>, String),
    Immediate(usize),
    Label(String),
}


pub struct AssemblyParser<'input> {
    lexer: AssemblyLexer<'input>,
}

impl<'input> AssemblyParser<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            lexer: AssemblyLexer::new(input),
        }
    }

    pub fn parse(&mut self) -> Result<Vec<(usize, AssemblyLine)>, Error> {
        let mut output = vec![];
        loop {
            let lineno = self.lexer.get_next_lineno();
            if let Some(line) = self.parse_line()? {
                output.push((lineno, line));
            } else {
                break;
            }
        }
        Ok(output)
    }

    fn parse_line(&mut self) -> Result<Option<AssemblyLine>, Error> {
        let token = loop {
            match self.lexer.get_next() {
                Some(token) if token == "\n" => { },
                Some(token) => { break token; }
                None => { return Ok(None); },
            }
        };

        let result = match token.as_str() {
            "." => {
                let name = self.lexer.expect_next()?;
                let list = self.parse_list_of_words()?;
                AssemblyLine::Directive(name, list)
            },
            word if word.chars().nth(0).map(|ch| is_word(ch)).unwrap_or(false) => {
                let next = self.lexer.peek();
                if next.is_some() && next.as_ref().unwrap() == ":" {
                    self.lexer.expect_next()?;
                    AssemblyLine::Label(token)
                } else {
                    let list = self.parse_list_of_operands()?;
                    AssemblyLine::Instruction(token, list)
                }
            },
            _ => {
                return Err(Error::new(&format!("parse error at line {}: expected word, found {:?}", self.lexer.lineno(), token)));
            },
        };

        self.lexer.expect_end()?;
        Ok(Some(result))
    }

    fn parse_list_of_words(&mut self) -> Result<Vec<String>, Error> {
        let mut list = vec![];

        // If we're already at the end of the line, then it's an empty list, so return
        let next = self.lexer.peek();
        if next.is_none() || next.as_ref().unwrap() == "\n" {
            return Ok(list);
        }

        loop {
            list.push(self.lexer.expect_next()?);

            let next = self.lexer.peek();
            if next.is_none() || next.as_ref().unwrap() != "," {
                return Ok(list);
            }
            self.lexer.expect_next()?;
        }
    }

    fn parse_list_of_operands(&mut self) -> Result<Vec<AssemblyOperand>, Error> {
        let mut list = vec![];

        // If we're already at the end of the line, then it's an empty list, so return
        let next = self.lexer.peek();
        if next.is_none() || next.as_ref().unwrap() == "\n" {
            return Ok(list);
        }

        loop {
            list.push(self.parse_operand()?);

            let next = self.lexer.peek();
            if next.is_none() || next.unwrap() != "," {
                return Ok(list);
            }
            self.lexer.expect_next()?;
        }
    }

    fn parse_operand(&mut self) -> Result<AssemblyOperand, Error> {
        let token = self.lexer.expect_next()?;
        match token.as_str() {
            "%" => {
                // TODO check for movem type ranges
                Ok(AssemblyOperand::Register(self.lexer.expect_next()?))
            },
            "(" => {
                let list = self.parse_list_of_operands()?;
                self.lexer.expect_token(")")?;

                if let Some(next) = self.lexer.peek() {
                    if next == "+" || next == "-" {
                        self.lexer.expect_next()?;
                        return Ok(AssemblyOperand::IndirectPost(list, next));
                    }
                }
                Ok(AssemblyOperand::Indirect(list))
            },
            "+" | "-" => {
                self.lexer.expect_token("(")?;
                let list = self.parse_list_of_operands()?;
                self.lexer.expect_token(")")?;
                Ok(AssemblyOperand::IndirectPre(token, list))
            },
            "#" => {
                let string = self.lexer.expect_next()?;
                let number = parse_any_number(self.lexer.lineno(), &string)?;
                Ok(AssemblyOperand::Immediate(number))
            },
            _ => {
                if is_digit(token.chars().nth(0).unwrap()) {
                    let number = parse_any_number(self.lexer.lineno(), &token)?;
                    Ok(AssemblyOperand::Immediate(number))
                } else {
                    Ok(AssemblyOperand::Label(token))
                }
            }
        }
    }
}

fn parse_any_number(lineno: usize, string: &str) -> Result<usize, Error> {
    let (radix, numeric) = if string.starts_with("0x") {
        (16, &string[2..])
    } else if string.starts_with("0b") {
        (2, &string[2..])
    } else if string.starts_with("0o") {
        (8, &string[2..])
    } else {
        (10, string)
    };
    usize::from_str_radix(numeric, radix)
        .map_err(|_| Error::new(&format!("parse error at line {}: expected number after #, but found {:?}", lineno, string)))
}


pub struct AssemblyLexer<'input> {
    lineno: usize,
    chars: Peekable<Chars<'input>>,
    peeked: Option<String>,
}

impl<'input> AssemblyLexer<'input> {
    pub fn new(input: &'input str) -> Self {
        Self {
            lineno: 1,
            chars: input.chars().peekable(),
            peeked: None,
        }
    }

    pub fn lineno(&self) -> usize {
        self.lineno
    }

    pub fn get_next_lineno(&mut self) -> usize {
        self.eat_whitespace();
        self.lineno
    }

    pub fn get_next(&mut self) -> Option<String> {
        if self.peeked.is_some() {
            let result = std::mem::replace(&mut self.peeked, None);
            return result;
        }

        self.eat_whitespace();

        let ch = self.chars.next()?;
        let mut string = ch.to_string();

        if is_word(ch) {
            while let Some(ch) = self.chars.next_if(|ch| is_word(*ch) || *ch == '.') {
                // Ignore periods in words
                if ch != '.' {
                    string.push(ch);
                }
            }
        }

        Some(string)
    }

    pub fn peek(&mut self) -> Option<String> {
        self.peeked = self.get_next();
        self.peeked.clone()
    }

    pub fn expect_next(&mut self) -> Result<String, Error> {
        self.get_next().ok_or_else(|| Error::new(&format!("unexpected end of input at line {}", self.lineno)))
    }

    pub fn expect_token(&mut self, expected: &str) -> Result<(), Error> {
        let token = self.expect_next()?;
        if token == expected {
            Ok(())
        } else {
            Err(Error::new(&format!("parse error at line {}: expected {:?}, found {:?}", self.lineno, expected, token)))
        }
    }

    pub fn expect_end(&mut self) -> Result<(), Error> {
        let token = self.get_next();
        if token.is_none() || token.as_ref().unwrap() == "\n" {
            Ok(())
        } else {
            Err(Error::new(&format!("expected end of line at {}: found {:?}", self.lineno, token)))
        }
    }

    fn eat_whitespace(&mut self) {
        while let Some(ch) = self.chars.peek() {
            if *ch == '|' {
                self.read_until('\n')
            } else if *ch == '/' {
                self.chars.next();
                if self.chars.next_if(|ch| *ch == '*').is_some() {
                    loop {
                        self.read_until('*');
                        self.chars.next();
                        if self.chars.next_if(|ch| *ch == '/').is_some() {
                            break;
                        }
                    }
                } else {

                }
            } else if *ch == ' ' || *ch == '\t' || *ch == '\r' {
                self.chars.next();
            } else {
                if *ch == '\n' {
                    self.lineno += 1;
                }
                break;
            }
        }
    }

    fn read_until(&mut self, test: char) {
        while let Some(ch) = self.chars.peek() {
            if *ch == test {
                return;
            }
            if *ch == '\n' {
                self.lineno += 1;
            }
            self.chars.next();
        }
    }
}

fn is_word(ch: char) -> bool {
    ('a'..='z').contains(&ch) || ('A'..='Z').contains(&ch) || ('0'..='9').contains(&ch) || (ch == '_')
}

fn is_digit(ch: char) -> bool {
    ('0'..='9').contains(&ch)
}

pub fn expect_args(lineno: usize, args: &[AssemblyOperand], expected: usize) -> Result<(), Error> {
    if args.len() == expected {
        Ok(())
    } else {
        Err(Error::new(&format!("error at line {}: expected {} args, but found {}", lineno, expected, args.len())))
    }
}

pub fn expect_label(lineno: usize, args: &[AssemblyOperand]) -> Result<String, Error> {
    expect_args(lineno, args, 1)?;
    if let AssemblyOperand::Label(name) = &args[0] {
        Ok(name.clone())
    } else {
        Err(Error::new(&format!("error at line {}: expected a label, but found {:?}", lineno, args)))
    }
}

pub fn expect_immediate(lineno: usize, operand: &AssemblyOperand) -> Result<usize, Error> {
    if let AssemblyOperand::Immediate(value) = operand {
        Ok(*value)
    } else {
        Err(Error::new(&format!("error at line {}: expected an immediate value, but found {:?}", lineno, operand)))
    }
}

