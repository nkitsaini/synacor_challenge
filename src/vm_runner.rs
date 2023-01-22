use std::collections::VecDeque;

use crate::{async_vm::Screen};
use anyhow::Context;
use tokio::{io::{self, AsyncBufReadExt}, select};
use either::*;


struct Runner {
	screen: Screen,
	buffer_history: String,
	stats: Stats
}

impl Runner {
	pub fn new(screen: Screen) -> Self {
		Self {screen, buffer_history: "".into()}
	}
	pub fn new_with_preset(mut screen: Screen, preset: Vec<String>) -> anyhow::Result<Self> {
		for p in preset {
			screen.send(p)?;
		}

		Ok(Self::new(screen))
	}

	pub fn push_char_history(&mut self, c: char) {
		self.buffer_history.push(c);
		if self.buffer_history.len() > 40 {
			self.buffer_history.remove(0);
		}
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		loop {
			let stdin = io::stdin();
			let reader = tokio::io::BufReader::new(stdin);
			let mut lines = reader.lines();

			// Moving out from select!() for IDE support
			let mut act = None;
			select! {
				line = lines.next_line() => {
					act = Some(Left(line?.context("Stdin closed")?));
				},
				out = self.screen.get_char() => {
					act = Some(Right(out?));
				}
			}

			match act.unwrap() {
				Left(x) => {
					// Check if custom command
				},
				Right(x) => {
					self.push_char_history(x);
					
				}
			}

		}
	}
}