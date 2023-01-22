use std::{sync::{Mutex, Arc}, process::exit, io::{Write, Read}, path::PathBuf};

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


trait Player {
    fn play(&mut self, screen: Screen) -> anyhow::Result<()>;
}
struct ReplayPlayer {
    replay: Vec<String>
}

impl Player for ReplayPlayer {
    fn play(&mut self, mut screen: Screen) -> anyhow::Result<()> {
        for x in &self.replay {
            let x = x.clone();
            screen.send(x)?;
        }

        Ok(())
    }
}
fn run(env_snap: EnvSnapshot) -> anyhow::Result<()> {
    let bytes = include_bytes!("../challenge.bin");
    let (mut env_screen, screen) = vm::Screen::create();

    let screen_recv = screen.text_recv;
    let screen_send = screen.text_send;
    let mut history = Arc::new(Mutex::new(Vec::<String>::new()));
    let hist2 = history.clone();
    let hist3 = history.clone();
    ctrlc::set_handler(move || {
        println!("\n=========== Summary");
        for entry in hist3.lock().unwrap().iter() {
            println!("{}", entry.trim());
        }
        println!("\n=========== End");
        exit(1);
    })?;
    let t1 = std::thread::spawn( move || {
        loop {
            let r = screen_recv.recv()?;
            print!("{}", r);
        }
        Ok(()) as anyhow::Result<()>
    });

    let t1 = std::thread::spawn(move || {
        loop {
            let mut read_buf = String::new();
            std::io::stdin().read_line(&mut read_buf)?;
            if read_buf.starts_with("save") {
                match read_buf.strip_prefix("save ") {
                    None => {println!(">> Usage: save <file_path>");},
                    Some(x) => {
                        let replays = serde_json::to_string_pretty(&hist2.lock().unwrap().clone()).unwrap();
                        match std::fs::File::create(x) {
                            Ok(mut f) => {
                                if let Err(x) = f.write_all(replays.as_bytes()) {
                                    println!(">> Error: {:?}", x);
                                } else {
                                    println!(">> Successfully Written To: {:?}", x);
                                };
                            },
                            Err(x) => {
                                println!(">> Error: {:?}", x);
                            }
                        }
                    }
                }
            } else {
                hist2.lock().unwrap().push(read_buf.clone());
                screen_send.send(read_buf)?;
            }
        }
        Ok(()) as anyhow::Result<()>
    });

    loop {
        history.lock().unwrap().clear();
        let mut env = env_snap.to_env(env_screen)?;
        env.run()?;
        env_screen = env.screen;
        std::thread::sleep(std::time::Duration::from_millis(100));
        println!("\n=================>");
        println!("=================> You died. Restarting the game");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    Ok(())
}

fn main() -> anyhow::Result<()> {
    loop {
        let mut executer = StaticExecuter::new();
        print!("{}", executer.bootstrap()?);

        loop {
            let mut cmd = "".into();
            std::io::stdin().read_line(&mut cmd)?;
            match executer.execute(cmd)? {
                None => break,
                Some(x) => print!("{x}")
            }
        }
        println!("=========== Restarting")
    }
    Ok(())
}
fn main1() -> anyhow::Result<()> {
    let args = Args::parse();

    let bytes = include_bytes!("../challenge.bin");
    // reverse_engineer::parse(bytes)?;
    // return Ok(());
    let (mut env_screen, screen) = vm::Screen::create();
    let mut replay_codes = vec![];
    if let Some(cp) = args.checkpoint {
        let mut buf = vec![];
        std::fs::File::open(cp).unwrap().read_to_end(&mut buf).unwrap();
        let x: Vec<String> = serde_json::from_slice(&buf).unwrap();
        replay_codes = x;
        for code in replay_codes.iter() {
            screen.text_send.send(code.clone()).unwrap();
        }
    }

    let screen_recv = screen.text_recv;
    let screen_send = screen.text_send;
    let mut history = Arc::new(Mutex::new(Vec::<String>::new()));
    let hist2 = history.clone();
    let hist3 = history.clone();

    let mut rp_cp = replay_codes.clone();
    history.lock().unwrap().append(&mut rp_cp);
    ctrlc::set_handler(move || {
        println!("\n=========== Summary");
        for entry in hist3.lock().unwrap().iter() {
            println!("{}", entry.trim());
        }
        println!("\n=========== End");
        exit(1);
    })?;
    let t1 = std::thread::spawn( move || {
        loop {
            let r = screen_recv.recv()?;
            print!("{}", r);
        }
        Ok(()) as anyhow::Result<()>
    });

    let t1 = std::thread::spawn(move || {
        loop {
            let mut read_buf = String::new();
            std::io::stdin().read_line(&mut read_buf)?;
            if read_buf.starts_with("save") {
                read_buf = read_buf.trim().to_string();
                match read_buf.strip_prefix("save ") {
                    None => {println!(">> Usage: save <file_path>");},
                    Some(x) => {
                        let replays = serde_json::to_string_pretty(&hist2.lock().unwrap().clone()).unwrap();
                        match std::fs::File::create(x) {
                            Ok(mut f) => {
                                if let Err(x) = f.write_all(replays.as_bytes()) {
                                    println!(">> Error: {:?}", x);
                                } else {
                                    println!(">> Successfully Written To: {:?}", x);
                                };
                            },
                            Err(x) => {
                                println!(">> Error: {:?}", x);
                            }
                        }
                    }
                }
            } else {
                hist2.lock().unwrap().push(read_buf.clone());
                screen_send.send(read_buf)?;
            }
        }
        Ok(()) as anyhow::Result<()>
    });

    loop {
        let mut env = vm::ExecutionEnv::new(bytes, env_screen, Some(25734));
        env.run()?;
        env_screen = env.screen;
        std::thread::sleep(std::time::Duration::from_millis(100));
        println!("\n=================>");
        println!("=================> You died. Reseting the game");
        history.lock().unwrap().clear();
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    println!("{:?}", history);
    // history.lock().unwrap().clear();
    // env.run()?;
    // println!("Hello, world!");
    Ok(())
}