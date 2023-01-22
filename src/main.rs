use std::{sync::{Mutex, Arc}, process::exit, io::{Write, Read}, path::PathBuf};

use anyhow::bail;
use vm::StaticExecuter;
use clap::Parser;

// mod async_vm;
// mod vm_runner;
mod vm;
mod op_parser;
mod reverse_engineer;


#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
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

#[derive(Debug, Default)]
struct GameState {
    inventory: Vec<String>,
    orb_weight: usize,
    last_symbol: Option<char>
}

impl GameState {
    fn update_from_executor(&mut self, executer: &mut StaticExecuter) -> anyhow::Result<()> {
        if executer.is_finished() {
            return Ok(())
        }
        let output = executer.execute("inv\n".into())?.unwrap();
        self.update_from_output(&output);
        Ok(())
    }

    fn update(&mut self, output: &str, executor: &mut StaticExecuter) -> anyhow::Result<()> {
        self.update_from_output(output);
        self.update_from_executor(executor)
    }

    fn update_from_output(&mut self, output: &str) {
        if output.find("Your inventory:").is_some() {
            self.inventory.clear();
            for mut line in output.lines() {
                line = line.trim_end();
                match line.strip_prefix("- ") {
                    None => continue,
                    Some(x) => {
                        self.inventory.push(x.to_string());
                    }
                }
            }
        }
        let contains_orb = self.inventory.contains(&("orb".into()));

        // When the orb evaporates (or we drop it)
        if !contains_orb {
            self.orb_weight = 0;
            self.last_symbol.take();
        }


        // When we take the orb (and previously it was zero i.e. not present)
        if contains_orb && self.orb_weight == 0 {
            self.orb_weight = 22;
        }
        if !contains_orb {
            return
        }

        // Orb weight (Sign) 
        if let Some(x) = output.find("mosaic depicting a") {
            assert!(self.last_symbol.is_none(), "Got two symboles one by one");
            if output[x..x+25].contains('*') {
                self.last_symbol = Some('*');
            } else if output[x..x+25].contains('+') {
                self.last_symbol = Some('+');
            } else if output[x..x+25].contains('-') {
                self.last_symbol = Some('-');
            } else {
                unreachable!()
            }
        }

        // Orb weight (Number) 
        if let Some(x) = output.find("mosaic depicting the number '") {
            assert!(self.last_symbol.is_some(), "Got number without symbol");
            let y = dbg!(&output[x+29..]).find('\'').unwrap();
            let num = dbg!(&output[x+29..x+29+y]);
            let num: usize = num.parse().unwrap();
            match self.last_symbol.unwrap() {
                '*' => {
                    self.orb_weight *= num;
                },
                '+' => {
                    self.orb_weight += num;
                },
                '-' => {
                    self.orb_weight -= num;
                },
                x => panic!("Unreachable State with previous sign: {:?}", x)
            }
            self.last_symbol.take();

        }

    }

    fn print(&self) {
        println!("---------------==================---------------- ");
        println!("-- Inventory: {:?}", self.inventory);
        println!("-- Orb Weight: {:?}", self.orb_weight);
        if let Some(x) = self.last_symbol {
            println!("-- Sign: {:?}", x);
        }
        println!("---------------==================---------------- ");
    }
}
fn main() -> anyhow::Result<()> {
    let replay_codes = Args::parse().get_replay()?;

    loop {
        let mut game_state = GameState::default();

        // let mut executer = StaticExecuter::new_from_checkpoint(replay_codes.clone())?;
        let mut executer = StaticExecuter::new();
        let output = executer.bootstrap()?;
        game_state.update(&output, &mut executer)?;
        print!("{}", output);
        for code in replay_codes.iter() {
            let output = executer.execute(code.to_string())?.unwrap();
            game_state.update(&output, &mut executer)?;
            print!("{}", output);
        }
        game_state.print();


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
                        Some(output) => {
                            game_state.update(&output, &mut executer)?;
                            print!("OUT2: {output}");
                            game_state.print();
                        }
                    }
                }
            }
        }
        println!("=========== Restarting")
    }
}
