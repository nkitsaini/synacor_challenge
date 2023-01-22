use core::num;
use std::collections::{VecDeque, HashSet, BTreeSet};
use std::sync::mpsc::TryRecvError;
use std::ops::Mul;
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
    pub(crate) text_recv: tokio::sync::mpsc::UnboundedReceiver<String>,
    pub(crate) text_send: tokio::sync::mpsc::UnboundedSender<String>,
    pub(crate) buffer: String
}

impl Screen {
    pub fn create() -> (Screen, Screen) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (tx2, rx2) = tokio::sync::mpsc::unbounded_channel();
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
    pub async fn get_char(&mut self) -> anyhow::Result<char> {
        while self.buffer.is_empty() {
            let data = self.text_recv.recv().await.context("MPSC died")?;
            self.buffer = data.chars().rev().collect();
        }
        Ok(self.buffer.pop().unwrap())
    }
    pub fn get_all(&mut self) -> anyhow::Result<String> {
        let mut rv = self.buffer.clone();
        loop {
            match self.text_recv.try_recv() {
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    bail!("Disconnected screen get");
                },
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
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
    pub fn try_get_char(&mut self) -> anyhow::Result<Option<char>> {
        while self.buffer.is_empty() {
            match self.text_recv.try_recv() {
				Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
					bail!("Disconnected screen get");
				},
				Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
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

    pub async fn run(&mut self) -> anyhow::Result<()> {
        loop {
            let mut values = [0u16; 4];
            for i in (self.curr_point.to_usize())
                ..(self.memory.len().min(self.curr_point.to_usize() + 4))
            {
                values[i - self.curr_point.to_usize()] = self.memory[i];
            }
            let op = Op::parse(&self.memory[self.curr_point.to_usize()..self.curr_point.to_usize()+4])?;
            if self.run_op(op).await? {
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

    async fn run_op(&mut self, mut op: Op) -> anyhow::Result<bool> {
        use Op::*;
        let mut jump_pos: Option<Mem> = None;

        self.operation_count += 1;

        if self.curr_point == 6027u16.try_into().unwrap() {
            self.registers[0] = 6;
            self.registers[1] = 4;
            op = Op::Ret;
        }
        if self.operation_count == 701400 {
            if let Some(x) = self.register_8_preset {
                self.registers[7] = x;
            }
        }

        match &op {
            Halt => {return Ok(true);},
            Set(ra, v) => {
                self.registers[ra.to_usize()] = self.resolve(*v)?.into();
            },
            Out(x) => {
                let r  = self.resolve(*x)?;
                self.screen.send_char(r as u8 as char)?;
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
                let v = self.screen.get_char().await?;
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
