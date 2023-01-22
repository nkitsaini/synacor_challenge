use std::{sync::{Mutex, Arc}, process::exit, io::{Write, Read}, path::PathBuf};

use anyhow::bail;
use vm::{Screen, EnvSnapshot, StaticExecuter};
use clap::Parser;

// mod async_vm;
// mod vm_runner;
mod vm;
mod op_parser;
mod reverse_engineer;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
   /// Name of the person to greet
   #[arg(short, long)]
   checkpoint: Option<PathBuf>,
}

impl Args {
    fn get_replay(&self) -> anyhow::Result<Vec<String>> {
        let mut replay_codes = vec![];

        if let Some(cp) = &self.checkpoint {
            let mut buf = vec![];
            std::fs::File::open(cp).unwrap().read_to_end(&mut buf)?;
            let x: Vec<String> = serde_json::from_slice(&buf)?;
            replay_codes = x;
        }
        Ok(replay_codes)
    }
}

enum CustomCommand {
    Save(PathBuf)
}

impl CustomCommand {
    fn parse(mut cmd: &str) -> anyhow::Result<Option<Self>> {
        if cmd.starts_with("save") {
            cmd = cmd.trim();
            match cmd.strip_prefix("save ") {
                None => bail!(">> Usage: save <file_path>"),
                Some(x) => {
                    return Ok(Some(Self::Save(x.into())));
                }
            }
        }
        return Ok(None)
    }

    fn execute(&self, executor: &StaticExecuter) -> anyhow::Result<String> {
        match self {
            Self::Save(x) => {
                let replays = serde_json::to_string_pretty(&executor.get_history()).unwrap();
                match std::fs::File::create(x) {
                    Ok(mut f) => {
                        if let Err(x) = f.write_all(replays.as_bytes()) {
                            bail!(x);
                        } else {
                            return Ok(format!(">> Successfully Written To: {:?}", x));
                        };
                    },
                    Err(x) => {
                        bail!(x);
                    }
                }
            }
        };
    }
}

enum Command {
    Custom(CustomCommand),
    Game(String)
}

impl Command {
    fn parse(val: String) -> anyhow::Result<Self> {
        Ok(match CustomCommand::parse(&val)? {
            Some(x) => Self::Custom(x),
            None => Self::Game(val)
        })
    }
}

fn main() -> anyhow::Result<()> {
    let replay_codes = Args::parse().get_replay()?;

    loop {
        let mut executer = StaticExecuter::new_from_checkpoint(replay_codes.clone())?;
        print!("{}", executer.bootstrap()?);

        loop {
            let mut cmd = "".into();
            std::io::stdin().read_line(&mut cmd)?;
            let cmd = match Command::parse(cmd) {
                Err(x) => {
                    println!(">> ERROR: {x}");
                    continue;
                },
                Ok(cmd) => cmd
            };
            match cmd {
                Command::Custom(cmd) => {
                    match cmd.execute(&executer) {
                        Err(x) => println!(">> ERROR: {x}"),
                        Ok(x)  => println!(">> {x}")
                    };
                },
                Command::Game(cmd) => {
                    match executer.execute(cmd)? {
                        None => break,
                        Some(x) => print!("{x}")
                    }
                }
            }
        }
        println!("=========== Restarting")
    }
}
