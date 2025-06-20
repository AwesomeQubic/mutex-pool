use criterion::{Criterion, criterion_group, criterion_main};
use mutex_pool::simple::AtomicU64Pool;
use std::{
    mem::forget,
    sync::Arc,
    thread,
};

fn count_pool(size: usize, threads: usize) {
    let pool = Arc::new(AtomicU64Pool::new(vec![u16::MAX; size]).unwrap());

    let mut handles = Vec::new();

    for _ in 0..threads {
        let pool = pool.clone();
        let handle = thread::spawn(move || {
            loop {
                let mut locked = match pool.try_lock() {
                    Some(x) => x,
                    None => {
                        return;
                    }
                };

                advance(&mut locked);
                if *locked == 1 {
                    forget(locked);
                }
            }
        });
        handles.push(handle);
    }

    for ele in handles {
        ele.join();
    }
}

fn advance(i: &mut u16) {
    *i ^= *i >> 7;
    *i ^= *i << 9;
    *i ^= *i >> 13;
}

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("count_pool_64_8", |x| x.iter(|| count_pool(64, 8)));
    c.bench_function("count_pool_64_4", |x| x.iter(|| count_pool(64, 4)));
    c.bench_function("count_pool_64_1", |x| x.iter(|| count_pool(64, 1)));
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
