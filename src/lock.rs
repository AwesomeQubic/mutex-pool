use crossbeam_utils::CachePadded;

#[cfg(loom)]
pub(crate) use loom::sync::atomic::*;

#[cfg(not(loom))]
pub(crate) use std::sync::atomic::*;

pub struct GroupLockU64(crossbeam_utils::CachePadded<AtomicU64>);

impl GroupLockU64 {
    pub fn create(space: usize) -> Option<Self> {
        if space > u64::BITS as usize {
            return None;
        }

        let table = if space != u64::BITS as usize {
            u64::MAX << space
        } else {
            0
        };

        Some(Self(CachePadded::new(AtomicU64::new(table))))
    }

    pub fn alloc(&self) -> Option<usize> {
        use crossbeam_utils::Backoff;

        let backoff = Backoff::new();
        fn next_in_sequence(prev: u64) -> Option<(usize, u64)> /* index */ {
            let trailing = prev.trailing_ones();

            if trailing == usize::BITS {
                return None;
            }

            let mask = 1 << trailing;
            Some((trailing as usize, mask))
        }

        let mut prev = self.0.load(Ordering::Relaxed);
        while let Some((index, mask)) = next_in_sequence(prev) {
            match self.0.compare_exchange_weak(
                prev,
                prev | mask,
                Ordering::Release,
                Ordering::Relaxed,
            ) {
                Ok(_) => {
                    return Some(index);
                }
                Err(out) => {
                    prev = out;
                    backoff.spin();
                }
            }
        }
        None
    }

    /*SAFETY: Caller has to ensure that resource is actually free */
    pub unsafe fn free(&self, index: usize) {
        let mask = !(1 << index);
        //We do not care about the order here just that its happens atomically
        self.0.fetch_and(mask, Ordering::Release);
    }
}
