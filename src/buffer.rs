/// Typed chunk of heap memory with handy snippets
use std::{
    alloc::{self, Layout, LayoutError},
    mem, ptr,
    ptr::NonNull,
    slice::SliceIndex,
};

pub type AllocResult<T> = std::result::Result<T, LayoutError>;

#[derive(Debug)]
pub struct Buffer<T> {
    ptr: NonNull<T>,
    capacity: usize,
}

impl<T> Buffer<T> {
    pub const CACHE_LINE_SIZE: usize = 64;

    fn new(capacity: usize, alloc_fn: impl FnOnce(Layout) -> *mut u8) -> AllocResult<Self> {
        if capacity == 0 || std::mem::size_of::<T>() == 0 {
            panic!("Zero-sized heap buffer is like a decently-sized heap buffer, but not");
        }

        let layout = Self::layout(capacity)?;
        let buffer = match NonNull::new(alloc_fn(layout)) {
            Some(ptr) => ptr,
            None => alloc::handle_alloc_error(layout),
        };

        Ok(Self { capacity, ptr: buffer.cast() })
    }

    #[inline]
    fn layout(capacity: usize) -> AllocResult<alloc::Layout> {
        alloc::Layout::array::<T>(capacity).and_then(|l| l.align_to(Self::CACHE_LINE_SIZE))
    }
}

#[allow(unused)]
#[allow(clippy::missing_safety_doc)]
impl<T> Buffer<T> {
    #[inline]
    pub fn with_capacity(capacity: usize) -> AllocResult<Self> {
        Self::new(capacity, |l| unsafe { alloc::alloc(l) })
    }

    #[inline]
    pub fn with_capacity_zeroed(capacity: usize) -> AllocResult<Self> {
        Self::new(capacity, |l| unsafe { alloc::alloc_zeroed(l) })
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    #[inline]
    pub fn as_ptr(&self) -> *const T {
        self.ptr.as_ptr()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut T {
        self.as_ptr() as _
    }

    #[inline]
    pub unsafe fn as_ref(&self, index: impl Into<usize>) -> &T {
        &*self.offset(index)
    }

    #[inline]
    pub unsafe fn as_mut(&mut self, index: impl Into<usize>) -> &mut T {
        &mut *self.offset_mut(index)
    }

    #[inline]
    pub unsafe fn offset(&self, index: impl Into<usize>) -> *const T {
        self.as_ptr().add(index.into())
    }

    #[inline]
    pub unsafe fn offset_mut(&mut self, index: impl Into<usize>) -> *mut T {
        self.as_mut_ptr().add(index.into())
    }

    #[inline]
    pub unsafe fn as_slice<I>(&self, index: I) -> &[T]
    where
        I: SliceIndex<[T], Output = [T]>,
    {
        std::slice::from_raw_parts(self.as_ptr(), self.capacity).get_unchecked(index)
    }

    #[inline]
    pub unsafe fn as_slice_mut<I>(&mut self, index: I) -> &mut [T]
    where
        I: SliceIndex<[T], Output = [T]>,
    {
        std::slice::from_raw_parts_mut(self.as_mut_ptr(), self.capacity).get_unchecked_mut(index)
    }

    /// returns the size of the memory chunk this buffer holds, in bytes
    #[inline]
    pub fn size(&self) -> usize {
        mem::size_of::<T>() * self.capacity
    }

    /// returns the number of cache lines this buffer occupies
    #[inline]
    pub fn num_lines(&self) -> usize {
        (self.size() + (Self::CACHE_LINE_SIZE - 1)) / Self::CACHE_LINE_SIZE
    }
}

#[allow(unused)]
#[allow(clippy::missing_safety_doc)]
impl<T> Buffer<T> {
    /// shifts 'n' elements 'at' position for 'offset' positions at direction determined
    /// by the sign of the 'offset' parameter
    #[inline]
    pub unsafe fn shift(
        &mut self,
        at: impl Into<usize>,
        offset: impl Into<isize>,
        n: impl Into<usize>,
    ) {
        let p = self.offset_mut(at);
        ptr::copy(p, p.offset(offset.into()), n.into());
    }

    #[inline]
    pub unsafe fn fillu8(&mut self, byte: u8) {
        ptr::write_bytes(self.as_mut_ptr().cast::<u8>(), byte, self.size());
    }

    /// eject memory this buffer holds from the cache hierarchy, counted in cache lines
    #[inline]
    #[cfg_attr(miri, ignore)]
    pub unsafe fn eject_lines<I: IntoIterator<Item = usize>>(&self, index: I) {
        #[cfg(target_feature = "sse2")]
        #[cfg(not(miri))]
        {
            use std::arch::x86_64::_mm_clflush;
            index.into_iter().for_each(|line| {
                _mm_clflush(self.as_ptr().cast::<u8>().add(line * Self::CACHE_LINE_SIZE))
            });
        }
    }

    /// load buffer memory into all cache levels,
    /// counted in cache lines
    #[inline]
    #[cfg_attr(miri, ignore)]
    pub unsafe fn prefetch_lines<I: IntoIterator<Item = usize>>(&self, index: I) {
        #[cfg(target_feature = "sse")]
        #[cfg(not(miri))]
        {
            use std::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};
            index
                .into_iter()
                .step_by(2)
                .map(|i| self.as_ptr().cast::<i8>().add(i * Self::CACHE_LINE_SIZE))
                .for_each(|line| _mm_prefetch::<_MM_HINT_T0>(line));
        }
    }

    /// load buffer memory into all cache levels,
    /// counted in `T` elements
    #[inline]
    #[cfg_attr(miri, ignore)]
    pub unsafe fn prefetch<I: IntoIterator<Item = usize>>(&self, index: I) {
        #[cfg(target_feature = "sse")]
        #[cfg(not(miri))]
        {
            use std::arch::x86_64::{_mm_prefetch, _MM_HINT_T0};
            index
                .into_iter()
                .map(|i| self.offset(i).cast::<i8>())
                .for_each(|i| _mm_prefetch::<_MM_HINT_T0>(i));
        }
    }
}

impl<T: Clone> Buffer<T> {
    #[inline]
    #[allow(dead_code)]
    pub unsafe fn fill(&mut self, value: T) {
        self.as_slice_mut(..).fill(value);
    }
}

impl<T> Clone for Buffer<T> {
    #[inline]
    fn clone(&self) -> Self {
        let mut buf = Self::with_capacity(self.capacity).unwrap();
        unsafe { buf.as_mut_ptr().copy_from_nonoverlapping(self.as_ptr(), self.capacity) };
        buf
    }
}

impl<T> Drop for Buffer<T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // Buffer is non-growing and so `capacity` haven't been changed since allocation,
            // therefore layout is exact same as for allocation
            alloc::dealloc(self.ptr.as_ptr() as *mut u8, Self::layout(self.capacity).unwrap())
        };
    }
}
