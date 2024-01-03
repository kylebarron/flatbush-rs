use std::fmt::Debug;

use num_traits::{Bounded, Num, NumCast, ToPrimitive};

pub trait IndexableNum:
    Num + NumCast + ToPrimitive + PartialOrd + Debug + Send + Sync + bytemuck::Pod + Bounded
{
    /// The type index to match the array order of `ARRAY_TYPES` in flatbush JS
    const TYPE_INDEX: u8;
    /// The number of bytes per element
    const BYTES_PER_ELEMENT: usize;
}

impl IndexableNum for i8 {
    const TYPE_INDEX: u8 = 0;
    const BYTES_PER_ELEMENT: usize = 1;
}

impl IndexableNum for u8 {
    const TYPE_INDEX: u8 = 1;
    const BYTES_PER_ELEMENT: usize = 1;
}

impl IndexableNum for i16 {
    const TYPE_INDEX: u8 = 3;
    const BYTES_PER_ELEMENT: usize = 2;
}

impl IndexableNum for u16 {
    const TYPE_INDEX: u8 = 4;
    const BYTES_PER_ELEMENT: usize = 2;
}

impl IndexableNum for i32 {
    const TYPE_INDEX: u8 = 5;
    const BYTES_PER_ELEMENT: usize = 4;
}

impl IndexableNum for u32 {
    const TYPE_INDEX: u8 = 6;
    const BYTES_PER_ELEMENT: usize = 4;
}

impl IndexableNum for f32 {
    const TYPE_INDEX: u8 = 7;
    const BYTES_PER_ELEMENT: usize = 4;
}

impl IndexableNum for f64 {
    const TYPE_INDEX: u8 = 8;
    const BYTES_PER_ELEMENT: usize = 8;
}
