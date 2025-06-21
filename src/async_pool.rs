use std::{
    mem::transmute,
    ops::{Deref, DerefMut},
    task::{Poll, Waker},
};

use crossbeam::queue::SegQueue;
use crossbeam_utils::CachePadded;

use crate::{lock::GroupLockU64, WrapCell};

pub struct AsyncAtomicU64Pool<T: Sync> {
    table: GroupLockU64,
    inner: Box<[WrapCell<T>]>,
    wakers: SegQueue<Waker>,
}

impl<T: Sync> AsyncAtomicU64Pool<T> {
    pub fn new(vec: Vec<T>) -> Result<Self, PoolCreationError> {
        let len = vec.len();
        let Some(lock) = GroupLockU64::create(len) else {
            return Err(PoolCreationError::TooManyValues);
        };

        Ok(Self {
            wakers: SegQueue::new(),
            table: lock,
            inner: crate::convert(vec.into_boxed_slice()),
        })
    }

    pub fn lock(&self) -> GuardFuture<'_, T> {
        GuardFuture(self)
    }

    pub fn try_lock(&self) -> Option<AtomicU64PoolGuard<'_, T>> {
        let index = self.table.alloc()?;

        Some(AtomicU64PoolGuard { index, pool: self })
    }

    /*SAFETY: Caller has to ensure that resource is actually free */
    unsafe fn free(&self, index: usize) {
        unsafe {self.table.free(index) };

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
