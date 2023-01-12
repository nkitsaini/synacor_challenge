use core::num;
use std::ops::Mul;
use std::fs::File;
use std::io::prelude::*;
use ux::{u15, u3};
use anyhow::{bail, Context};

const DEBUG_PRINT: bool = true;

#[derive(Debug, Clone, Copy)]
struct Reg(u3);


impl From<u3> for Reg {
    fn from(value: u3) -> Self {
        Self(value)
    }
}
fn wrapping_mul(a: Num, b: Num) -> Num {
    let a_u16: u64 = a.into();
    let b_u16: u64 = b.into();
    let mul_u16 = a_u16 * b_u16;
    let num_max_u64: u64 = Num::MAX.into();
    let mul_u16_wrapped: u64 = mul_u16%(num_max_u64 + 1);
    mul_u16_wrapped.try_into().unwrap()
}
fn wrapping_add(a: Num, b: Num) -> Num {
    let a_u16: u64 = a.into();
    let b_u16: u64 = b.into();
    let mul_u16 = a_u16 + b_u16;
    let num_max_u64: u64 = Num::MAX.into();
    let mul_u16_wrapped: u64 = mul_u16%(num_max_u64 + 1);
    mul_u16_wrapped.try_into().unwrap()
}

fn wrapping_mod(a: Num, b: Num) -> Num {
    let a_u16: u16 = a.into();
    let b_u16: u16 = b.into();
    (a_u16%b_u16).try_into().unwrap()
}

impl TryFrom<u16> for Reg {
    type Error = anyhow::Error;
    fn try_from(value: u16) -> anyhow::Result<Self> {
        let u15_max: u16 = u15::MAX.into();
        if value <= u15::MAX.into() {
            bail!("Value is too small to be register: {}", value)
        } else if value - u15_max -1 >=8 {
            bail!("Value is too small to be register: {}", value)
        }
        let reg_val: u16 = value - u15_max -1;
        let v: u3 = reg_val.try_into().unwrap();
        Ok(Self(v))
    }
}

impl From<Val> for Addr {
    fn from(value: Val) -> Self {
        match value {
            Val::Num(x) => Self::Mem(x),
            Val::Reg(v) => Self::Reg(v)
        }
    }
}
impl From<Addr> for Val {
    fn from(value: Addr) -> Self {
        match value {
            Addr::Mem(x) => Self::Num(x),
            Addr::Reg(v) => Self::Reg(v)
        }
    }
}

// type Reg = u3;
type Num = u15;
type Mem = u15;
type MemBlock = u16;
type RegBlock = u16;

#[derive(Debug, Clone, Copy)]
enum Addr {
    Reg(Reg), // 0..7
    Mem(Mem), // 15-bit
}

// impl std::error::Error for ux::conversion::TryFromIntError {
//     fnn
// }


impl TryFrom<u16> for Addr {
    type Error = anyhow::Error;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        let v = Val::try_from(value)?;
        Ok(match v {
            Val::Num(n) => Self::Mem(n),
            Val::Reg(r) => Self::Reg(r),
        })
    }
}

impl From<u15> for Addr {
    fn from(value: u15) -> Self  {
        Self::Mem(value)
    }
}

impl From<u8> for Addr {
    fn from(value: u8) -> Self  {
        Self::Mem(value.into())
    }
}
trait IntoUsize {
    fn to_usize(&self) -> usize;
}
impl IntoUsize for u3 {
    fn to_usize(&self) -> usize {
        TryInto::<usize>::try_into(*self).unwrap()
    }
}
impl IntoUsize for u15 {
    fn to_usize(&self) -> usize {
        TryInto::<usize>::try_into(*self).unwrap()
    }
}
impl IntoUsize for Reg {
    fn to_usize(&self) -> usize {
        TryInto::<usize>::try_into(self.0).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
enum Val {
    Reg(Reg),
    Num(Num),
}

impl From<u15> for Val {
    fn from(value: u15) -> Self  {
        Self::Num(value)
    }
}

impl From<u8> for Val {
    fn from(value: u8) -> Self  {
        Self::Num(value.into())
    }
}

impl TryFrom<u16> for Val {
    type Error = anyhow::Error;
    fn try_from(value: u16) -> Result<Self, Self::Error> {
        Ok(match TryInto::<Num>::try_into(value){
            Ok(x) => Self::Num(x),
            Err(v) => {
                Self::Reg(value.try_into()?)
            }
        })
    }
}

struct ExecutionEnv {
    stack: Vec<MemBlock>,
    memory: [MemBlock; 32768],
    registers: [MemBlock; 8],
    curr_point: Mem,
    line_buffer: String
}

impl ExecutionEnv {
    pub fn new(content: Vec<u8>) -> Self {
        let mut rv = Self {
            stack: vec![],
            memory: [0u16; 32768],
            registers: [0u16; 8],
            curr_point: 0.into(),
            line_buffer: Default::default()
        };

        assert_eq!(content.len() % 2, 0, "Input is not 16-bit multiple");
        for i in 0..(content.len() / 2) -1 {
            let val = u16::from_le_bytes([content[i * 2], content[i* 2 + 1]]);
            rv.memory[i] = val;
        }

        rv
    }
    pub fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let mut values = [0u16; 4];
            for i in (self.curr_point.to_usize())
                ..(self.memory.len().min(self.curr_point.to_usize() + 4))
            {
                values[i - self.curr_point.to_usize()] = self.memory[i];
            }
            let op = Op::parse(values)?;
            if self.run_op(op)? {
                break
            };
        }
        Ok(())
    }
    pub fn resolve(&self, v: Val) -> anyhow::Result<MemBlock> {
        let rv: MemBlock = match v {
            Val::Num(n) => n.into(),
            Val::Reg(r) => self.registers[r.to_usize()],
        };
        // let v: Val = rv.try_into().unwrap();
        // assert!()
        // eprintln!("Resolve: {:?} -> {}", v, rv);
        // eprintln!("Registers: {:?}", self.registers);
        Ok(rv)
    }

    pub fn set_mem_from(&mut self, mem: Addr, val: Val) -> anyhow::Result<()> {
        let value = self.resolve(val)?;
        match mem {
            Addr::Mem(m) => self.memory[m.to_usize()] = value,
            Addr::Reg(r) => self.registers[r.to_usize()] = value,
        }
        Ok(())
    }

    pub fn set_mem(&mut self, mem: Addr, val: MemBlock) -> anyhow::Result<()> {
        match mem {
            Addr::Mem(m) => self.memory[m.to_usize()] = val,
            Addr::Reg(r) => self.registers[r.to_usize()] = val,
        }
        Ok(())
    }

    pub fn run_op(&mut self, op: Op) -> anyhow::Result<bool> {
        use Op::*;
        let mut jump_pos: Option<Mem> = None;
        // eprintln!("OPERATION: {:?}", &op);
        // eprintln!("Registers Before Op: {:?}", self.registers);

        match &op {
            Halt => {return Ok(true);},
            Set(ra, v) => {
                self.registers[ra.to_usize()] = self.resolve(*v)?.into();
            },
            Out(x) => {
                let r  = self.resolve(*x)?;
                print!("{}", r as u8 as char);
                std::io::stdout().flush()?;
            },
            Noop => { },
            Push(v) => {
                let v = self.resolve(*v)?;
                self.stack.push(v.into());
            },
            Pop(v) => {
                let val = self.stack.pop().context("Pop from empty stack")?;
                self.set_mem(*v, val.try_into().unwrap())?;
            },
            Eq(addr, a, b) => {
                // if self.registers == [844, 1, 844, 0, 0, 0, 0, 0] && self.resolve(*b)? == 10000 {
                //     self.set_mem(*addr, 1)?;
                // }
                // else
                if self.resolve(*a)? == self.resolve(*b)? {
                    self.set_mem(*addr, 1)?;
                } else {
                    self.set_mem(*addr, 0)?;
                }
            },
            Gt(addr, a, b) => {
                if self.resolve(*a)? > self.resolve(*b)? {
                    self.set_mem(*addr, 1)?;
                } else {
                    self.set_mem(*addr, 0)?;
                }
            },
            Jmp(addr) => {
                jump_pos = Some(*addr);
            },
            Jt(v, m) => {
                if self.resolve(*v)? != 0 {
                    jump_pos = Some(*m);
                }
            },
            Jf(v, m) => {
                if self.resolve(*v)? == 0 {
                    jump_pos = Some(*m);
                }
            },
            Add(addr, a, b) => {
                let a: u15 = self.resolve(*a)?.try_into()?;
                let b: u15 = self.resolve(*b)?.try_into()?;
                // self.set_mem(*addr,  a.wrapping_add(b).into())?;
                self.set_mem(*addr,  wrapping_add(a, b).into())?;
            },
            // Add(addr, a, b) => {
            //     let a = self.resolve(a)?;
            //     let b = self.resolve(b)?;
            //     self.set_mem(addr,  Val::Num(a+b))?;
            // },
            Mult(addr, a, b) => {
                let a: u15 = self.resolve(*a)?.try_into()?;
                let b: u15= self.resolve(*b)?.try_into()?;
                self.set_mem(*addr,  wrapping_mul(a, b).into())?;
            },
            Mod(addr, a, b) => {
                let a: u15 = self.resolve(*a)?.try_into()?;
                let b: u15= self.resolve(*b)?.try_into()?;
                self.set_mem(*addr,  wrapping_mod(a, b).into())?;
            },
            And(addr, a, b) => {
                let a: u15 = self.resolve(*a)?.try_into()?;
                let b: u15= self.resolve(*b)?.try_into()?;
                self.set_mem(*addr,  (a&b).into())?;
            },
            Or(addr, a, b) => {
                let a: u15 = self.resolve(*a)?.try_into()?;
                let b: u15= self.resolve(*b)?.try_into()?;
                self.set_mem(*addr,  (a|b).into())?;
            },
            Not(addr, a) => {
                let a: u15 = self.resolve(*a)?.try_into()?;
                self.set_mem(*addr,  (bit_not(a)).into())?;
            },
            Call(addr) => {
                // let next_execution = 
                let bts: u15 = op.param_bytes().into();
                let next_execution = self.curr_point + bts;
                self.stack.push(next_execution.into());

                let loc = self.resolve(*addr)?;
                jump_pos = Some(loc.try_into()?);
                // let a: u15 = self.resolve(*a)?.try_into()?;
                // self.set_mem(*addr,  (bit_not(a)).into())?;
            },
            Rmem(addr, a) => {
                // dbg!(addr, a);
                let m = match a {
                    Addr::Mem(x) => {
                        let x: u15 = *x;
                        self.memory[x.to_usize()]
                    }, 
                    Addr::Reg(r) => {
                        let x: u15 = self.resolve(Val::Reg(*r))?.try_into()?;
                        self.memory[x.to_usize()]
                    }
                };
                // dbg!(m);
                self.set_mem(*addr,  m)?;
            },
            Wmem(addr, a) => {
                // Maybe read from memory <a> instead of a as value
                // let v = self.resolve(*a)?;

                let mut faddr = *addr;
                
                let b: Val = (*addr).into();
                let x = self.resolve(b)?;
                let x: u15 = x.try_into()?;
                // let v = match addr {
                //     Addr::Reg(r) => {
                //         let x: u15 = self.resolve(Val::Reg(*r))?.try_into()?;
                //         Addr::Mem(x)
                //     }
                //     _ =
                // };
                // let v = self.resolve(*a)?;
                // dbg!(v);
                // self.set_mem(Addr::Mem(*addr),  v)?;
                // self.set_mem(*addr,  self.resolve(*a)?)?;
                self.set_mem(Addr::Mem(x),  self.resolve(*a)?)?;
            },
            Ret => {
                match self.stack.pop() {
                    Some(x) => {
                        jump_pos = Some(x.try_into()?)
                    },
                    None => return Ok(true)
                }
            },
            In(x) => {
                let v = self.read_one_stdin();
                self.set_mem(*x, v? as u16)?;
            }
            // _ => todo!(),
           
            _ => todo!()
    // Add(Addr, Val, Val),
    // /// 10: store into <a> the product of <b> and <c> (modulo 32768)
    // Mult(Addr, Val, Val),
    // /// 11: store into <a> the remainder of <b> divided by <c>
    // Mod(Addr, Val, Val),
    // /// 12: stores into <a> the bitwise and of <b> and <c>
    // And(Addr, Val, Val),
    // /// 13: stores into <a> the bitwise or of <b> and <c>
    // Or(Addr, Val, Val),
    // /// 14: stores 15-bit bitwise inverse of <b> in <a>
    // Not(Addr, Val),
    // /// 15: read memory at address <b> and write it to <a>
    // Rmem(Addr, MemAddr),
    // /// 16: write the value from <b> into memory at address <a>
    // Wmem(MemAddr, Val),
    // /// 17: write the address of the next instruction to the stack and jump to <a>
    // Call(MemAddr),
    // /// 18: remove the top element from the stack and jump to it; empty stack = halt
    // Ret,
    // /// 19: write the character represented by ascii code <a> to the terminal
    // Out(Val),
    // /// 20: read a character from the terminal and write its ascii code to <a>;
    // /// it can be assumed that once input starts, it will continue until a newline is encountered;
    // /// this means that you can safely read whole lines from the keyboard and trust
    // /// that they will be fully read
    // In(Addr),
    // /// 21: no operation
    // Noop,

        };
        match jump_pos {
            None => {
                let bts: u15 = op.param_bytes().into();
                self.curr_point = self.curr_point + bts;
                // jump_pos += op.param_bytes();
            },
            Some(x) => {
                self.curr_point = x;
            }
        }
        // eprintln!("Registers After Op: {:?}", self.registers);
        // eprintln!("====================>");

        return Ok(false);
    }

    pub fn read_one_stdin(&mut self) -> anyhow::Result<char> {
        if self.line_buffer.len() == 0 {
            let mut read_buf = String::new();
            std::io::stdin().read_line(&mut read_buf)?;
            self.line_buffer = read_buf.chars().rev().collect();
        }
        Ok(self.line_buffer.pop().unwrap())
    }
}

#[derive(Debug, Clone, Copy)]
enum Op {
    /// 0: stop execution and terminate the program
    Halt,
    /// 1: set register <a> to the value of <b>
    Set(Reg, Val),
    /// 2: push <a> onto the stack
    Push(Val),
    /// 3: remove the top element from the stack and write it into <a>; empty stack = error
    Pop(Addr),
    /// 4: set <a> to 1 if <b> is equal to <c>; set it to 0 otherwise
    Eq(Addr, Val, Val),
    /// 5: set <a> to 1 if <b> is greater than <c>; set it to 0 otherwise
    Gt(Addr, Val, Val),
    /// 6: jump to <a>
    Jmp(Mem),
    /// 7: if <a> is nonzero, jump to <b>
    Jt(Val, Mem),
    /// 8: if <a> is zero, jump to <b>
    Jf(Val, Mem),
    /// 9: assign into <a> the sum of <b> and <c> (modulo 32768)
    Add(Addr, Val, Val),
    /// 10: store into <a> the product of <b> and <c> (modulo 32768)
    Mult(Addr, Val, Val),
    /// 11: store into <a> the remainder of <b> divided by <c>
    Mod(Addr, Val, Val),
    /// 12: stores into <a> the bitwise and of <b> and <c>
    And(Addr, Val, Val),
    /// 13: stores into <a> the bitwise or of <b> and <c>
    Or(Addr, Val, Val),
    /// 14: stores 15-bit bitwise inverse of <b> in <a>
    Not(Addr, Val),
    /// 15: read memory at address <b> and write it to <a>
    Rmem(Addr, Addr),
    /// 16: write the value from <b> into memory at address <a>
    Wmem(Addr, Val),
    /// 17: write the address of the next instruction to the stack and jump to <a>
    Call(Val), // should be ideally Mem but not sure why register 1 is specified
    /// 18: remove the top element from the stack and jump to it; empty stack = halt
    Ret,
    // /// 19: write the character represented by ascii code <a> to the terminal
    Out(Val),
    /// 20: read a character from the terminal and write its ascii code to <a>;
    /// it can be assumed that once input starts, it will continue until a newline is encountered;
    /// this means that you can safely read whole lines from the keyboard and trust
    /// that they will be fully read
    In(Addr),
    /// 21: no operation
    Noop,
}

impl Op {
    pub fn param_bytes(&self) -> u8 {
        match self {
            Self::Halt => 1,
            Self::Ret => 1,
            Self::Noop => 1,

            Self::Call(_) => 2,
            Self::Push(_) => 2,
            Self::Pop(_) => 2,
            Self::In(_) => 2,
            Self::Out(_) => 2,
            Self::Jmp(_) => 2,

            Self::Set(_, _) => 3,
            Self::Jt(_, _) => 3,
            Self::Jf(_, _) => 3,
            Self::Rmem(_, _) => 3,
            Self::Wmem(_, _) => 3,
            Self::Not(_, _) => 3,

            Self::Eq(_, _, _) => 4,
            Self::Gt(_, _, _) => 4,
            Self::Add(_, _, _) => 4,
            Self::Mod(_, _, _) => 4,
            Self::Mult(_, _, _) => 4,
            Self::And(_, _, _) => 4,
            Self::Or(_, _, _) => 4,
        }
    }
    pub fn parse(val: [u16; 4]) -> anyhow::Result<Self> {
        // dbg!("parsing op code", val);
        // let a: Val = val[1].try_into().ok().context("a")?;
        // let a: Val = val[2].try_into().ok().context("b")?;
        // let b: Mem = val[1].try_into()?;
        // eprintln!("Values: {:?}", val);
        return Ok(match val[0] {
            0 => Self::Halt,
            1 => Self::Set(val[1].try_into()?, val[2].try_into()?),
            2 => Self::Push(val[1].try_into()?),
            3 => Self::Pop(val[1].try_into()?),
            4 => Self::Eq(val[1].try_into()?, val[2].try_into()?, val[3].try_into()?),
            5 => Self::Gt(val[1].try_into()?, val[2].try_into()?, val[3].try_into()?),
            6 => Self::Jmp(val[1].try_into()?),
            7 => Self::Jt(val[1].try_into()?, val[2].try_into()?),
            8 => Self::Jf(val[1].try_into()?, val[2].try_into()?),
            9 => Self::Add(val[1].try_into()?, val[2].try_into()?, val[3].try_into()?),
            10 => Self::Mult(val[1].try_into()?, val[2].try_into()?, val[3].try_into()?),
            11 => Self::Mod(val[1].try_into()?, val[2].try_into()?, val[3].try_into()?),
            12 => Self::And(val[1].try_into()?, val[2].try_into()?, val[3].try_into()?),
            13 => Self::Or(val[1].try_into()?, val[2].try_into()?, val[3].try_into()?),
            14 => Self::Not(val[1].try_into()?, val[2].try_into()?),
            15 => Self::Rmem(val[1].try_into()?, val[2].try_into()?),
            16 => Self::Wmem(val[1].try_into()?, val[2].try_into()?),
            17 => Self::Call(val[1].try_into()?),
            18 => Self::Ret,
            19 => Self::Out(val[1].try_into()?),
            20 => Self::In(val[1].try_into()?),
            21 => Self::Noop,
            _ => bail!("Unimplemented op code: {}", val[0]),
        });
    }
}

fn main() -> anyhow::Result<()> {
    let mut file = File::open("challenge.bin")?;
    let mut buf = vec![];
    file.read_to_end(&mut buf)?;
    let mut env = ExecutionEnv::new(buf);
    env.run()?;
    // println!("Hello, world!");
    Ok(())
}

fn bit_not(x: Num) -> Num {
    !x & Num::MAX
}
// #[test]
// fn repl() {
//     let a=u15::MAX;
//     let b: u15 = 0b001.into();
//     let c = !b;
//     let d = bit_not(b);
//     println!("a: {}, b: {}, c: {}, d: {}", a, b, c, d);
//     assert!(false);
// }
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_mod_add() {
        // let a = u15::MAX;
        let a: u15 = 32758u16.try_into().unwrap();
        let b: u15 = 15u16.try_into().unwrap();
        assert_eq!(wrapping_add(a, b), 5.into());
    }
    #[test]
    fn test_mod_mul() {
        let b: u15 = 500u16.try_into().unwrap();
        let a: u15 = 77u16.try_into().unwrap();
        let c: u15 = 5732u16.try_into().unwrap();
        // direct = 38500
        // module = 38500 - 32767 = 5733
        assert_eq!(wrapping_mul(a, b), c);
    }

}
