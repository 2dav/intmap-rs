mod map;
use map::{Distance, Keys, SearchResult, Table};
use num_traits::{AsPrimitive, FromPrimitive, PrimInt};
use std::fmt::{Debug, Display};

mod private {
    pub trait SealedKey {}
}

macro_rules! sealed_set {
    ($name:ident [$($type_set:ty)+] $seal:path$(: $($bounds:path)*)?) => {
        pub trait $name: $($( $bounds +)*)? where Self: Sized { }
        $(impl $seal for $type_set{})+
        $(impl $name for $type_set {})+
    };
}

sealed_set!(IntKey [i32 u32 i64 usize u64 i128 u128] private::SealedKey: 
    Debug Display PrimInt FromPrimitive Default
    AsPrimitive::<u32> AsPrimitive::<usize>);

pub struct IntMap<K, V> {
    table: Table<K, V>,
    index_mask: K,
}

impl<K: IntKey, V> IntMap<K, V> {
    pub fn with_capacity(capacity: u32) -> Self {
        let capacity = capacity.min(1 << 30).next_power_of_two();
        let table_cap = capacity as usize + Distance::MAX as usize;
        let table = Table::with_capacity(table_cap);
        let index_mask = K::from_u32(capacity - 1).unwrap();

        Self { index_mask, table }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.table.clear();
    }
}

impl<K: IntKey, V> IntMap<K, V> {
    #[inline]
    fn index_for_key(&self, key: K) -> usize {
        (key & self.index_mask).as_()
    }
}

impl<K: IntKey, V> IntMap<K, V> {
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        match self.table.search(&key, self.index_for_key(key)) {
            SearchResult::Found(index) => Some(std::mem::replace(&mut self.table[index], value)),
            SearchResult::NotFound(index, distance) => {
                self.table.insert(index, key, value, distance);
                None
            }
        }
    }

    pub fn remove(&mut self, key: K) -> Option<V> {
        match self.table.search(&key, self.index_for_key(key)) {
            SearchResult::Found(index) => Some(self.table.remove(index)),
            SearchResult::NotFound(..) => None,
        }
    }

    pub fn get(&self, key: K) -> Option<&V> {
        match self.table.search(&key, self.index_for_key(key)) {
            SearchResult::Found(index) => Some(&self.table[index]),
            SearchResult::NotFound(..) => None,
        }
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        match self.table.search(&key, self.index_for_key(key)) {
            SearchResult::Found(index) => Some(&mut self.table[index]),
            SearchResult::NotFound(..) => None,
        }
    }

    pub fn contains(&self, key: K) -> bool {
        self.table.search(&key, self.index_for_key(key)).is_found()
    }

    pub fn keys(&self) -> Keys<K> {
        self.table.keys()
    }
}

impl<K: IntKey, V> IntMap<K, V> {
    pub fn len(&self) -> usize {
        self.table.len()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        1 + AsPrimitive::<usize>::as_(self.index_mask)
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    pub fn is_full(&self) -> bool {
        self.len() == self.capacity()
    }

    pub fn probes(&self) -> Vec<usize> {
        self.keys().enumerate().map(|(i, k)| i - self.index_for_key(*k)).collect()
    }

    #[inline]
    pub fn avg_probes_count(&self) -> f32 {
        (self.probes().into_iter().sum::<usize>() as f32) / self.len() as f32
    }

    #[inline]
    pub fn load_factor(&self) -> f32 {
        self.len() as f32 / self.capacity() as f32
    }
}

impl<K: Clone, V: Clone> Clone for IntMap<K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Self { table: self.table.clone(), index_mask: self.index_mask.clone() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        let m = IntMap::<i32, u32>::with_capacity(10);
        assert!(m.is_empty());
        assert!(!m.is_full());
        assert_eq!(m.len(), 0);
        // power of two capacity
        assert_eq!(m.capacity(), 16);
    }

    #[test]
    fn insert() {
        let mut m: IntMap<i32, i32> = IntMap::with_capacity(2);
        assert!(m.insert(1, 2).is_none());
        assert!(m.contains(1));
        assert!(!m.contains(0));
        assert_eq!(*m.get(1).unwrap(), 2);
        *m.get_mut(1).unwrap() = 4;
        assert!(m.contains(1));
        assert_eq!(*m.get(1).unwrap(), 4);
    }

    #[test]
    fn insert_overwrite() {
        let mut m = IntMap::with_capacity(2);
        m.insert(1, 2);
        assert_eq!(m.insert(1, 3), Some(2));
        assert!(m.contains(1));
        assert_eq!(m.len(), 1);
        assert_eq!(*m.get(1).unwrap(), 3);
    }

    #[test]
    fn insert_some() {
        const N: u32 = 128;
        let mut m = IntMap::with_capacity(N as u32);
        (0..N).for_each(|i| {
            m.insert(i, i);
        });
        assert_eq!(m.len(), N as usize);
        for i in 0..N {
            assert!(m.contains(i));
            assert_eq!(*m.get(i).unwrap(), i);
        }
    }

    #[test]
    fn insert_sparse_keys() {
        let mut m = IntMap::with_capacity(4);
        m.insert(0, 1);
        m.insert(3, 1);
        assert_eq!(m.keys().cloned().collect::<Vec<u32>>(), &[0, 3]);
    }

    #[test]
    fn insert_collide() {
        let mut m: IntMap<u32, u32> = IntMap::with_capacity(4);
        m.insert(0, 0);
        m.insert(1, 1);
        /*
        index key distance
        0     0   0
        1     1   0
        */
        m.insert(4, 2);
        /*
        index key distance
        0     0   0
        1     4   1
        2     1   1
        */
        assert_eq!(m.len(), 3);
        m.insert(8, 3);
        /*
        index key distance
        0     0   0
        1     4   1
        2     8   2
        3     1   2
        */
        assert_eq!(m.keys().cloned().collect::<Vec<u32>>(), &[0, 4, 8, 1]);
    }

    #[test]
    fn insert_collide_inside() {
        let mut m: IntMap<u32, u32> = IntMap::with_capacity(8);
        m.insert(0, 0);
        m.insert(8, 1);
        m.insert(1, 2);
        m.insert(2, 3);
        /*
        index key distance
        0     0   0
        1     8   1
        2     1   1
        3     2   1
        */
        m.insert(16, 4);
        /*
        index key distance
        0     0   0
        1     8   1
        2     16  2
        3     1   2
        4     2   2
        */
        assert_eq!(m.len(), 5);
        assert_eq!(m.keys().cloned().collect::<Vec<u32>>(), &[0, 8, 16, 1, 2]);
    }

    #[test]
    fn insert_signed_remove() {
        let mut m = IntMap::with_capacity(4);
        m.insert(-10, 1);
        m.insert(-20, 2);
        m.insert(-30, 3);
        m.insert(-40, 4);
        m.insert(-20, 22);
        assert!(m.is_full());
        assert_eq!(m.remove(-40), Some(4));
        assert_eq!(m.remove(-10), Some(1));
        assert_eq!(m.remove(-30), Some(3));
        assert_eq!(m.remove(-20), Some(22));
        assert!(m.is_empty());
    }

    #[test]
    fn remove() {
        let mut m = IntMap::with_capacity(4);
        m.insert(1, 1);
        assert_eq!(m.len(), 1);
        assert_eq!(m.remove(1), Some(1));
        assert!(m.is_empty());
        m.insert(1, 1);
        m.insert(2, 2);
        m.insert(3, 3);
        m.insert(4, 4);
        assert!(m.is_full());
        assert_eq!(m.remove(2), Some(2));
        assert_eq!(m.remove(4), Some(4));
        assert_eq!(m.remove(1), Some(1));
        assert_eq!(m.remove(3), Some(3));
        assert!(m.is_empty());
    }

    #[test]
    fn remove_conflict() {
        let mut m = IntMap::with_capacity(4);
        m.insert(1, 2);
        assert_eq!(*m.get(1).unwrap(), 2);
        m.insert(5, 3);
        assert_eq!(*m.get(1).unwrap(), 2);
        assert_eq!(*m.get(5).unwrap(), 3);
        m.insert(9, 4);
        assert_eq!(*m.get(1).unwrap(), 2);
        assert_eq!(*m.get(5).unwrap(), 3);
        assert_eq!(*m.get(9).unwrap(), 4);
        assert!(m.remove(1).is_some());
        assert_eq!(*m.get(9).unwrap(), 4);
        assert_eq!(*m.get(5).unwrap(), 3);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn insert_remove_seq() {
        const N: u32 = 1 << 11;
        let mut m = IntMap::with_capacity(N);

        for _ in 0..10 {
            assert!(m.is_empty());
            for i in 1..1001 {
                m.insert(i, i);

                for j in 1..=i {
                    let r = m.get(j);
                    assert_eq!(r, Some(&j));
                }

                for j in i + 1..1001 {
                    let r = m.get(j);
                    assert_eq!(r, None);
                }
            }

            for i in 1001..2001 {
                assert!(!m.contains(i));
            }

            // remove forwards
            for i in 1..1001 {
                assert!(m.remove(i).is_some());

                for j in 1..=i {
                    assert!(!m.contains(j));
                }

                for j in i + 1..1001 {
                    assert!(m.contains(j));
                }
            }

            for i in 1..1001 {
                assert!(!m.contains(i));
            }

            for i in 1..1001 {
                m.insert(i, i);
            }

            // remove backwards
            for i in (1..1001).rev() {
                assert!(m.remove(i).is_some());

                for j in i..1001 {
                    assert!(!m.contains(j));
                }

                for j in 1..i {
                    assert!(m.contains(j));
                }
            }
        }
    }

    #[test]
    fn drop_check() {
        use std::sync::Arc;
        let is_dropped = Arc::new(1);
        let mut m = IntMap::<u32, Arc<i32>>::with_capacity(2);
        m.insert(0, Arc::clone(&is_dropped));
        m.insert(1, Arc::clone(&is_dropped));
        drop(m);
        assert_eq!(Arc::strong_count(&is_dropped), 1);
    }

    #[test]
    fn drop_check_replace() {
        use std::sync::Arc;
        let is_dropped = Arc::new(1);
        let mut m = IntMap::<u32, Arc<i32>>::with_capacity(2);
        m.insert(0, Arc::clone(&is_dropped));
        m.insert(0, Arc::clone(&is_dropped));
        drop(m);
        assert_eq!(Arc::strong_count(&is_dropped), 1);
    }

    #[test]
    fn drop_check2() {
        let mut m = IntMap::<u32, String>::with_capacity(5);
        m.insert(0, String::from("should"));
        m.insert(1, String::from("not"));
        m.insert(2, String::from("crash"));
        m.insert(3, String::from("with"));
        m.insert(4, String::from("double-free"));
        drop(m);
    }
}
