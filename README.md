```
construction (flatbush) time:   [77.642 ms 77.880 ms 78.153 ms]
construction (flatbush f64 to f32, including casting)
                        time:   [86.559 ms 87.194 ms 88.119 ms]
construction (flatbush f32)
                        time:   [79.957 ms 80.450 ms 81.125 ms]
construction (rstar bulk)
                        time:   [154.73 ms 155.12 ms 155.57 ms]

search() results in 34384 items
search() on f32 results in 34391 items

flatbush buffer size: 41533064 bytes
flatbush f32 buffer size: 23073928 bytes

search (flatbush)       time:   [98.864 µs 98.967 µs 99.084 µs]
search (flatbush f32)   time:   [104.81 µs 105.86 µs 107.02 µs]
search (rstar)          time:   [149.09 µs 149.37 µs 149.64 µs]
```
