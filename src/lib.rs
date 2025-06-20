use std::cell::UnsafeCell;

pub mod async_pool;
pub mod simple;

pub type Pool<T> = simple::AtomicU64Pool<T>;

#[repr(transparent)]
pub(crate) struct WrapCell<T>(UnsafeCell<T>);
unsafe impl<T: Sync> Sync for WrapCell<T> {}
