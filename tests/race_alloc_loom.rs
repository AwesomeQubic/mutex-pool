use loom::sync::atomic::Ordering::*;
use loom::sync::atomic::*;
use loom::thread;
use std::sync::Arc;

fn test_locks(entries: usize, threads: usize) {
    let expected = entries.min(threads);

    let lock = Arc::new(mutex_pool::lock::GroupLockU64::create(entries).unwrap());

    let handles: Vec<_> = (0..threads)
        .map(|_| {
            let lock = lock.clone();
            thread::spawn(move || {
                if let Some(_) = lock.alloc() {
                    return 1;
                } else {
                    return 0;
                }
            })
        })
        .collect();

    let loaded = handles.into_iter().map(|x| x.join().unwrap()).sum();
    assert_eq!(expected, loaded);
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