use mutex_pool::simple::AtomicU64Pool;

fn main() {
    let pool = AtomicU64Pool::new(vec![MyCoolObject(0); 4]).unwrap();

    let my_object1 = pool.try_lock().unwrap();
    let my_object2 = pool.try_lock().unwrap();
    let my_object3 = pool.try_lock().unwrap();
    let my_object4 = pool.try_lock().unwrap();
    let my_error = pool.try_lock();
}

#[derive(Clone)]
struct MyCoolObject(u8);
