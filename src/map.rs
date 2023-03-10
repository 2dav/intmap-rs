use std::{
    borrow::Borrow,
    cmp::Ordering,
    mem::{self, MaybeUninit},
    ops::{Index, IndexMut},
};

pub type Distance = i8;
pub const FREE: Distance = -1;

pub struct Table<K, V> {
    distances: Buffer<Distance>,
    keys: Buffer<MaybeUninit<K>>,
    values: Buffer<MaybeUninit<V>>,
    capacity: usize,
    len: usize,
}

unsafe impl<K, V: Send> Send for Table<K, V> {}
unsafe impl<K, V: Sync> Sync for Table<K, V> {}

impl<K, V> Table<K, V> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            keys: Buffer::with_capacity(capacity),
            values: Buffer::with_capacity(capacity),
            distances: Buffer::with_capacity_filled(capacity, FREE),
            len: 0,
            capacity,
        }
    }

    pub(crate) fn keys(&self) -> &[K] {
        unsafe { slice_assume_init_ref(self.keys.as_slice(self.len)) }
    }

    pub(crate) fn values(&self) -> &[V] {
        unsafe { slice_assume_init_ref(self.values.as_slice(self.len)) }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn clear(&mut self) {
        self.distances.fill(FREE, self.capacity);
        self.len = 0;
    }
}

impl<K, V> Table<K, V> {
    pub fn search<Q>(&self, key: &Q, mut index: usize) -> SearchResult
    where
        K: Borrow<Q>,
        Q: ?Sized + Ord,
    {
        if self.distances[index] == FREE {
            return SearchResult::NotFound(index, 0);
        } else if key.eq(unsafe { self.keys[index].assume_init_ref().borrow() }) {
            return SearchResult::Found(index);
        }

        index += 1;

        if self.distances[index] < 1 {
            return SearchResult::NotFound(index, 1);
        } else if key.eq(unsafe { self.keys[index].assume_init_ref().borrow() }) {
            return SearchResult::Found(index);
        }

        index += 1;

        for distance in 2..Distance::MAX {
            index = match self.distance_key_cmp(index, distance, key) {
                Ordering::Less => return SearchResult::NotFound(index, distance),
                Ordering::Equal => return SearchResult::Found(index),
                Ordering::Greater => index + 1,
            }
        }
        panic!("maximum probes count reached, you might want to increase capacity");
    }

    fn distance_key_cmp<Q: ?Sized>(&self, index: usize, distance: Distance, key: &Q) -> Ordering
    where
        Q: Ord,
        K: Borrow<Q>,
    {
        // any 'distance' initiated by 'search' routine is greater than 'distance' of an empty 'slot',
        // thus, subsequent 'key' read is always valid since slot is non-empty
        if distance > self.distances[index] {
            Ordering::Less
        } else if key.eq(unsafe { self.keys[index].assume_init_ref().borrow() }) {
            Ordering::Equal
        } else {
            Ordering::Greater
        }
    }
}

impl<K, V> Table<K, V> {
    pub fn insert(&mut self, index: usize, mut key: K, mut value: V, mut distance: Distance) {
        if self.distances[index] == FREE {
            self.write(index, key, value, distance);
        } else {
            // Safety:
            // to this point we know the slot is non-empty and thus it's memory is initialized
            unsafe { self.swap_at(index, &mut key, &mut value, &mut distance) };
            self.emplace(index + 1, key, value, distance + 1);
        }

        self.len += 1;
    }

    pub fn remove(&mut self, index: usize) -> V {
        self.len -= 1;
        self.distances[index] = FREE;

        let ret = unsafe { self.values[index].assume_init_read() };

        self.shift_up(index);

        ret
    }

    #[inline(never)]
    fn emplace(&mut self, mut index: usize, mut key: K, mut value: V, mut distance: Distance) {
        loop {
            if self.distances[index] == FREE {
                self.write(index, key, value, distance);
                break;
            } else if distance > self.distances[index] {
                unsafe { self.swap_at(index, &mut key, &mut value, &mut distance) };
            }

            assert!(distance < Distance::MAX, "probes count overflow, increase initial capacity");
            distance += 1;
            index += 1;
        }
    }

    fn shift_up(&mut self, mut index: usize) {
        loop {
            index = index + 1;

            if self.distances[index] < 1 {
                break;
            }

            self.distances[index] -= 1;

            unsafe { self.swap_indices(index, index - 1) };
        }
    }

    #[inline]
    fn write(&mut self, index: usize, key: K, value: V, distance: Distance) {
        self.keys[index].write(key);
        self.values[index].write(value);
        self.distances[index] = distance;
    }

    // SAFETY:
    // ensure that memory at 'index' is properly initialized
    #[inline]
    unsafe fn swap_at(&mut self, i: usize, key: &mut K, value: &mut V, distance: &mut Distance) {
        use std::ptr::swap_nonoverlapping;
        swap_nonoverlapping(self.distances.offset_mut(i), distance as *mut _, 1);
        swap_nonoverlapping(self.keys.offset_mut(i), key as *mut _ as *mut _, 1);
        swap_nonoverlapping(self.values.offset_mut(i), value as *mut _ as *mut _, 1);
    }

    // SAFETY:
    // ensure that memory at both indices is properly initialized
    #[inline]
    unsafe fn swap_indices(&mut self, i: usize, j: usize) {
        self.distances.swap_indices(i, j);
        self.keys.swap_indices(i, j);
        self.values.swap_indices(i, j);
    }
}

impl<K, V: Clone> Clone for Table<K, V> {
    #[inline]
    fn clone(&self) -> Self {
        let mut distances = Buffer::with_capacity_filled(self.capacity, FREE);
        let mut keys = Buffer::with_capacity(self.capacity);
        let mut values = Buffer::with_capacity(self.capacity);
        unsafe {
            self.distances.copy_to(&mut distances, self.capacity);
            self.keys.copy_to(&mut keys, self.capacity);
            self.values.copy_to(&mut values, self.capacity);
        }
        Self { distances, keys, values, capacity: self.capacity, len: self.len }
    }
}

impl<K, V> Drop for Table<K, V> {
    #[inline]
    fn drop(&mut self) {
        drop(unsafe {
            mem::replace(&mut self.distances, Buffer::with_capacity(0))
                .into_inner(self.capacity, self.capacity)
        });
        drop(unsafe {
            mem::replace(&mut self.keys, Buffer::with_capacity(0))
                .into_inner(self.capacity, self.capacity)
        });
        drop(unsafe {
            mem::replace(&mut self.values, Buffer::with_capacity(0))
                .into_inner(self.capacity, self.capacity)
        });
    }
}

impl<K, V> Index<usize> for Table<K, V> {
    type Output = V;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { self.values[index].assume_init_ref() }
    }
}

impl<K, V> IndexMut<usize> for Table<K, V> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { self.values[index].assume_init_mut() }
    }
}

pub enum SearchResult {
    Found(usize),
    NotFound(usize, Distance),
}

impl SearchResult {
    pub fn is_found(&self) -> bool {
        match self {
            SearchResult::Found(_) => true,
            SearchResult::NotFound(..) => false,
        }
    }
}

#[repr(transparent)]
struct Buffer<T>(*mut T);
impl<T> Buffer<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self(mem::ManuallyDrop::new(Vec::with_capacity(capacity)).as_mut_ptr())
    }

    pub fn as_slice(&self, len: usize) -> &[T] {
        unsafe { std::slice::from_raw_parts(self.0, len) }
    }

    pub fn as_slice_mut(&mut self, len: usize) -> &mut [T] {
        unsafe { std::slice::from_raw_parts_mut(self.0, len) }
    }

    pub unsafe fn into_inner(self, len: usize, capacity: usize) -> Vec<T> {
        Vec::from_raw_parts(self.0, len, capacity)
    }

    pub unsafe fn offset_mut(&self, offset: usize) -> *mut T {
        self.0.add(offset)
    }

    pub unsafe fn copy_to(&self, dst: &mut Buffer<T>, len: usize) {
        std::ptr::copy_nonoverlapping(self.0, dst.0, len);
    }

    pub unsafe fn swap_indices(&mut self, i: usize, j: usize) {
        std::ptr::swap_nonoverlapping(self.offset_mut(i), self.offset_mut(j), 1);
    }
}

impl<T: Copy> Buffer<T> {
    pub fn with_capacity_filled(capacity: usize, value: T) -> Self {
        let mut this = Self::with_capacity(capacity);
        this.fill(value, capacity);
        this
    }

    pub fn fill(&mut self, value: T, n: usize) {
        self.as_slice_mut(n).fill(value);
    }
}

impl<T> Index<usize> for Buffer<T> {
    type Output = T;

    #[inline]
    fn index(&self, index: usize) -> &Self::Output {
        unsafe { &*self.0.add(index) }
    }
}

impl<T> IndexMut<usize> for Buffer<T> {
    #[inline]
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        unsafe { &mut *self.0.add(index) }
    }
}

#[inline]
unsafe fn slice_assume_init_ref<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    unsafe { &*(slice as *const [MaybeUninit<T>] as *const [T]) }
}
