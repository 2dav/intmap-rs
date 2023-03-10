#![no_main]
use arbitrary::Arbitrary;
use fxhash;
use intmap_rs::IntMap;
use libfuzzer_sys::fuzz_target;
use std::collections::HashMap;

#[derive(Arbitrary, Debug)]
enum Op {
    Insert(i32, u32),
    Get(i32),
    Delete(i32),
    Contains(i32),
}

fuzz_target!(|ops: Vec<Op>| {
    const CAP: u32 = 100000;
    let mut map = IntMap::<i32, u32>::with_capacity(CAP);
    let mut truth: HashMap<i32, u32, fxhash::FxBuildHasher> =
        HashMap::with_capacity_and_hasher(CAP as usize, Default::default());

    for op in ops {
        match op {
            Op::Insert(key, value) => {
                let a = truth.insert(key, value);
                let b = map.insert(key, value);
                assert_eq!(a, b);
            }
            Op::Get(key) => {
                let a = truth.get(&key);
                let b = map.get(key);
                assert_eq!(a, b);
            }
            Op::Delete(key) => {
                let a = truth.remove(&key);
                let b = map.remove(key);
                assert_eq!(a, b);
            }
            Op::Contains(key) => {
                let a = truth.contains_key(&key);
                let b = map.contains(key);
                assert_eq!(a, b);
            }
        }
    }
});
