use std::{

    mem::transmute,
    ops::{Deref, DerefMut},
    sync::atomic::AtomicU64,
    task::{Poll, Waker},
};

use crossbeam::queue::SegQueue;
use crossbeam_utils::CachePadded;

use crate::WrapCell;

pub struct AsyncAtomicU64Pool<T: Sync> {
    table: CachePadded<AtomicU64>,
    inner: Vec<WrapCell<T>>,
    wakers: SegQueue<Waker>,
}

impl<T: Sync> AsyncAtomicU64Pool<T> {
    pub fn new(vec: Vec<T>) -> Result<Self, PoolCreationError> {
        let len = vec.len();

        //usize::BITS is always < than usize::MAX
        if len > (u64::BITS as usize) {
            return Err(PoolCreationError::TooManyValues);
        }

        //SAFETY: We are using repr transparent thus its ok
        unsafe {
            let new_vec: Vec<WrapCell<T>> = transmute(vec);
            let table = if len != usize::BITS as usize {
                u64::MAX << len
            } else {
                0
            };
            Ok(AsyncAtomicU64Pool {
                table: CachePadded::new(AtomicU64::new(table)),
                inner: new_vec,
                wakers: SegQueue::new(),
            })
        }
    }

    pub fn lock(&self) -> GuardFuture<'_, T> {
        GuardFuture(self)
    }

    pub fn try_lock(&self) -> Option<AtomicU64PoolGuard<'_, T>> {
        let index = self.alloc()?;

        Some(AtomicU64PoolGuard {
            index,
            pool: self,
        })
    }

    fn alloc(&self) -> Option<usize> {
        fn next_in_sequence(prev: u64) -> Option<(usize, u64)> /* index */ {
            let trailing = prev.trailing_ones();

            if trailing == usize::BITS {
                return None;
            }

            let mask = 1 << trailing;
            Some((trailing as usize, mask))
        }

        let mut prev = self.table.load(std::sync::atomic::Ordering::Relaxed);
        while let Some((index, mask)) = next_in_sequence(prev) {
            match self.table.compare_exchange(
                prev,
                prev | mask,
                std::sync::atomic::Ordering::Relaxed,
                std::sync::atomic::Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Some(index);
                }
                Err(out) => prev = out,
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

        if let Some(next) = self.wakers.pop() {
            next.wake();
        }
    }
}

pub struct GuardFuture<'a, T: Sync>(&'a AsyncAtomicU64Pool<T>);

impl<'a, T: Sync> Future for GuardFuture<'a, T> {
    type Output = AtomicU64PoolGuard<'a, T>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let pool = self.0;

        match pool.try_lock() {
            Some(x) => Poll::Ready(x),
            None => {
                self.0.wakers.push(cx.waker().clone());
                Poll::Pending
            }
        }
    }
}

pub struct AtomicU64PoolGuard<'a, T: Sync> {
    index: usize,
    pool: &'a AsyncAtomicU64Pool<T>,
}

impl<T: Sync> AtomicU64PoolGuard<'_, T> {
    pub fn index(&self) -> usize {
        self.index
    }
}

impl<T: Sync> Deref for AtomicU64PoolGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            //Since index is given to use beforehand we know its safe
            let entry = self.pool.inner.get_unchecked(self.index);
            &*(entry.0.get())
        }
    }
}

impl<T: Sync> DerefMut for AtomicU64PoolGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            //Since index is given to use beforehand we know its safe
            let entry = self.pool.inner.get_unchecked(self.index);
            &mut *(entry.0.get())
        }
    }
}

impl<T: Sync> Drop for AtomicU64PoolGuard<'_, T> {
    fn drop(&mut self) {
        //We know this index is safe
        unsafe { self.pool.free(self.index) };
    }
}

#[derive(Debug)]
pub enum PoolCreationError {
    TooManyValues,
}
