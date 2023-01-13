use core::num;
use std::{ops::Mul, sync::mpsc::Receiver};
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

#[derive(Debug, Clone, Copy)]
enum Addr {
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

pub struct ExecutionEnv {
    pub(crate) stack: Vec<MemBlock>,
    pub(crate) memory: [MemBlock; 32768],
    pub(crate) registers: [MemBlock; 8],
    pub(crate) curr_point: Mem,
    pub(crate) screen: Screen,
	pub(crate) register_8_preset: Option<u16>
}

pub struct Screen {
    pub(crate) text_recv: std::sync::mpsc::Receiver<String>,
    pub(crate) text_send: std::sync::mpsc::Sender<String>,
    pub(crate) buffer: String
}

impl Screen {
    pub fn create() -> (Screen, Screen) {
        let (tx, rx) = std::sync::mpsc::channel();
        let (tx2, rx2) = std::sync::mpsc::channel();
        return (Screen{text_recv: rx, text_send: tx2, buffer: "".into()}, Screen{text_recv: rx2, text_send: tx, buffer: "".into()})
    }
    pub fn send(&mut self, val: String) -> anyhow::Result<()> {
        self.text_send.send(val)?;
        Ok(())
    }
    pub fn send_char(&mut self, val: char) -> anyhow::Result<()> {
        self.text_send.send(val.to_string())?;
        Ok(())
    }
    pub fn get_char(&mut self) -> anyhow::Result<char> {
        while self.buffer.is_empty() {
            let data = self.text_recv.recv()?;
            self.buffer = data.chars().rev().collect();
        }
        Ok(self.buffer.pop().unwrap())
    }

    /// consume all the strings in recv
    pub fn reset(&mut self) -> anyhow::Result<()> {
        while let Ok(x) = self.text_recv.try_recv() { }
        Ok(())
    }
}

impl ExecutionEnv {
    pub fn new(content: &[u8], screen: Screen, register_preset: Option<u16>) -> Self {
        let mut rv = Self {
            stack: vec![],
            memory: [0u16; 32768],
            registers: [0u16; 8],
            curr_point: 0.into(),
            screen: screen,
			register_8_preset: register_preset
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
    fn resolve(&self, v: Val) -> anyhow::Result<MemBlock> {
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

    fn set_mem_from(&mut self, mem: Addr, val: Val) -> anyhow::Result<()> {
        let value = self.resolve(val)?;
        match mem {
            Addr::Mem(m) => self.memory[m.to_usize()] = value,
            Addr::Reg(r) => self.registers[r.to_usize()] = value,
        }
        Ok(())
    }

    fn set_mem(&mut self, mem: Addr, val: MemBlock) -> anyhow::Result<()> {
        match mem {
            Addr::Mem(m) => self.memory[m.to_usize()] = val,
            Addr::Reg(r) => self.registers[r.to_usize()] = val,
        }
        Ok(())
    }
	
    pub fn check_teleporter(&mut self) -> anyhow::Result<bool> {
        loop {
            let mut values = [0u16; 4];
            for i in (self.curr_point.to_usize())
                ..(self.memory.len().min(self.curr_point.to_usize() + 4))
            {
                values[i - self.curr_point.to_usize()] = self.memory[i];
            }
            let op = Op::parse(values)?;
            if self.run_op(op)? {
				if let Op::Call(x) = op {
					return Ok(false)

				}
            };
        }
        Ok(false)
    }

    fn run_op(&mut self, op: Op) -> anyhow::Result<bool> {
        use Op::*;
        let mut jump_pos: Option<Mem> = None;
		if let Op::Call(x) = &op {
			// dbg!("Call", x);
			if let Val::Num(a) = *x {
				let b: u16 = a.into();
				if b == 6027 {
					// panic!("got");
					return Ok(true);
				}
			}
		}

		// } else {
			// eprintln!("OPERATION: {:?}", &op);
		// }
        // eprintln!("Registers Before Op: {:?}", self.registers);
		// assert_eq!(self.registers[7], 0);
		// self.registers[7] = 1;

        match &op {
            Halt => {return Ok(true);},
            Set(ra, v) => {
                self.registers[ra.to_usize()] = self.resolve(*v)?.into();
            },
            Out(x) => {
                let r  = self.resolve(*x)?;
                self.screen.send_char(r as u8 as char)?;
                // print!("{}", r as u8 as char);
                // std::io::stdout().flush()?;
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
				if let Some(x) = self.register_8_preset {
					self.registers[7] = x;
				}
                let v = self.screen.get_char()?;
                self.set_mem(*x, v as u16)?;
            }

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

    // pub fn read_one_stdin(&mut self) -> anyhow::Result<char> {
    //     if self.line_buffer.len() == 0 {
    //         let mut read_buf = String::new();
    //         std::io::stdin().read_line(&mut read_buf)?;
    //         self.line_buffer = read_buf.chars().rev().collect();
    //     }
    //     Ok(self.line_buffer.pop().unwrap())
    // }
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


// fn main() -> anyhow::Result<()> {
//     let bytes = include_bytes!("../challenge.bin");
//     let (mut env_screen, screen) = Screen::create();

//     let screen_recv = screen.text_recv;
//     let screen_send = screen.text_send;
//     let t1 = std::thread::spawn( move || {
//         loop {
//             let r = screen_recv.recv()?;
//             print!("{}", r);
//         }
//         Ok(()) as anyhow::Result<()>
//     });

//     let t1 = std::thread::spawn( move || {
//         loop {
//             let mut read_buf = String::new();
//             std::io::stdin().read_line(&mut read_buf)?;
//             screen_send.send(read_buf)?;
//         }
//         Ok(()) as anyhow::Result<()>
        
//     });
//     loop {
//         let mut env = ExecutionEnv::new(bytes, env_screen);
//         env.run()?;
//         env_screen = env.screen;
//         std::thread::sleep(std::time::Duration::from_millis(100));
//         println!("\n=================>");
//         println!("=================> You died. Restarting the game");
//         std::thread::sleep(std::time::Duration::from_secs(2));
//     }
//     // env.run()?;
//     // println!("Hello, world!");
//     Ok(())
// }

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
#[test]
fn register_finder() {
	let values = [
		"take tablet",
		"doorway",
		"north",
		"north",
		"bridge",
		"continue",
		"down",
		"east",
		"take empty lantern",
		"west",
		"west",
		"passage",
		"ladder",
		"west",
		"south",
		"north",
		"take can",
		"west",
		"use can",
		"use lantern",
		"ladder",
		"darkness",
		"continue",
		"west",
		"west",
		"west",
		"west",
		"north",
		"take red coin",
		"north",
		"east",
		"take concave coin",
		"down",
		"take corroded coin",
		"up",
		"west",
		"west",
		"take blue coin",
		"up",
		"take shiny coin",
		"down",
		"east",
		"use blue coin",
		"use red coin",
		"use shiny coin",
		"use concave coin",
		"use corroded coin",
		"north",
		"take teleporter",
		"use teleporter",
	];
    let bytes = include_bytes!("../challenge.bin");
	let (mut s1, mut s2) = Screen::create();
	for register_val in 1..32768 {
	// for register_val in 1..3 {
		s1.reset().unwrap();
		let mut ev = ExecutionEnv::new(bytes, s1, Some(register_val));
		for k in &values {
			let mut s = k.to_string();
			s.push('\n');
			s2.send(s).unwrap();
		}
		// s2.send(val)
		dbg!(register_val);
		match ev.check_teleporter() {
			Ok(x) => { assert!(!x); },
			Err(x) => unreachable!()
		};
		s1 = ev.screen;
	}
}

#[test]
fn register_finder2() {
	let values = [
		"take tablet",
		"doorway",
		"north",
		"north",
		"bridge",
		"continue",
		"down",
		"east",
		"take empty lantern",
		"west",
		"west",
		"passage",
		"ladder",
		"west",
		"south",
		"north",
		"take can",
		"west",
		"use can",
		"use lantern",
		"ladder",
		"darkness",
		"continue",
		"west",
		"west",
		"west",
		"west",
		"north",
		"take red coin",
		"north",
		"east",
		"take concave coin",
		"down",
		"take corroded coin",
		"up",
		"west",
		"west",
		"take blue coin",
		"up",
		"take shiny coin",
		"down",
		"east",
		"use blue coin",
		"use red coin",
		"use shiny coin",
		"use concave coin",
		"use corroded coin",
		"north",
		"take teleporter",
		"use teleporter",
	];
    let bytes = include_bytes!("../challenge.bin");
	let (mut s1, mut s2) = Screen::create();
	for register_val in 16000..32768 {
	// for register_val in 1..3 {
		s1.reset().unwrap();
		let mut ev = ExecutionEnv::new(bytes, s1, Some(register_val));
		for k in &values {
			let mut s = k.to_string();
			s.push('\n');
			s2.send(s).unwrap();
		}
		// s2.send(val)
		dbg!(register_val);
		match ev.check_teleporter() {
			Ok(x) => { assert!(!x); },
			Err(x) => unreachable!()
		};
		s1 = ev.screen;
	}
}
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
