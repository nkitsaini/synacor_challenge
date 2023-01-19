use crate::op_parser::*;

pub fn parse(content: &[u8]) -> anyhow::Result<String> {
	let mut rv = String::new();
	let mut i = 0;
	let mut code_mem = [0u16; 38000];
	code_mem.copy_first(&Op::convert_bytes(content));

	while i < content.len() {
		let op = Op::parse(&code_mem[i..i+4])?;
		dbg!(op);
		i += op.param_bytes() as usize;
	}
	Ok(rv)
}