use std::{sync::{Mutex, Arc}, process::exit};

use vm::Screen;

mod vm;

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

fn main() -> anyhow::Result<()> {
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
            hist2.lock().unwrap().push(read_buf.clone());
            screen_send.send(read_buf)?;
        }
        Ok(()) as anyhow::Result<()>
    });

    loop {
        history.lock().unwrap().clear();
        let mut env = vm::ExecutionEnv::new(bytes, env_screen, None);
        env.run()?;
        env_screen = env.screen;
        std::thread::sleep(std::time::Duration::from_millis(100));
        println!("\n=================>");
        println!("=================> You died. Restarting the game");
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    println!("{:?}", history);
    // history.lock().unwrap().clear();
    // env.run()?;
    // println!("Hello, world!");
    Ok(())
}