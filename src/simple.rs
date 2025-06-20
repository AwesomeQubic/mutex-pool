use std::{
    mem::transmute,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU32, AtomicU64, AtomicUsize},
    thread::{self, Thread},
};

use crossbeam_utils::CachePadded;

#[cfg(target_arch = "aarch64")]
use wyrand::WyRand;

use crate::WrapCell;

#[cfg(target_arch = "aarch64")]
static COUNTER: AtomicU64 = AtomicU64::new(0);

#[cfg(target_arch = "aarch64")]
thread_local! {
    static ID: u32 = {
        let seed = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        WyRand::gen_u64(seed).0 as u32
    }
}

pub struct AtomicU64Pool<T: Sync> {
    table: CachePadded<AtomicU64>,
    inner: Vec<WrapCell<T>>,
}

impl<T: Sync> AtomicU64Pool<T> {
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
            Ok(AtomicU64Pool {
                table: CachePadded::new(AtomicU64::new(table)),
                inner: new_vec,
            })
        }
    }

    pub fn try_lock<'a>(&'a self) -> Option<AtomicU64PoolGuard<'a, T>> {
        let Some(index) = self.alloc() else {
            return None;
        };

        Some(AtomicU64PoolGuard {
            index: index,
            pool: &self,
        })
    }

    #[cfg(target_arch = "aarch64")]
    fn alloc(&self) -> Option<usize> {
        fn next_in_sequence(prev: u64, distinguish: u32) -> Option<(usize, u64)> /* index */ {
            let trailing = prev.trailing_ones();

            if trailing == usize::BITS {
                return None;
            }

            let free = prev.count_zeros();
            let target = distinguish % free;

            let index = find_nth_zero_bit_position(prev, target + 1);
            let mask = 1 << index;

            Some((index as usize, mask))
        }

        let tid = ID.with(|x| *x);

        let mut prev = self.table.load(std::sync::atomic::Ordering::Relaxed);
        while let Some((index, mask)) = next_in_sequence(prev, tid) {
            let current = self
                .table
                .fetch_or(mask, std::sync::atomic::Ordering::Relaxed);

            if successful(current, mask) {
                return Some(index);
            } else {
                //println!("({s}) {prev:b} -> {current:b} {mask:b} ({index}) ({tid})");
                prev = current
            };
        }
        None
    }

    #[cfg(target_arch = "x86_64")]
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
    }
}

/// SAFETY: Caller must ensure that `num` has at least `n` zero bits
#[cfg(target_arch = "aarch64")]
fn find_nth_zero_bit_position(num: u64, mut n: u32) -> usize {
    let mut pos: u32 = 0;
    loop {
        let shifted = num >> pos;
        let zeros = shifted.trailing_zeros();

        if n <= zeros {
            return (pos + n) as usize - 1;
        }

        // Skip this zero run and the next run of 1s
        n -= zeros;
        pos += zeros;

        // Fast-forward through 1s
        pos += shifted.trailing_ones();
    }
}

#[cfg(target_arch = "aarch64")]
fn successful(table: u64, mask: u64) -> bool {
    (table & mask) == 0
}

#[derive(Debug)]
pub enum PoolCreationError {
    TooManyValues,
}

pub struct AtomicU64PoolGuard<'a, T: Sync> {
    index: usize,
    pool: &'a AtomicU64Pool<T>,
}

impl<'a, T: Sync> AtomicU64PoolGuard<'a, T> {
    pub fn index(&self) -> usize {
        self.index
    }
}

impl<'a, T: Sync> Deref for AtomicU64PoolGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe {
            //Since index is given to use beforehand we know its safe
            let entry = self.pool.inner.get_unchecked(self.index);
            &*(entry.0.get())
        }
    }
}

impl<'a, T: Sync> DerefMut for AtomicU64PoolGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe {
            //Since index is given to use beforehand we know its safe
            let entry = self.pool.inner.get_unchecked(self.index);
            &mut *(entry.0.get())
        }
    }
}

impl<'a, T: Sync> Drop for AtomicU64PoolGuard<'a, T> {
    fn drop(&mut self) {
        //We know this index is safe
        unsafe { self.pool.free(self.index) };
    }
}
