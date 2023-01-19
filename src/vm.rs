use core::num;
use std::collections::{VecDeque, HashSet, BTreeSet};
use std::sync::mpsc::TryRecvError;
use std::{ops::Mul, sync::mpsc::Receiver};
use std::fs::File;
use std::io::prelude::*;
use serde::{Serialize, Deserialize};
use ux::{u15, u3};
use anyhow::{bail, Context};
use crate::op_parser::*;

const DEBUG_PRINT: bool = true;

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

#[derive(Default, Serialize, Deserialize, Clone)]
pub struct EnvSnapshot {
    pub(crate) stack: Vec<MemBlock>,
    // pub(crate) memory: [MemBlock; 32768],
    pub(crate) memory: Vec<MemBlock>,
    pub(crate) registers: [MemBlock; 8],
    pub(crate) curr_point: u16,
	pub(crate) register_8_preset: Option<u16>,
    pub(crate) operation_count: u64
}

impl EnvSnapshot {
	pub fn new(env: &ExecutionEnv) -> Self {
		Self {
			stack: env.stack.clone(),
			memory: Vec::from_iter(env.memory.iter().cloned()),
			registers: env.registers.clone(),
			curr_point: env.curr_point.into(),
			register_8_preset: env.register_8_preset,
            operation_count: env.operation_count
		}
	}
	pub fn to_json(&self) -> String {
		serde_json::to_string(self).unwrap()
	}
	pub fn from_json(json: &str) -> anyhow::Result<Self> {
		Ok(serde_json::from_str(json)?)
	}
	pub fn to_env(&self, screen: Screen) -> anyhow::Result<ExecutionEnv> {
		Ok(ExecutionEnv {
			stack: self.stack.clone(),
			memory: self.memory.clone().try_into().ok().context("memory length not correct")?,
			registers: self.registers.clone(),
			curr_point: self.curr_point.try_into()?,
			screen: screen,
			register_8_preset: self.register_8_preset,
            operation_count: self.operation_count
		})
	}
}

pub struct ExecutionEnv {
    pub(crate) stack: Vec<MemBlock>, // 
    pub(crate) memory: [MemBlock; 32768], // [code] 32768
    pub(crate) registers: [MemBlock; 8],  //    (1)
    pub(crate) curr_point: Mem,
    pub(crate) screen: Screen,
	pub(crate) register_8_preset: Option<u16>,
    pub(crate) operation_count: u64
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
    pub fn get_all(&mut self) -> anyhow::Result<String> {
        let mut rv = self.buffer.clone();
        loop {
            match self.text_recv.try_recv() {
                Err(TryRecvError::Disconnected) => {
                    bail!("Disconnected screen get");
                },
                Err(TryRecvError::Empty) => {
                    self.buffer = "".to_string();
                    return Ok(rv)
                }
                Ok(x) => {
                    rv += &x;
                }
            };
        }
        // while self.buffer.is_empty() {
        //     let data = self.text_recv.recv()?;
        //     self.buffer = data.chars().rev().collect();
        // }
        // Ok(self.buffer.pop().unwrap())
    }
    // pub fn drain(count: usize) -> anyhow::Result<()> {
    //     self.try_get_char()

    // }
    pub fn try_get_char(&mut self) -> anyhow::Result<Option<char>> {
        while self.buffer.is_empty() {
            match self.text_recv.try_recv() {
				Err(TryRecvError::Disconnected) => {
					bail!("Disconnected screen get");
				},
				Err(TryRecvError::Empty) => {
					return Ok(None)
				}
				Ok(x) => {
					self.buffer = x.chars().rev().collect();
				}
			};
        }
        Ok(Some(self.buffer.pop().unwrap()))
    }

	pub fn is_empty(&mut self) -> anyhow::Result<bool> {
		let c = self.try_get_char()?;
		match c {
			Some(x) => {
				self.buffer.push(x);
				return Ok(false)
			},
			None => {
				return Ok(true)
			}
		}
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
			register_8_preset: register_preset,
            operation_count: 0
        };

        rv.memory.copy_first(&Op::convert_bytes(content));
        rv
    }

	pub fn snapshot(&self) -> EnvSnapshot {
		EnvSnapshot::new(self)
	}

    pub fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let mut values = [0u16; 4];
            for i in (self.curr_point.to_usize())
                ..(self.memory.len().min(self.curr_point.to_usize() + 4))
            {
                values[i - self.curr_point.to_usize()] = self.memory[i];
            }
            let op = Op::parse(&self.memory[self.curr_point.to_usize()..self.curr_point.to_usize()+4])?;
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
            let op = Op::parse(&self.memory[self.curr_point.to_usize()..self.curr_point.to_usize()+4])?;
			if let Op::Call(x) = &op {
				if let Val::Num(a) = *x {
					let b: u16 = a.into();
					if b == 6027 {
						return Ok(false);
					}
				}
			}
            if self.run_op(op)? {
				break
            };
        }
        Ok(false)
    }

    pub fn run_until_condition(&mut self, condition: fn(&ExecutionEnv) -> bool) -> anyhow::Result<()> {
        loop {
            let op = Op::parse(&self.memory[self.curr_point.to_usize()..self.curr_point.to_usize()+4])?;
            if self.run_op(op)? {
                break
            };
            if condition(self) {
                break
            }
        }
        Ok(())

    }

    pub fn run_until_empty(&mut self) -> anyhow::Result<bool> {
        loop {
            let mut values = [0u16; 4];
            let op = Op::parse(&self.memory[self.curr_point.to_usize()..self.curr_point.to_usize()+4])?;
			if let Op::In(x) = &op {
				if self.screen.is_empty()? {
					return Ok(false);
				}
			}
            if self.run_op(op)? {
				if let Op::Call(x) = op {
					return Ok(false)

				}
            };
        }
        Ok(false)
    }

    fn run_op(&mut self, mut op: Op) -> anyhow::Result<bool> {
        use Op::*;
        let mut jump_pos: Option<Mem> = None;

        self.operation_count += 1;
        
        // eprintln!("OPERATION: {:?}", &op);
        // eprintln!("MemoryAddr: {:?}", &self.curr_point);
        // eprintln!("Registers: {:?}", &self.registers);
        // eprintln!("Count: {:?}", &self.operation_count);
        // Bypass teleporter confirmation
        if self.curr_point == 6027u16.try_into().unwrap() {
            op = Op::Ret;
        }
        if self.operation_count == 701400 {
            if let Some(x) = self.register_8_preset {
                self.registers[7] = x;
            }
        }

        // if op == Mem

        match &op {
            Halt => {return Ok(true);},
            Set(ra, v) => {
                self.registers[ra.to_usize()] = self.resolve(*v)?.into();
            },
            Out(x) => {
                let r  = self.resolve(*x)?;
                self.screen.send_char(r as u8 as char)?;
                // eprint!("{}", r as u8 as char);
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
                if self.operation_count == 865476 {
                    // self.registers[0] = 1;
                    // println!("------> did jt");
                    // println!("------> did jt {:?}, {}", v, m);
                    // // dbg!(v, m);
                    // jump_pos = Some(*m);
                }
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
            },
            Rmem(addr, a) => {
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
                self.set_mem(*addr,  m)?;
            },
            Wmem(addr, a) => {
                // let mut faddr = *addr;
                
                let b: Val = (*addr).into();
                let x6 = self.resolve(b)?;
                let x: u15 = x6.try_into()?;
                
                // assert!((x6 as usize) < include_bytes!("../challenge.bin").len()/2);

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
				// if let Some(x) = self.register_8_preset {
				// 	self.registers[7] = x;
				// }
                let v = self.screen.get_char()?;
                self.set_mem(*x, v as u16)?;
            }

        };
        match jump_pos {
            None => {
                let bts: u15 = op.param_bytes().into();
                self.curr_point = self.curr_point + bts;
            },
            Some(x) => {
                self.curr_point = x;
            }
        }

        return Ok(false);
    }
}

fn bit_not(x: Num) -> Num {
    !x & Num::MAX
}

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
	// for register_val in 32768..u16::MAX {
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
			Ok(x) => {
                assert!(s2.get_all().unwrap().contains("Unusual"));
                assert!(!x);
            },
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
	];

    let bytes = include_bytes!("../challenge.bin");
	let (mut s1, mut s2) = Screen::create();
	// for register_val in 16000..32768 {
	s1.reset().unwrap();
	let mut ev = ExecutionEnv::new(bytes, s1, None);
	for k in &values {
		let mut s = k.to_string();
		s.push('\n');
		s2.send(s).unwrap();
	}
	// s2.send(val)
	// dbg!(register_val);
	match ev.run_until_empty() {
		Ok(x) => { assert!(!x); },
		Err(x) => unreachable!()
	};
	let mut f = std::fs::File::create("/tmp/snapshot.json").unwrap();
	let x = ev.snapshot().to_json();
	f.write_all(x.as_bytes()).unwrap();
	// s1 = ev.screen;
	// }
}

#[test]
fn register_finder3() {
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
	// for register_val in 16000..32768 {
	for register_val in 1..32768 {
	// for register_val in 0..16000 {
        s1.reset().unwrap();
        let mut ev = ExecutionEnv::new(bytes, s1, Some(register_val));
        for k in &values {
            let mut s = k.to_string();
            s.push('\n');
            s2.send(s).unwrap();
        }
        // s2.send(val);
        dbg!(register_val);
        match ev.run_until_condition(|s| s.curr_point == 6027u16.try_into().unwrap()) {
            Ok(x) => { },
            Err(x) => unreachable!()
        };
        // let mut f = std::fs::File::create("/tmp/snapshot.json").unwrap();
        // let x = ev.snapshot().to_json();
        // f.write_all(x.as_bytes()).unwrap();
        s1 = ev.screen;
	}
}

#[test]
fn register_finder4() {
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
	// for register_val in 16000..32768 {
	for register_val in 16000..32768 {
	// for register_val in 0..16000 {
        s1.reset().unwrap();
        let mut ev = ExecutionEnv::new(bytes, s1, Some(register_val));
        for k in &values {
            let mut s = k.to_string();
            s.push('\n');
            s2.send(s).unwrap();
        }
        // s2.send(val);
        dbg!(register_val);
        match ev.run_until_condition(|s| s.curr_point == 6027u16.try_into().unwrap()) {
            Ok(x) => { },
            Err(x) => unreachable!()
        };
        // let mut f = std::fs::File::create("/tmp/snapshot.json").unwrap();
        // let x = ev.snapshot().to_json();
        // f.write_all(x.as_bytes()).unwrap();
        s1 = ev.screen;
	}
}

fn run_test_fr(preset: u16) -> ExecutionEnv {
    // dbg!(preset);
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
    let mut ev = ExecutionEnv::new(bytes, s1, Some(preset));

    for k in &values {
        let mut s = k.to_string();
        s.push('\n');
        s2.send(s).unwrap();
    }

    // std::thread::sp
    // let end_point = 873145;
    // match ev.run_until_condition(|s| s.operation_count == 873145) {
    //     Ok(x) => { },
    //     Err(x) => unreachable!()
    // };
    let mut should_shout = false;
    match ev.run_until_empty() {
        Ok(x) => { },
        Err(x) => {
            should_shout = true;
        }
    };

    // Drain unnecessary chars
    for i in 0..9508 {
        s2.try_get_char().unwrap();
    }
    let mut r = String::new();
    while let Some(x) = s2.try_get_char().unwrap() {
        r.push(x);
    }

    if r.find("Miscalibration detected!  Aborting teleportation!").is_none()  {
            should_shout = true;
    }

    if should_shout {
        loop {
            println!("=========================================== Found it: {}", preset);
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    } else {
        if preset %100 == 0 {
            // ev.operation_count += 1;
            println!("Nope, no luck: {}", preset);
        }
    }

    ev
}

#[test]
fn register_finder5() {
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

    let mut value = None;
    let bytes = include_bytes!("../challenge.bin");
	let (mut s1, mut s2) = Screen::create();
	// for register_val in 16000..32768 {
	for register_val in 1..32768 {
	// for register_val in 0..16000 {
        s1.reset().unwrap();
        let mut ev = ExecutionEnv::new(bytes, s1, Some(register_val));
        for k in &values {
            let mut s = k.to_string();
            s.push('\n');
            s2.send(s).unwrap();
        }
        // s2.send(val);
        dbg!(register_val);
        match ev.run_until_condition(|s| s.curr_point == 6027u16.try_into().unwrap()) {
            Ok(x) => { },
            Err(x) => unreachable!()
        };
        match value {
            None => {
                value = Some(ev.operation_count);
            },
            Some(x) => {
                assert_eq!(x, ev.operation_count);
            }
        }
        // let mut f = std::fs::File::create("/tmp/snapshot.json").unwrap();
        // let x = ev.snapshot().to_json();
        // f.write_all(x.as_bytes()).unwrap();
        s1 = ev.screen;
	}
}

// fn get_snapshot_for()
#[test]
fn register_finder6() {
	let values = [ "take tablet", "doorway", "north", "north", "bridge", "continue", "down", "east", "take empty lantern", "west", "west", "passage", "ladder", "west",
		"south", "north", "take can", "west", "use can", "use lantern", "ladder", "darkness", "continue",
		"west", "west", "west", "west", "north", "take red coin", "north", "east", "take concave coin", "down",
		"take corroded coin", "up", "west", "west", "take blue coin", "up", "take shiny coin", "down",
		"east", "use blue coin", "use red coin", "use shiny coin", "use concave coin", "use corroded coin",
		"north", "take teleporter", "use teleporter"];

    let mut a = threadpool::ThreadPool::new(12);
    let u15_max: u16 = u15::MAX.into();
    // let value = run_test_fr(1).operation_count;
	let (mut s1, mut s2) = Screen::create();
    let bytes = include_bytes!("../challenge.bin");
    let mut ev = ExecutionEnv::new(bytes, s1, Some(1));
    for x in &values[..values.len()-1] {
        s2.send(format!("{x}\n")).unwrap();
    }
    ev.run_until_empty().unwrap();
    let snap = ev.snapshot();
    let snap_ref: &'static EnvSnapshot = Box::leak(Box::new(snap));




    let (mut pr_tx, pr_rx) = std::sync::mpsc::channel();
    
    let range = 1..=u16::MAX;
    let range = (1..=u16::MAX).rev();
    let mut all_to_wait = BTreeSet::from_iter(range.clone());
    for i in range {
        // let t = tx.clone();
        let pr_tx = pr_tx.clone();
        a.execute(move || {
            let (s1, mut s2) = Screen::create();
            let mut env = snap_ref.to_env(s1).unwrap();
            env.registers[7] = i;
            s2.send("use teleporter\n".to_string()).unwrap();
            env.run_until_empty().unwrap();
            // for i in 0..9508 {
            //     s2.try_get_char().unwrap();
            // }
            let mut r = String::new();
            while let Some(x) = s2.try_get_char().unwrap() {
                r.push(x);
            }
            // dbg!(r.len());
            let mut should_shout = r.len() != 332;
            // dbg!(r);
            if r.find("Miscalibration detected!  Aborting teleportation!").is_none()  {
                    should_shout = true;
            }
            if should_shout {
                loop {
                    println!("=========================================== Found it: {}", i);
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            } else {
                if i %100 == 0 {
                    // ev.operation_count += 1;
                    println!("Nope, no luck: {}", i);
                }
            }

            // if run_test_fr(i).operation_count != value {
            //     loop {
            //         std::thread::sleep(std::time::Duration::from_millis(10));
            //         println!("============= Wrong value: {}", i);
            //     }
            //     t.send(i).unwrap();
            // }
            // pr_tx.send(i).unwrap();
        });
    }
    loop {
        let v = pr_rx.recv().unwrap();
        assert!(all_to_wait.remove(&v));
        // dbg!(all_to_wait.first(), all_to_wait.len());
        // let v = dbg!(rx.recv().unwrap());
        // assert!(false, "val: {}", v);
    }
    a.join();
    println!("--------------------- Waiting");
    // a.iter();
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
