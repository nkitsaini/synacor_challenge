use ux::{u15, u3};
use anyhow::{bail, Context};

pub trait FillSlice<T> {
    fn copy_first(&mut self, x: &[T]);
}

impl FillSlice<u16> for [u16] {
    fn copy_first(&mut self, x: &[u16]) {
        assert!(self.len()>= x.len());
        self[..x.len()].copy_from_slice(&x);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Reg(u3);


impl From<u3> for Reg {
    fn from(value: u3) -> Self {
        Self(value)
    }
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
pub type Num = u15;
pub type Mem = u15;
pub type MemBlock = u16;

#[derive(Debug, Clone, Copy)]
pub enum Addr {
    Reg(Reg), // 0..7
    Mem(Mem), // 15-bit
}

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
pub trait IntoUsize {
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
pub enum Val {
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

#[derive(Debug, Clone, Copy)]
pub enum Op {
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


    pub fn convert_bytes(val: &[u8]) -> Vec<u16> {
        let mut rv = vec![0; val.len()/2];
        for i in 0..(val.len() / 2) -1 {
            let val = u16::from_le_bytes([val[i * 2], val[i* 2 + 1]]);
            rv[i] = val;
        }
        rv
    }

    pub fn parse(val: &[u16]) -> anyhow::Result<Self> {
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
            _ => bail!("Unknown op code: {}", val[0]),
        });
    }
}