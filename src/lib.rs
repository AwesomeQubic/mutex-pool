use std::cell::UnsafeCell;

pub mod async_pool;
pub mod lock;
pub mod simple;

pub type Pool<T> = simple::AtomicU64Pool<T>;

#[repr(transparent)]
pub(crate) struct WrapCell<T>(UnsafeCell<T>);
unsafe impl<T: Sync> Sync for WrapCell<T> {}

pub(crate) fn convert<T: Sized>(b: Box<[T]>) -> Box<[WrapCell<T>]> {
    let ptr = Box::into_raw(b) as *mut [WrapCell<T>];
    unsafe { Box::from_raw(ptr) }
}
