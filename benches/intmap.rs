use arbitrary::Arbitrary;
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use fxhash;
use intmap_rs::IntMap;
use rand::distributions::Standard;
use rand::Rng;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Instant;

type Hashr = fxhash::FxBuildHasher;

pub fn criterion_benchmark(c: &mut Criterion) {
    macro_rules! bench {
        ($group:ident, $bname:tt, |$bench:ident| $body:block) => {
            $group.bench_function($bname, |$bench| $body);
        };
        ($gname:expr, $($bname:tt => |$bench:ident| $body:block)+) => {
            let mut group = c.benchmark_group($gname);
            $(bench!(group, $bname, |$bench| $body);)+
            group.finish();
        };
    }

    type K = i32;
    type V = i32;
    type K64 = i64;
    type V64 = i64;

    // Batch size
    const N: usize = 1_000_000;
    // Map's capacity double the batch size to get 0.5 load factor
    const CAP: usize = N.next_power_of_two() * 2;

    //
    // Maps
    //
    let mut intmap32 = IntMap::with_capacity(CAP as u32);
    let mut intmap64 = IntMap::with_capacity(CAP as u32);

    let mut brown32: HashMap<K, V, Hashr> =
        HashMap::with_capacity_and_hasher(CAP as usize, Default::default());
    let mut brown64: HashMap<K64, V64, Hashr> =
        HashMap::with_capacity_and_hasher(CAP as usize, Default::default());
    //
    // Bench data
    //
    let keys32 = rand::thread_rng()
        .sample_iter::<K, Standard>(Standard)
        .filter(|x| x < &(K::MAX - N as K))
        .map(|x| x + N as K)
        .take(N)
        .collect::<Vec<_>>();
    let keys64 = rand::thread_rng()
        .sample_iter::<K64, Standard>(Standard)
        .filter(|x| x < &(K64::MAX - N as K64))
        .map(|x| x + N as K64)
        .take(N)
        .collect::<Vec<_>>();

    #[derive(Arbitrary, Debug)]
    enum Op {
        Insert(K64, V64),
        Get(K64),
        Delete(K64),
        Contains(K64),
    }

    const OP_SIZE: usize = std::mem::size_of::<Op>();
    let b = rand::thread_rng()
        .sample_iter(rand::distributions::Standard)
        .take(OP_SIZE * N)
        .collect::<Vec<u8>>();
    let ops: Vec<Op> =
        Arbitrary::arbitrary_take_rest(arbitrary::Unstructured::new(&b[..])).unwrap();
    drop(b);

    // prefill maps
    for k in keys32.iter() {
        intmap32.insert(*k, *k as V);
        brown32.insert(*k, *k as V);
    }
    for k in keys64.iter() {
        intmap64.insert(*k, *k as V64);
        brown64.insert(*k, *k as V64);
    }

    //
    // 'fxhash' hashing time
    //
    bench!("hasher",
    "fxhash64" => |b|{
        b.iter_custom(|iters|{
         let start = Instant::now();
         for k in keys64.iter().cycle().take(iters as usize){
             let mut h = fxhash::FxHasher::default();
             (*k).hash(&mut h);
             black_box(h.finish());
         }
         start.elapsed()
        })
    });

    //
    // Successfull lookups, i.e. lookup for an element that is in the map
    //
    bench!("Successfull lookups",
        "brown32" => |b|{
            b.iter_custom(|iters|{
                let start = Instant::now();
                for key in keys32.iter().cycle().take(iters as usize) {
                    black_box(brown32.get(key));
                }
                start.elapsed()
            })
        }
        "intmap32" => |b|{
            b.iter_custom(|iters|{
                let start = Instant::now();
                for key in keys32.iter().cycle().take(iters as usize) {
                    black_box(intmap32.get(*key));
                }
                start.elapsed()
            })
        }
        "brown64" => |b|{
            b.iter_custom(|iters|{
                let start = Instant::now();
                for key in keys64.iter().cycle().take(iters as usize) {
                    black_box(brown64.get(key));
                }
                start.elapsed()
            })
        }
        "intmap64" => |b|{
            b.iter_custom(|iters|{
                let start = Instant::now();
                for key in keys64.iter().cycle().take(iters as usize) {
                    black_box(intmap64.get(*key));
                }
                start.elapsed()
            })
        }
    );

    //
    // Unsuccessfull lookups, i.e. lookup for an element that's not in the map
    //
    bench!("Unsuccessfull lookups",
        "brown32" => |b|{
            b.iter_custom(|iters| {
                let start = Instant::now();
                for key in (0..N).into_iter().cycle().take(iters as usize) {
                    black_box(brown32.get(&(key as K)));
                }
                start.elapsed()
            })
        }
        "intmap32" => |b|{
            b.iter_custom(|iters| {
                let start = Instant::now();
                for key in (0..N).into_iter().cycle().take(iters as usize) {
                    black_box(intmap32.get(key as K));
                }
                start.elapsed()
            })
        }
        "brown64" => |b|{
            b.iter_custom(|iters| {
                let start = Instant::now();
                for key in (0..N).into_iter().cycle().take(iters as usize) {
                    black_box(brown64.get(&(key as K64)));
                }
                start.elapsed()
            })
        }
        "intmap64" => |b|{
            b.iter_custom(|iters| {
                let start = Instant::now();
                for key in (0..N).into_iter().cycle().take(iters as usize) {
                    black_box(intmap64.get(key as K64));
                }
                start.elapsed()
            })
        }
    );

    //
    // Insertions, random numbers
    //
    brown32.clear();
    brown64.clear();
    intmap32.clear();
    intmap64.clear();

    bench!("Insertions random",
        "brown32" => |b|{
            b.iter_batched(
                || brown32.clone(),
                |mut map| {
                    for k in keys32.iter(){
                        map.insert(*k, *k as V);
                    }
                },
                criterion::BatchSize::LargeInput
            )
        }
        "intmap32" => |b|{
            b.iter_batched(
                || intmap32.clone(),
                |mut map| {
                    for k in keys32.iter(){
                        map.insert(*k, *k as V);
                    }
                },
                criterion::BatchSize::LargeInput
            )
        }
        "brown64" => |b|{
            b.iter_batched(
                || brown64.clone(),
                |mut map| {
                    for k in keys64.iter(){
                        map.insert(*k, *k as V64);
                    }
                },
                criterion::BatchSize::LargeInput
            )
        }
        "intmap64" => |b|{
            b.iter_batched(
                || intmap64.clone(),
                |mut map| {
                    for k in keys64.iter(){
                        map.insert(*k, *k as V64);
                    }
                },
                criterion::BatchSize::LargeInput
            )
        }
    );

    //
    // Insertions, linear sequence
    //
    bench!("Insertions linear",
        "vec32" => |b|{
            b.iter_custom(|iters| {
                let mut vec:Vec<(K, V)> = Vec::with_capacity(iters as usize);
                let start = Instant::now();
                for k in 0..iters as usize {
                    vec.push((k as K, k as V));
                }
                start.elapsed()
            })
        }
        "brown32" => |b|{
            b.iter_custom(|iters| {
                brown32.clear();
                brown32.reserve(iters as usize);
                let start = Instant::now();
                for k in 0..iters as usize {
                    black_box(brown32.insert(k as K, k as V));
                }
                start.elapsed()
            })
        }
        "intmap32" => |b|{
            b.iter_custom(|iters| {
                let mut intmap32 = IntMap::with_capacity((iters as u32).max(2));
                let start = Instant::now();
                for k in 0..iters as usize {
                    black_box(intmap32.insert(k as K, k as V));
                }
                start.elapsed()
            })
        }
        "brown64" => |b|{
            b.iter_custom(|iters| {
                brown64.clear();
                brown64.reserve(iters as usize);
                let start = Instant::now();
                for k in 0..iters as usize {
                    black_box(brown64.insert(k as K64, k as V64));
                }
                start.elapsed()
            })
        }
        "intmap64" => |b|{
            b.iter_custom(|iters| {
                let mut intmap64 = IntMap::with_capacity((iters as u32).max(2));
                let start = Instant::now();
                for k in 0..iters as usize {
                    black_box(intmap64.insert(k as K64, k as V64));
                }
                start.elapsed()
            })
        }
    );

    //
    // Deletions, random numbers
    //
    for k in keys32.iter() {
        brown32.insert(*k, *k as V);
        intmap32.insert(*k, *k as V);
    }
    for k in keys64.iter() {
        brown64.insert(*k, *k as V64);
        intmap64.insert(*k, *k as V64);
    }

    bench!("Deletions random",
        "brown32" => |b|{
            b.iter_batched(
                || brown32.clone(),
                |mut map|{
                    for k in keys32.iter(){
                        map.remove(k);
                    }
                },
                criterion::BatchSize::LargeInput
            )
        }
        "intmap32" => |b|{
            b.iter_batched(
                || intmap32.clone(),
                |mut map|{
                    for k in keys32.iter(){
                        map.remove(*k);
                    }
                },
                criterion::BatchSize::LargeInput
            )
        }
        "brown64" => |b|{
            b.iter_batched(
                || brown64.clone(),
                |mut map|{
                    for k in keys64.iter(){
                        map.remove(k);
                    }
                },
                criterion::BatchSize::LargeInput
            )
        }
        "intmap64" => |b|{
            b.iter_batched(
                || intmap64.clone(),
                |mut map|{
                    for k in keys64.iter(){
                        map.remove(*k);
                    }
                },
                criterion::BatchSize::LargeInput
            )
        }
    );

    //
    // Deletions, linear
    //
    bench!("Deletions linear",
        "brown32" => |b|{
            b.iter_custom(|iters| {
                brown32.clear();
                brown32.reserve(iters as usize);
                for k in 0..iters as usize{
                    brown32.insert(k as K, k as V);
                }
                let start = Instant::now();
                for k in 0..iters as usize {
                    black_box(brown32.remove(&(k as K)));
                }
                start.elapsed()
            })
        }
        "intmap32" => |b|{
            b.iter_custom(|iters| {
                let mut intmap32 = IntMap::with_capacity((iters as u32).max(2));
                for k in 0..iters as usize{
                    intmap32.insert(k as K, k as V);
                }
                let start = Instant::now();
                for k in 0..iters as usize {
                    black_box(intmap32.remove(k as K));
                }
                start.elapsed()
            })
        }
        "brown64" => |b|{
            b.iter_custom(|iters| {
                brown64.clear();
                brown64.reserve(iters as usize);
                for k in 0..iters as usize{
                    brown64.insert(k as K64, k as V64);
                }
                let start = Instant::now();
                for k in 0..iters as usize {
                    black_box(brown64.remove(&(k as K64)));
                }
                start.elapsed()
            })
        }
        "intmap64" => |b|{
            b.iter_custom(|iters| {
                let mut intmap = IntMap::with_capacity((iters as u32).max(2));
                for k in 0..iters as usize{
                    intmap.insert(k as K64, k as V64);
                }
                let start = Instant::now();
                for k in 0..iters as usize {
                    black_box(intmap.remove(k as K64));
                }
                start.elapsed()
            })
        }
    );

    bench!("Workload",
        "brown64" => |b|{
            b.iter_custom(|iters| {
                brown64.clear();
                brown64.reserve(iters as usize);
                let start = Instant::now();
                ops.iter().cycle().take(iters as usize).for_each(|op|
                    match op {
                        Op::Insert(key, value) => {
                            black_box(brown64.insert(*key, *value));
                        }
                        Op::Get(key) => {
                            black_box(brown64.get(key));
                        }
                        Op::Delete(key) => {
                            black_box(brown64.remove(key));
                        }
                        Op::Contains(key) => {
                            black_box(brown64.contains_key(key));
                        }
                    });
                start.elapsed()
            })
        }
        "intmap64" => |b|{
            b.iter_custom(|iters| {
                intmap64.clear();
                let start = Instant::now();
                ops.iter().cycle().take(iters as usize).for_each(|op|
                    match op {
                        Op::Insert(key, value) => {
                            black_box(intmap64.insert(*key, *value));
                        }
                        Op::Get(key) => {
                            black_box(intmap64.get(*key));
                        }
                        Op::Delete(key) => {
                            black_box(intmap64.remove(*key));
                        }
                        Op::Contains(key) => {
                            black_box(intmap64.contains(*key));
                        }
                    });
                start.elapsed()
            })
        }
    );
}

criterion_group! {
    name = benches;
    config = Criterion::default()
        .with_plots()
        .warm_up_time(std::time::Duration::from_secs(5))
        .measurement_time(std::time::Duration::from_secs(10));
    targets = criterion_benchmark
}

criterion_main!(benches);
