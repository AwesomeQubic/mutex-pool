use std::{
    sync::Arc,
    thread::{self, park},
    time::Duration,
};

use mutex_pool::Pool;

fn main() {
    let pool = Pool::new(vec![2, 3, 4, 5, 5, 5, 5, 10]).unwrap();
    let pool_ref = &pool;
    thread::scope(move |scope| {
        for i in 0..20 {
            scope.spawn(move || {
                loop {
                    let locked = pool_ref.lock();
                    let index = locked.index();
                    println!("{i} will seep on {} for {}", locked.index(), *locked);
                    thread::sleep(Duration::from_secs(*locked));
                    println!("Freeing {index}");
                    drop(locked);
                }
            });
        }

        loop {
            park();
        }
    })
}
