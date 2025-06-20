use mutex_pool::async_pool::AsyncAtomicU64Pool;
use std::{
    sync::{Arc, atomic::AtomicU64},
    time::Duration,
};
use tokio::{spawn, sync::Notify};
use tokio::{
    sync::{
        Barrier,
        mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel},
    },
    time::sleep,
};

static TEST: AtomicU64 = AtomicU64::new(0);

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn test() {
    println!("Testing async");
    let (a_s, a_r) = unbounded_channel::<u64>();
    let (b_s, b_r) = unbounded_channel::<u64>();
    let notfiy = Arc::new(Notify::new());
    let pool = Arc::new(AsyncAtomicU64Pool::new(vec![a_s, b_s]).unwrap());

    spawn(recv(a_r, notfiy.clone()));
    spawn(recv(b_r, notfiy.clone()));

    let blockade = Arc::new(Barrier::new(64));
    let mut handles = Vec::with_capacity(64);
    for i in 0..64 {
        let out = 1u64 << i;
        handles.push(spawn(worker(
            pool.clone(),
            out,
            blockade.clone(),
        )));
    }

    for ele in handles {
        let _ = ele.await;
    }

    if TEST.load(std::sync::atomic::Ordering::Acquire) != u64::MAX {
        panic!("TEST FAILED");
    }
}

async fn worker(
    pool: Arc<AsyncAtomicU64Pool<UnboundedSender<u64>>>,
    to_send: u64,
    sync: Arc<Barrier>,
) {
    sync.wait().await;
    let lock = pool.lock().await;
    let _ = lock.send(to_send);
    sleep(Duration::from_millis(30)).await;
    drop(lock);
}

async fn recv(mut recv: UnboundedReceiver<u64>, end: Arc<Notify>) {
    loop {
        tokio::select! {
            out = recv.recv() => {
                let Some(out) = out else {
                    return;
                };
                TEST.fetch_or(out, std::sync::atomic::Ordering::Relaxed);
            }
            _ = end.notified() => {
                return;
            }
        }
    }
}
