use std::ops::{Deref, DerefMut};

use crate::{WrapCell, lock::GroupLockU64};

pub struct AtomicU64Pool<T: Sync> {
    table: GroupLockU64,
    inner: Box<[WrapCell<T>]>,
}

impl<T: Sync> AtomicU64Pool<T> {
    pub fn new(vec: Vec<T>) -> Result<Self, PoolCreationError> {
        let len = vec.len();
        let Some(lock) = GroupLockU64::create(len) else {
            return Err(PoolCreationError::TooManyValues);
        };

        Ok(AtomicU64Pool {
            table: lock,
            inner: crate::convert(vec.into_boxed_slice()),
        })
    }

    pub fn try_lock(&self) -> Option<AtomicU64PoolGuard<'_, T>> {
        let index = self.table.alloc()?;

        Some(AtomicU64PoolGuard { index, pool: self })
    }

    unsafe fn free(&self, index: usize) {
        unsafe {
            self.table.free(index);
        }
    }
}

#[derive(Debug)]
pub enum PoolCreationError {
    TooManyValues,
}

pub struct AtomicU64PoolGuard<'a, T: Sync> {
    index: usize,
    pool: &'a AtomicU64Pool<T>,
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
