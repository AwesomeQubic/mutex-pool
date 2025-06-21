use std::{sync::Arc, thread};

use mutex_pool::simple::AtomicU64Pool;

#[test]
fn racey() {
    let a = Arc::new(AtomicU64Pool::new(vec![0]).unwrap());

    let a1 = a.clone();
    let a2 = a.clone();

    let t1 = thread::spawn(move || {
        if let Some(mut g) = a1.try_lock() {
            *g += 1;
        }
    });
    let t2 = thread::spawn(move || {
        if let Some(mut g) = a2.try_lock() {
            *g += 1;
        }
    });

    t1.join().unwrap();
    t2.join().unwrap();
}
