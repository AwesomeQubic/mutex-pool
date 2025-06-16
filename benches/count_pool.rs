use criterion::{Criterion, criterion_group, criterion_main};
use mutex_pool::Pool;
use std::{
    hint::black_box,
    mem::forget,
    sync::{
        Arc,
        atomic::{AtomicU32, AtomicUsize},
    },
    thread,
};

fn count_pool(size: usize, threads: usize) {
    let pool = Arc::new(Pool::new(vec![u16::MAX; size]).unwrap());
    let counter = Arc::new(AtomicUsize::new(size));

    let mut handles = Vec::new();

    for i in 0..threads {
        let pool = pool.clone();
        let counter = counter.clone();
        let handle = thread::spawn(move || {
            loop {
                let mut locked = match pool.try_lock() {
                    Some(x) => x,
                    None => {
                        if counter.load(std::sync::atomic::Ordering::Relaxed) < i + 1 {
                            return;
                        }
                        continue;
                    }
                };
                match locked.checked_sub(1) {
                    Some(x) => {
                        *locked = x;
                    }
                    None => {
                        //Keep it allocated
                        forget(locked);

                        if counter.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) > i + 1 {
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
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("count_pool_64_8", |x| x.iter(|| count_pool(64, 8)));
    c.bench_function("count_pool_64_4", |x| x.iter(|| count_pool(64, 4)));
    c.bench_function("count_pool_64_1", |x| x.iter(|| count_pool(64, 1)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
