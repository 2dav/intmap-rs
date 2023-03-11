Cache-efficient map with integer-only keys
- Linear probing
- Open addressing
- Robin-hood hashing
- Fixed capacity

Memory overhead is only one byte per element with a huge "but" - all spare memory
allocated upfront, regardless of the number of elements stored, i.e. `N*(1 + size<K> + size<V>)` bytes, 
thus you pay for what you aren't using.

Based on the ["I Wrote The Fastest Hashtable"](https://probablydance.com/2017/02/26/i-wrote-the-fastest-hashtable) by Malte Skarupke.

### Benchmarks
`hashbrown` with `fxhash` as a baseline, 32/64 means the type of key used i32/i64
``` 
**Successfull lookups** i.e. lookup for an element that is in the map
hashbrown32  19 ns    intmap32  6 ns
hashbrown64  18 ns    intmap64  10 ns

**Unsuccessfull lookups** i.e. lookup for an element that's not in the map
hashbrown32  3 ns     intmap32  5 ns
hashbrown64  3 ns     intmap64  5 ns

**Random keys**
insertions:
hashbrown32  16 ns    intmap32  9 ns
hashbrown64  17 ns    intmap64  12 ns
deletions:
hashbrown32  21 ns    intmap32  9 ns
hashbrown64  21 ns    intmap64  12 ns

**Monotonically increasing keys**
insertions:
hashbrown32  47 ns    intmap32  3 ns   
hashbrown64  55 ns    intmap64  4 ns
deletions:
hashbrown32  45 ns    intmap32  2 ns
hashbrown64  38 ns    intmap64  2 ns

**Fuzz workload** i.e. series of mixed random ops(insert, delete, get, contains), avg. time per op
hashbrown64  43 ns/op    intmap64  18 ns/op

fxhash avg. time to hash i64 <1 ns
```

### Fuzzing
```
cargo install cargo-fuzz
cd fuzz
rustup override set nightly
cargo fuzz run intmapfuzz --release --debug-assertions -s address --jobs 12 -- -max_len=65536
```
> oom/timeout/crash: 0/0/0 time: 39606s 
