use std::collections::HashMap;


#[inline]
fn run_cached(registers: &mut [u16; 3], cache: &mut HashMap<[u16; 3], [u16; 3]>) {
    if let Some(x) = cache.get(registers) {
        // dbg!("Cached");
        *registers = *x;
        return
    }
    let mut regs = registers.clone();
    run_cached_helper(&mut regs, cache);
    cache.insert(registers.clone(), regs.clone());
    *registers = regs;
}
// [4, 5445, val]; Goes into both if
// [4, 5444, val];
// [4, 5443, val];
//      .
//      .
// [4, 0, val];
// [4, 0, val];


#[inline]
fn run_cached_helper(registers: &mut [u16; 3], cache: &mut HashMap<[u16; 3], [u16; 3]>) {
    // eprintln!("{:?}", registers);
    // dbg!(&registers);
    if registers[0] != 0 {
        if registers[1]  != 0 {
            let val = registers[0];
            // registers[1] += 32767;
            // registers[1] %= 32768;
            registers[1] -= 1;
            run_cached(registers, cache);
            registers[1] = registers[0];
            registers[0] = val - 1;
            // registers[0] = val - 32767;
            // registers[0] %= 32768;
            return run_cached(registers, cache);
        }
        // registers[0] += 32767;
        // registers[0] %= 32768;
        registers[0] -= 1;
        registers[1] = registers[2];
        return run_cached(registers, cache);
    }

    registers[0] = registers[1] + 1;
    registers[0] %= 32768;

}

fn run(registers: &mut [u16; 8]) {
    // eprintln!("{:?}", registers);
    // dbg!(&registers);
    if registers[0] != 0 {
        if registers[1]  != 0 {
            let val = registers[0];
            registers[1] += 32767;
            registers[1] %= 32768;
            run(registers);
            registers[1] = registers[0];
            registers[0] = val + 32767;
            registers[0] %= 32768;
            run(registers);
            return
        }
        registers[0] += 32767;
        registers[0] %= 32768;
        registers[1] = registers[7];
        run(registers);
        return
    }
    registers[0] = registers[1] + 1;
    registers[0] %= 32768;
    return
}

fn run_for_present(val: u16) -> [u16; 3] {
    // let mut registers = [4, 5445, 3, 10, 101, 0, 0, i];
    let mut registers = [4, 5445, val];
    // let mut cache = Default::default();
    let mut cache = Default::default();
    run_cached(&mut registers, &mut cache);
    registers
}

fn main() {
    // let pool = threadpool::ThreadPool::new(1);
    // let mut pool = threadpool::Builder::new().num_threads(12).thread_stack_size(64_000_000).build();
    // let a= std::thread::spawn(|| {
    //     dbg!(run_for_present(1));
    // });
    // a.join().unwrap();
    // 27284
    // 27285
    // 27286
    // 28000
    for i in 1..=u16::MAX {
    // for i in 27284..=u16::MAX {
    // for i in 20..25 {
        // pool.execute(move || {
            // dbg!(i);
            let rv = run_for_present(i);

            eprintln!("{}: {:?}", i, rv);
            // dbg!("Done", i);
            if rv[0] == 6 {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    println!("Found: {}", i);
                }
            }
        // });
    }
    // pool.join();

    println!("Hello, world!");
}
