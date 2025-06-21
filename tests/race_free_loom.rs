use loom::sync::atomic::Ordering::*;
use loom::sync::atomic::*;
use loom::thread::{self, yield_now};
use std::sync::Arc;

fn test_locks(entries: usize, threads: usize) {
    let lock = Arc::new(mutex_pool::lock::GroupLockU64::create(entries).unwrap());
    let uses = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let lock = lock.clone();
            let uses = uses.clone();
            thread::spawn(move || {
                if let Some(index) = lock.alloc() {
                    let mask = 1u64 << index;
                    let old = uses.fetch_or(mask, Ordering::SeqCst);
                    if old & mask != 0 {
                        return true;
                    }

                    yield_now();

                    uses.fetch_and(!mask, Ordering::SeqCst);
                    unsafe { lock.free(index); }

                    false
                } else {
                    false
                }
            })
        })
        .collect();

    let failed = handles.into_iter().map(|x| x.join().unwrap()).any(|x| x);
    if failed {
        panic!("Thread had not exclusive access to a index");
    }
}

#[test]
fn loom_1_2() {
    loom::model(|| {
        test_locks(1, 2);
    });
}

#[test]
fn loom_1_3() {
    loom::model(|| {
        test_locks(1, 3);
    });
}

#[test]
fn loom_2_3() {
    loom::model(|| {
        test_locks(2, 3);
    });
}

#[test]
fn loom_3_3() {
    loom::model(|| {
        test_locks(3, 3);
    });
}