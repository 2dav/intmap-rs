Cache-efficient map with integer-only keys
- Linear probing
- Open addressing
- Robin-hood hashing
- Fixed capacity

Note:
- memory pages backing aux data(u8*capacity) are being touched upon construction and `clear` 
operation, thus you're paying for what you don't use
- drops underlying memory all at once, without calling individual destructors

### Benchmarks
`hashbrown` with `fxhash` as a baseline, 32/64 means the type of key used i32/i64
``` 
**Successfull lookups** i.e. lookup for an element that is in the map
hashbrown32  19 ns    intmap32  6 ns
hashbrown64  18 ns    intmap64  11 ns

**Unsuccessfull lookups** i.e. lookup for an element that's not in the map
hashbrown32  3 ns     intmap32  5 ns
hashbrown64  3 ns     intmap64  5 ns

**Random keys**
insertions:
hashbrown32  16 ns    intmap32  9 ns
hashbrown64  17 ns    intmap64  13 ns
deletions:
hashbrown32  21 ns    intmap32  10 ns
hashbrown64  21 ns    intmap64  12 ns

**Monotonically increasing keys**
insertions:
hashbrown32  47 ns    intmap32  3 ns   
hashbrown64  55 ns    intmap64  6 ns
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
