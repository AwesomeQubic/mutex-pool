use std::{
    mem::forget,
    process::exit,
    sync::{Arc, atomic::AtomicUsize},
    thread,
};

use mutex_pool::Pool;

fn count_pool(size: usize, threads: usize) {
    let pool = Arc::new(Pool::new(vec![u16::MAX; size]).unwrap());
    let counter = Arc::new(AtomicUsize::new(size));
    let checker = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();

    println!("Starting threads");

    for i in 0..threads {
        let pool = pool.clone();
        let counter = counter.clone();
        let checker = checker.clone();
        let handle = thread::spawn(move || {
            loop {
                let Some(mut locked) = pool.try_lock() else {
                    let counter_now = counter.load(std::sync::atomic::Ordering::Relaxed);
                    if counter_now < i + 1 {
                        return;
                    }
                    continue;
                };

                let mask = 1 << locked.index();
                let old_or = checker.fetch_or(mask, std::sync::atomic::Ordering::Relaxed);
                if (old_or & mask) != 0 {
                    println!("DUAL ALLOCATION");
                    exit(-1);
                }

                match locked.checked_sub(1) {
                    Some(x) => {
                        *locked = x;
                        checker.fetch_and(!mask, std::sync::atomic::Ordering::Relaxed);
                    }
                    None => {
                        //Keep it allocated
                        forget(locked);
                        let counter_now =
                            counter.fetch_sub(1, std::sync::atomic::Ordering::Acquire);
                        if counter_now < i + 1 {
                            return;
                        }
                    }
                }
            }
        });
        handles.push(handle);
    }

    for ele in handles {
        ele.join();
    }

    if counter.load(std::sync::atomic::Ordering::Acquire) != 0 {
        panic!("INVALID RESULTS");
    }

    println!("Done exiting...");
}

fn main() {
    count_pool(64, 8);
}
