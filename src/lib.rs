pub struct Pool<T: Sync> {

}

struct WrapCell<T>(UnsafeCell<T>);
unsafe impl<T: Sync> Sync for WrapCell<Sync> {}