use std::cmp;

use bytemuck::cast_slice_mut;

use crate::indices::MutableIndices;
use crate::kdbush::constants::{KDBUSH_HEADER_SIZE, KDBUSH_MAGIC, KDBUSH_VERSION};
use crate::kdbush::OwnedKdbush;

// Scalar array type to match js
// https://github.com/mourner/kdbush/blob/0309d1e9a1a53fd47f65681c6845627c566d63a6/index.js#L2-L5
const ARRAY_TYPE_INDEX: u8 = 8;

const DEFAULT_NODE_SIZE: usize = 64;

pub struct KdbushBuilder {
    /// data buffer
    data: Vec<u8>,

    num_items: usize,
    node_size: usize,

    coords_byte_size: usize,
    ids_byte_size: usize,
    pad_coords_byte_size: usize,

    pos: usize,
}

impl KdbushBuilder {
    pub fn new(num_items: usize) -> Self {
        Self::new_with_node_size(num_items, DEFAULT_NODE_SIZE)
    }

    pub fn new_with_node_size(num_items: usize, node_size: usize) -> Self {
        assert!((2..=65535).contains(&node_size));
        assert!(num_items <= u32::MAX.try_into().unwrap());

        // TODO: make generic and remove hardcoded f64
        let f64_bytes_per_element = 8;
        let coords_byte_size = num_items * 2 * f64_bytes_per_element;
        let indices_bytes_per_element = if num_items < 65536 { 2 } else { 4 };
        let ids_byte_size = num_items * indices_bytes_per_element;
        let pad_coords_byte_size = (8 - (ids_byte_size % 8)) % 8;

        let data_buffer_length =
            KDBUSH_HEADER_SIZE + coords_byte_size + ids_byte_size + pad_coords_byte_size;
        let mut data = vec![0; data_buffer_length];

        // Set data header;
        data[0] = KDBUSH_MAGIC;
        data[1] = (KDBUSH_VERSION << 4) + ARRAY_TYPE_INDEX;
        cast_slice_mut(&mut data[2..4])[0] = node_size as u16;
        cast_slice_mut(&mut data[4..8])[0] = num_items as u32;

        Self {
            data,
            num_items,
            node_size,
            coords_byte_size,
            ids_byte_size,
            pad_coords_byte_size,
            pos: 0,
        }
    }

    /// Add a point to the index.
    pub fn add(&mut self, x: f64, y: f64) -> usize {
        let index = self.pos >> 1;
        let (coords, mut ids) = split_data_borrow(
            &mut self.data,
            self.num_items,
            self.ids_byte_size,
            self.coords_byte_size,
            self.pad_coords_byte_size,
        );

        ids.set(index, index);
        coords[self.pos] = x;
        self.pos += 1;
        coords[self.pos] = y;
        self.pos += 1;

        index
    }

    pub fn finish(mut self) -> OwnedKdbush {
        assert_eq!(
            self.pos >> 1,
            self.num_items,
            "Added {} items when expected {}.",
            self.pos >> 1,
            self.num_items
        );

        let (coords, mut ids) = split_data_borrow(
            &mut self.data,
            self.num_items,
            self.ids_byte_size,
            self.coords_byte_size,
            self.pad_coords_byte_size,
        );

        // kd-sort both arrays for efficient search
        sort(&mut ids, coords, self.node_size, 0, self.num_items - 1, 0);

        OwnedKdbush {
            buffer: self.data,
            node_size: self.node_size,
            num_items: self.num_items,
        }
    }
}

/// Mutable borrow of coords and ids
fn split_data_borrow(
    data: &mut [u8],
    num_items: usize,
    ids_byte_size: usize,
    coords_byte_size: usize,
    pad_coords: usize,
) -> (&mut [f64], MutableIndices) {
    let (ids_buf, padded_coords_buf) = data[KDBUSH_HEADER_SIZE..].split_at_mut(ids_byte_size);
    let coords_buf = &mut padded_coords_buf[pad_coords..];
    debug_assert_eq!(coords_buf.len(), coords_byte_size);

    let ids = if num_items < 65536 {
        MutableIndices::U16(cast_slice_mut(ids_buf))
    } else {
        MutableIndices::U32(cast_slice_mut(ids_buf))
    };
    let coords = cast_slice_mut(coords_buf);

    (coords, ids)
}

fn sort(
    ids: &mut MutableIndices,
    coords: &mut [f64],
    node_size: usize,
    left: usize,
    right: usize,
    axis: usize,
) {
    if right - left <= node_size {
        return;
    }

    // middle index
    let m = (left + right) >> 1;

    // sort ids and coords around the middle index so that the halves lie either left/right or
    // top/bottom correspondingly (taking turns)
    select(ids, coords, m, left, right, axis);

    // recursively kd-sort first half and second half on the opposite axis
    sort(ids, coords, node_size, left, m - 1, 1 - axis);
    sort(ids, coords, node_size, m + 1, right, 1 - axis);
}

/// Custom Floyd-Rivest selection algorithm: sort ids and coords so that [left..k-1] items are
/// smaller than k-th item (on either x or y axis)
#[inline]
fn select(
    ids: &mut MutableIndices,
    coords: &mut [f64],
    k: usize,
    mut left: usize,
    mut right: usize,
    axis: usize,
) {
    while right > left {
        if right - left > 600 {
            let n = (right - left + 1) as f64;
            let m = (k - left + 1) as f64;
            let z = f64::ln(n);
            let s = 0.5 * f64::exp((2.0 * z) / 3.0);
            let sd = 0.5
                * f64::sqrt((z * s * (n - s)) / n)
                * (if m - n / 2.0 < 0.0 { -1.0 } else { 1.0 });
            let new_left = cmp::max(left, f64::floor(k as f64 - (m * s) / n + sd) as usize);
            let new_right = cmp::min(
                right,
                f64::floor(k as f64 + ((n - m) * s) / n + sd) as usize,
            );
            select(ids, coords, k, new_left, new_right, axis);
        }

        let t = coords[2 * k + axis];
        let mut i = left;
        let mut j = right;

        swap_item(ids, coords, left, k);
        if coords[2 * right + axis] > t {
            swap_item(ids, coords, left, right);
        }

        while i < j {
            swap_item(ids, coords, i, j);
            i += 1;
            j -= 1;
            while coords[2 * i + axis] < t {
                i += 1;
            }
            while coords[2 * j + axis] > t {
                j -= 1;
            }
        }

        if coords[2 * left + axis] == t {
            swap_item(ids, coords, left, j);
        } else {
            j += 1;
            swap_item(ids, coords, j, right);
        }

        if j <= k {
            left = j + 1;
        }
        if k <= j {
            right = j - 1;
        }
    }
}

#[inline]
fn swap_item(ids: &mut MutableIndices, coords: &mut [f64], i: usize, j: usize) {
    ids.swap(i, j);
    coords.swap(2 * i, 2 * j);
    coords.swap(2 * i + 1, 2 * j + 1);
}
