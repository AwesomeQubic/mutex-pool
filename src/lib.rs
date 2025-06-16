use std::{
    cell::UnsafeCell,
    mem::transmute,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, AtomicUsize},
};

use atomic_wait::wait;

pub struct Pool<T: Sync> {
    free: AtomicU32,

    
    table: AtomicUsize,
    inner: Vec<WrapCell<T>>,
}

impl<T: Sync> Pool<T> {
    pub fn new(vec: Vec<T>) -> Result<Self, PoolCreationError> {
        let len = vec.len();

        //usize::BITS is always < than usize::MAX
        if len > (usize::BITS as usize) {
            return Err(PoolCreationError::TooManyValues);
        }

        //SAFETY: We are using repr transparent thus its ok
        unsafe {
            let new_vec: Vec<WrapCell<T>> = transmute(vec);
            let table = if len != usize::BITS as usize {
                usize::MAX << len
            } else {
                0
            };
            Ok(Pool {
                free: AtomicU32::new(len as u32),
                table: AtomicUsize::new(table),
                inner: new_vec,
            })
        }
    }

    pub fn lock<'a>(&'a self) -> PoolGuard<'a, T> {
        let index;
        loop {
            match self.alloc() {
                Some(x) => {
                    index = x;
                    break;
                }
                None => wait(&self.free, 0),
            };
        }

        PoolGuard {
            index: index,
            pool: &self,
        }
    }

    pub fn try_lock<'a>(&'a self) -> Option<PoolGuard<'a, T>> {
        let Some(index) = self.alloc() else {
            return None;
        };

        Some(PoolGuard {
            index: index,
            pool: &self,
        })
    }

    fn alloc(&self) -> Option<usize> {
        const fn next_in_sequence(prev: usize) -> Option<(usize, usize)> /* (index, next) */ {
            let index = prev.trailing_ones();
            if index == usize::BITS {
                return None;
            }
            let mask = 1 << index;
            Some((index as usize, prev | mask))
        }

        let mut prev = self.table.load(std::sync::atomic::Ordering::SeqCst);
        while let Some((index, next)) = next_in_sequence(prev) {
            match self.table.compare_exchange_weak(
                prev,
                next,
                std::sync::atomic::Ordering::SeqCst,
                std::sync::atomic::Ordering::SeqCst,
            ) {
                x @ Ok(_) => {
                    self.free.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    return Some(index);
                }
                Err(next_prev) => prev = next_prev,
            }
        }
        None
    }

    /*SAFETY: Caller has to ensure that resource is actually free */
    unsafe fn free(&self, index: usize) {
        let mask = !(1 << index);
        //We do not care about the order here just that its happens atomically
        self.table
            .fetch_and(mask, std::sync::atomic::Ordering::Relaxed);

        //We do not care about the order here just that its happens atomically
        if self.free.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) == 1 {
            atomic_wait::wake_one(&self.free as *const AtomicU32);
        }
    }
}

#[derive(Debug)]
pub enum PoolCreationError {
    TooManyValues,
}

pub struct PoolGuard<'a, T: Sync> {
    index: usize,
    pool: &'a Pool<T>,
}

impl<'a, T: Sync> PoolGuard<'a, T> {
    pub fn index(&self) -> usize {
        self.index
    }
}

impl<'a, T: Sync> Deref for PoolGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            //Since index is given to use beforehand we know its safe
            let entry = self.pool.inner.get_unchecked(self.index);
            &*(entry.0.get())
        }
    }
}

impl<'a, T: Sync> DerefMut for PoolGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            //Since index is given to use beforehand we know its safe
            let entry = self.pool.inner.get_unchecked(self.index);
            &mut *(entry.0.get())
        }
    }
}

impl<'a, T: Sync> Drop for PoolGuard<'a, T> {
    fn drop(&mut self) {
        //We know this index is safe
        unsafe { self.pool.free(self.index) };
    }
}

#[repr(transparent)]
struct WrapCell<T>(UnsafeCell<T>);
unsafe impl<T: Sync> Sync for WrapCell<T> {}
