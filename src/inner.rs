use std::{
    borrow::Borrow,
    cmp::Ordering,
    marker::PhantomData,
    mem::MaybeUninit,
    ops::{Deref, DerefMut},
    ptr::NonNull,
    slice::SliceIndex,
};

use super::buffer::{self, Buffer};

pub type Distance = i8;
pub const FREE: Distance = -1;

#[derive(Clone)]
pub struct Inner<K, V> {
    distances: Buffer<Distance>,
    keys: Buffer<MaybeUninit<K>>,
    values: Buffer<MaybeUninit<V>>,
}

pub struct RowRef<K, V, R> {
    index: usize,
    inner: NonNull<Inner<K, V>>,
    _marker: PhantomData<R>,
}

impl<'a, K, V: 'a> Copy for RowRef<K, V, Immut<'a>> {}
impl<'a, K, V: 'a> Clone for RowRef<K, V, Immut<'a>> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

unsafe impl<K, V: Send> Send for Inner<K, V> {}
unsafe impl<K, V: Sync> Sync for Inner<K, V> {}

pub struct Immut<'a>(PhantomData<&'a ()>);
pub struct Mut<'a>(PhantomData<&'a mut ()>);

impl<K: Default + Clone, V> Inner<K, V> {
    pub fn with_capacity(capacity: usize) -> buffer::AllocResult<Self> {
        let value = Buffer::with_capacity(capacity)?;
        let key = Buffer::with_capacity(capacity)?;
        let distance = Buffer::with_capacity(capacity)?;
        let mut this = Self { keys: key, values: value, distances: distance };
        this.mark_all_free();
        Ok(this)
    }

    pub fn mark_all_free(&mut self) {
        unsafe {
            self.distances.fill(FREE);
            // eject touched memory(above first 64 lines) from the CPU caches
            let nlines = self.distances.num_lines();
            self.distances.eject_lines(64.min(nlines)..nlines);
        }
    }
}

impl<K, V> Inner<K, V> {
    /// Safety:
    /// ensure 'index' is within bounds
    pub unsafe fn row(&self, index: usize) -> RowRef<K, V, Immut<'_>> {
        debug_assert!(index < self.keys.capacity());
        RowRef {
            index,
            inner: NonNull::new_unchecked(self as *const _ as *mut _),
            _marker: PhantomData,
        }
    }

    /// Safety:
    /// ensure 'index' is within bounds
    pub unsafe fn row_mut(&mut self, index: usize) -> RowRef<K, V, Mut<'_>> {
        debug_assert!(index < self.keys.capacity());
        RowRef { index, inner: NonNull::new_unchecked(self as *mut _), _marker: PhantomData }
    }

    #[inline]
    pub fn keys<I>(&self, index: I) -> &[K]
    where
        I: SliceIndex<[MaybeUninit<K>], Output = [MaybeUninit<K>]>,
    {
        unsafe { slice_assume_init_ref(self.keys.as_slice(index)) }
    }

    #[inline]
    pub fn values<I>(&self, index: I) -> &[V]
    where
        I: SliceIndex<[MaybeUninit<V>], Output = [MaybeUninit<V>]>,
    {
        unsafe { slice_assume_init_ref(self.values.as_slice(index)) }
    }
}

impl<K, V, R> Deref for RowRef<K, V, R> {
    type Target = Inner<K, V>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { self.inner.as_ref() }
    }
}

impl<K, V> DerefMut for RowRef<K, V, Mut<'_>> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.inner.as_mut() }
    }
}

impl<K, V, R> RowRef<K, V, R> {
    pub fn distance(&self) -> &Distance {
        unsafe { self.distances.as_ref(self.index) }
    }

    pub fn key(&self) -> &MaybeUninit<K> {
        unsafe { self.keys.as_ref(self.index) }
    }

    pub fn is_empty(&self) -> bool {
        FREE.eq(self.distance())
    }

    // Safety:
    // ensure 'index' + 1 is within bounds
    unsafe fn next(self) -> Self {
        Self { index: self.index + 1, inner: self.inner, _marker: PhantomData }
    }
}

impl<'a, K, V> RowRef<K, V, Immut<'a>> {
    pub fn value(&self) -> &'a MaybeUninit<V> {
        unsafe { &*self.values.offset(self.index) }
    }
}

impl<'a, K, V: 'a> RowRef<K, V, Mut<'a>> {
    pub fn distance_mut(&mut self) -> &mut Distance {
        let ix = self.index;
        unsafe { self.distances.as_mut(ix) }
    }

    pub fn key_mut(&mut self) -> &mut MaybeUninit<K> {
        let ix = self.index;
        unsafe { self.keys.as_mut(ix) }
    }

    pub fn value_mut(&mut self) -> &'a mut MaybeUninit<V> {
        let ix = self.index;
        unsafe { &mut *self.values.offset_mut(ix) }
    }

    pub fn insert(self, key: K, value: V, distance: Distance) {
        match self.is_empty() {
            true => self.write(key, value, distance),
            false => self.emplace(key, value, distance),
        }
    }

    // Safety:
    // Ensure value is initialized
    pub unsafe fn remove(mut self) -> V {
        let value = self.value_mut().assume_init_read();
        let mut distance = self.distance_mut();

        *distance = FREE;

        loop {
            self = self.next();

            distance = self.distance_mut();

            if *distance < 1 {
                break;
            }

            *distance -= 1;

            self.swap_with_index(self.index - 1);
        }

        value
    }

    unsafe fn swap_with_index(&mut self, j: usize) {
        use std::ptr::swap_nonoverlapping;
        let i = self.index;
        swap_nonoverlapping(self.distances.offset_mut(i), self.distances.offset_mut(j), 1);
        swap_nonoverlapping(self.keys.offset_mut(i), self.keys.offset_mut(j), 1);
        swap_nonoverlapping(self.values.offset_mut(i), self.values.offset_mut(j), 1);
    }

    fn swap(&mut self, key: &mut K, value: &mut V, distance: &mut Distance) {
        use std::ptr::swap_nonoverlapping;
        unsafe {
            let ix = self.index;
            swap_nonoverlapping(self.distances.offset_mut(ix), distance as *mut _, 1);
            swap_nonoverlapping(self.keys.offset_mut(ix), key as *mut _ as _, 1);
            swap_nonoverlapping(self.values.offset_mut(ix), value as *mut _ as _, 1);
        }
    }

    fn write(mut self, key: K, value: V, distance: Distance) {
        *self.distance_mut() = distance;
        self.key_mut().write(key);
        self.value_mut().write(value);
    }

    fn emplace(mut self, mut key: K, mut value: V, mut distance: Distance) {
        self.swap(&mut key, &mut value, &mut distance);

        loop {
            assert!(distance < Distance::MAX, "probes count overflow, increase initial capacity");
            distance += 1;

            self = unsafe { self.next() };

            if self.is_empty() {
                self.write(key, value, distance);
                break;
            } else if distance.gt(self.distance()) {
                self.swap(&mut key, &mut value, &mut distance);
            }
        }
    }
}

pub enum SearchResult<K, V, R> {
    Found(RowRef<K, V, R>),
    NotFound(RowRef<K, V, R>, Distance),
}

impl<K, V, R> SearchResult<K, V, R> {
    pub fn is_found(&self) -> bool {
        match self {
            SearchResult::Found(_) => true,
            SearchResult::NotFound(..) => false,
        }
    }
}

impl<K, V, R> RowRef<K, V, R> {
    pub fn search<Q: ?Sized>(mut self, key: &Q) -> SearchResult<K, V, R>
    where
        Q: Ord,
        K: Borrow<Q>,
    {
        for distance in 0..Distance::MAX {
            self = match self.distance_key_cmp(distance, key) {
                Ordering::Less => return SearchResult::NotFound(self, distance),
                Ordering::Equal => return SearchResult::Found(self),
                Ordering::Greater => unsafe { self.next() },
            }
        }
        panic!("maximum probes count reached, you might want to increase capacity");
    }

    #[inline]
    fn distance_key_cmp<Q: ?Sized>(&self, distance: Distance, key: &Q) -> Ordering
    where
        Q: Ord,
        K: Borrow<Q>,
    {
        // any 'distance' initiated by 'search' routine is greater than 'distance' of an empty 'slot',
        // thus, subsequent 'key' read is always valid since slot is non-empty
        if distance.gt(self.distance()) {
            Ordering::Less
        } else if key.eq(unsafe { self.key().assume_init_ref().borrow() }) {
            Ordering::Equal
        } else {
            Ordering::Greater
        }
    }
}

#[inline]
unsafe fn slice_assume_init_ref<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    unsafe { &*(slice as *const [MaybeUninit<T>] as *const [T]) }
}
