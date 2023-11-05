use std::marker::PhantomData;

use bytemuck::cast_slice_mut;

use crate::constants::VERSION;
use crate::index::OwnedFlatbush;
use crate::indices::MutableIndices;
use crate::util::compute_num_nodes;

const ARRAY_TYPE_INDEX: u8 = 8;

pub struct FlatbushBuilder {
    /// data buffer
    data: Vec<u8>,
    num_items: usize,
    node_size: usize,
    num_nodes: usize,
    level_bounds: Vec<usize>,
    nodes_byte_size: usize,
    indices_byte_size: usize,

    pos: usize,

    min_x: f64,
    min_y: f64,
    max_x: f64,
    max_y: f64,

    // Used in the future to have
    // FlatbushBuilder<T>
    _type: PhantomData<f64>,
}

impl FlatbushBuilder {
    pub fn new(num_items: usize) -> Self {
        Self::new_with_node_size(num_items, 16)
    }

    pub fn new_with_node_size(num_items: usize, node_size: usize) -> Self {
        assert!((2..=65535).contains(&node_size));
        assert!(num_items <= u32::MAX.try_into().unwrap());

        let (num_nodes, level_bounds) = compute_num_nodes(num_items, node_size);

        let f64_bytes_per_element = 8;
        let indices_bytes_per_element = if num_nodes < 16384 { 2 } else { 4 };
        let nodes_byte_size = num_nodes * 4 * f64_bytes_per_element;
        let indices_byte_size = num_nodes * indices_bytes_per_element;

        let data_buffer_length = 8 + nodes_byte_size + indices_byte_size;
        let mut data = vec![0; data_buffer_length];

        // Set data header
        data[0] = 0xfb;
        data[1] = (VERSION << 4) + ARRAY_TYPE_INDEX;
        cast_slice_mut(&mut data[2..4])[0] = node_size as u16;
        cast_slice_mut(&mut data[4..8])[0] = node_size as u32;

        Self {
            data,
            num_items,
            num_nodes,
            node_size,
            level_bounds,
            nodes_byte_size,
            indices_byte_size,
            pos: 0,
            min_x: f64::INFINITY,
            min_y: f64::INFINITY,
            max_x: f64::NEG_INFINITY,
            max_y: f64::NEG_INFINITY,
            _type: PhantomData,
        }
    }

    /// Add a given rectangle to the index.
    pub fn add(&mut self, min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> usize {
        let index = self.pos >> 2;
        let (boxes, mut indices) = split_data_borrow(
            &mut self.data,
            self.num_nodes,
            self.nodes_byte_size,
            self.indices_byte_size,
        );

        indices.set(index, index);
        boxes[self.pos] = min_x;
        self.pos += 1;
        boxes[self.pos] = min_y;
        self.pos += 1;
        boxes[self.pos] = max_x;
        self.pos += 1;
        boxes[self.pos] = max_y;
        self.pos += 1;

        if min_x < self.min_x {
            self.min_x = min_x
        };
        if min_y < self.min_y {
            self.min_y = min_y
        };
        if max_x > self.max_x {
            self.max_x = max_x
        };
        if max_y > self.max_y {
            self.max_y = max_y
        };

        index
    }

    pub fn add_interleaved(&mut self, boxes: &[f64]) {
        debug_assert!(boxes.len() % 4 == 0);
        let num_boxes = boxes.len() / 4;

        let (tree_boxes, indices) = split_data_borrow(
            &mut self.data,
            self.num_nodes,
            self.nodes_byte_size,
            self.indices_byte_size,
        );

        tree_boxes[self.pos..self.pos + boxes.len()].clone_from_slice(boxes);

        match indices {
            MutableIndices::U16(indices_arr) => {
                let current_index = self.pos >> 2;
                let new_indices =
                    Vec::from_iter(current_index as u16..(current_index + num_boxes) as u16);
                indices_arr[current_index..current_index + num_boxes]
                    .clone_from_slice(&new_indices);
            }
            MutableIndices::U32(indices_arr) => {
                let current_index = self.pos >> 2;
                let new_indices =
                    Vec::from_iter(current_index as u32..(current_index + num_boxes) as u32);
                indices_arr[current_index..current_index + num_boxes]
                    .clone_from_slice(&new_indices);
            }
        }

        self.pos += boxes.len();

        for box_ in boxes.chunks(4) {
            let min_x = box_[0];
            let min_y = box_[1];
            let max_x = box_[2];
            let max_y = box_[3];

            if min_x < self.min_x {
                self.min_x = min_x
            };
            if min_y < self.min_y {
                self.min_y = min_y
            };
            if max_x > self.max_x {
                self.max_x = max_x
            };
            if max_y > self.max_y {
                self.max_y = max_y
            };
        }
    }

    pub fn finish(mut self) -> OwnedFlatbush {
        assert_eq!(
            self.pos >> 2,
            self.num_items,
            "Added {} items when expected {}.",
            self.pos >> 2,
            self.num_items
        );
        let (boxes, mut indices) = split_data_borrow(
            &mut self.data,
            self.num_nodes,
            self.nodes_byte_size,
            self.indices_byte_size,
        );

        if self.num_items <= self.node_size {
            // only one node, skip sorting and just fill the root box
            boxes[self.pos] = self.min_x;
            self.pos += 1;
            boxes[self.pos] = self.min_y;
            self.pos += 1;
            boxes[self.pos] = self.max_x;
            self.pos += 1;
            boxes[self.pos] = self.max_y;
            self.pos += 1;

            return OwnedFlatbush {
                buffer: self.data,
                node_size: self.node_size,
                num_items: self.num_items,
                num_nodes: self.num_nodes,
                level_bounds: self.level_bounds,
            };
        }

        let width = self.max_x - self.min_x; // || 1.0;
        let height = self.max_y - self.min_y; // || 1.0;
        let mut hilbert_values: Vec<u32> = Vec::with_capacity(self.num_items);
        let hilbert_max = ((1 << 16) - 1) as f64;

        {
            // map item centers into Hilbert coordinate space and calculate Hilbert values
            let mut pos = 0;
            for _ in 0..self.num_items {
                let min_x = boxes[pos];
                pos += 1;
                let min_y = boxes[pos];
                pos += 1;
                let max_x = boxes[pos];
                pos += 1;
                let max_y = boxes[pos];
                pos += 1;

                let x = (hilbert_max * ((min_x + max_x) / 2. - self.min_x) / width).floor() as u32;
                let y = (hilbert_max * ((min_y + max_y) / 2. - self.min_y) / height).floor() as u32;
                hilbert_values.push(hilbert(x, y));
            }
        }

        // sort items by their Hilbert value (for packing later)
        sort(
            &mut hilbert_values,
            boxes,
            &mut indices,
            0,
            self.num_items - 1,
            self.node_size,
        );

        {
            // generate nodes at each tree level, bottom-up
            let mut pos = 0;
            for end in self.level_bounds[..self.level_bounds.len() - 1].iter() {
                while pos < *end {
                    let node_index = pos;

                    // calculate bbox for the new node
                    let mut node_min_x = boxes[pos];
                    pos += 1;
                    let mut node_min_y = boxes[pos];
                    pos += 1;
                    let mut node_max_x = boxes[pos];
                    pos += 1;
                    let mut node_max_y = boxes[pos];
                    pos += 1;
                    for _ in 1..self.node_size {
                        if pos >= *end {
                            break;
                        }

                        node_min_x = node_min_x.min(boxes[pos]);
                        pos += 1;
                        node_min_y = node_min_y.min(boxes[pos]);
                        pos += 1;
                        node_max_x = node_max_x.max(boxes[pos]);
                        pos += 1;
                        node_max_y = node_max_y.max(boxes[pos]);
                        pos += 1;
                    }

                    // add the new node to the tree data
                    indices.set(self.pos >> 2, node_index);
                    boxes[self.pos] = node_min_x;
                    self.pos += 1;
                    boxes[self.pos] = node_min_y;
                    self.pos += 1;
                    boxes[self.pos] = node_max_x;
                    self.pos += 1;
                    boxes[self.pos] = node_max_y;
                    self.pos += 1;
                }
            }
        }

        OwnedFlatbush {
            buffer: self.data,
            node_size: self.node_size,
            num_items: self.num_items,
            num_nodes: self.num_nodes,
            level_bounds: self.level_bounds,
        }
    }
}

/// Mutable borrow of boxes and indices
fn split_data_borrow(
    data: &mut [u8],
    num_nodes: usize,
    nodes_byte_size: usize,
    indices_byte_size: usize,
) -> (&mut [f64], MutableIndices) {
    let (boxes_buf, indices_buf) = data[8..].split_at_mut(nodes_byte_size);
    debug_assert_eq!(indices_buf.len(), indices_byte_size);

    let boxes = cast_slice_mut(boxes_buf);
    let indices = MutableIndices::new(indices_buf, num_nodes);
    (boxes, indices)
}

/// Custom quicksort that partially sorts bbox data alongside the hilbert values.
// Partially taken from static_aabb2d_index under the MIT/Apache license
fn sort(
    values: &mut [u32],
    boxes: &mut [f64],
    indices: &mut MutableIndices,
    left: usize,
    right: usize,
    node_size: usize,
) {
    debug_assert!(left <= right);

    if left / node_size >= right / node_size {
        return;
    }

    let midpoint = (left + right) / 2;
    let pivot = values[midpoint];
    let mut i = left.wrapping_sub(1);
    let mut j = right.wrapping_add(1);

    loop {
        loop {
            i = i.wrapping_add(1);
            if values[i] >= pivot {
                break;
            }
        }

        loop {
            j = j.wrapping_sub(1);
            if values[j] <= pivot {
                break;
            }
        }

        if i >= j {
            break;
        }

        swap(values, boxes, indices, i, j);
    }

    sort(values, boxes, indices, left, j, node_size);
    sort(values, boxes, indices, j.wrapping_add(1), right, node_size);
}

/// Swap two values and two corresponding boxes.
#[inline]
fn swap(values: &mut [u32], boxes: &mut [f64], indices: &mut MutableIndices, i: usize, j: usize) {
    values.swap(i, j);

    let k = 4 * i;
    let m = 4 * j;
    boxes.swap(k, m);
    boxes.swap(k + 1, m + 1);
    boxes.swap(k + 2, m + 2);
    boxes.swap(k + 3, m + 3);

    indices.swap(i, j);
}

// Taken from static_aabb2d_index under the mit/apache license
// https://github.com/jbuckmccready/static_aabb2d_index/blob/9e6add59d77b74d4de0ac32159db47fbcb3acc28/src/static_aabb2d_index.rs#L486C1-L544C2
fn hilbert(x: u32, y: u32) -> u32 {
    // Fast Hilbert curve algorithm by http://threadlocalmutex.com/
    // Ported from C++ https://github.com/rawrunprotected/hilbert_curves (public domain)
    let mut a_1 = x ^ y;
    let mut b_1 = 0xFFFF ^ a_1;
    let mut c_1 = 0xFFFF ^ (x | y);
    let mut d_1 = x & (y ^ 0xFFFF);

    let mut a_2 = a_1 | (b_1 >> 1);
    let mut b_2 = (a_1 >> 1) ^ a_1;
    let mut c_2 = ((c_1 >> 1) ^ (b_1 & (d_1 >> 1))) ^ c_1;
    let mut d_2 = ((a_1 & (c_1 >> 1)) ^ (d_1 >> 1)) ^ d_1;

    a_1 = a_2;
    b_1 = b_2;
    c_1 = c_2;
    d_1 = d_2;
    a_2 = (a_1 & (a_1 >> 2)) ^ (b_1 & (b_1 >> 2));
    b_2 = (a_1 & (b_1 >> 2)) ^ (b_1 & ((a_1 ^ b_1) >> 2));
    c_2 ^= (a_1 & (c_1 >> 2)) ^ (b_1 & (d_1 >> 2));
    d_2 ^= (b_1 & (c_1 >> 2)) ^ ((a_1 ^ b_1) & (d_1 >> 2));

    a_1 = a_2;
    b_1 = b_2;
    c_1 = c_2;
    d_1 = d_2;
    a_2 = (a_1 & (a_1 >> 4)) ^ (b_1 & (b_1 >> 4));
    b_2 = (a_1 & (b_1 >> 4)) ^ (b_1 & ((a_1 ^ b_1) >> 4));
    c_2 ^= (a_1 & (c_1 >> 4)) ^ (b_1 & (d_1 >> 4));
    d_2 ^= (b_1 & (c_1 >> 4)) ^ ((a_1 ^ b_1) & (d_1 >> 4));

    a_1 = a_2;
    b_1 = b_2;
    c_1 = c_2;
    d_1 = d_2;
    c_2 ^= (a_1 & (c_1 >> 8)) ^ (b_1 & (d_1 >> 8));
    d_2 ^= (b_1 & (c_1 >> 8)) ^ ((a_1 ^ b_1) & (d_1 >> 8));

    a_1 = c_2 ^ (c_2 >> 1);
    b_1 = d_2 ^ (d_2 >> 1);

    let mut i0 = x ^ y;
    let mut i1 = b_1 | (0xFFFF ^ (i0 | a_1));

    i0 = (i0 | (i0 << 8)) & 0x00FF00FF;
    i0 = (i0 | (i0 << 4)) & 0x0F0F0F0F;
    i0 = (i0 | (i0 << 2)) & 0x33333333;
    i0 = (i0 | (i0 << 1)) & 0x55555555;

    i1 = (i1 | (i1 << 8)) & 0x00FF00FF;
    i1 = (i1 | (i1 << 4)) & 0x0F0F0F0F;
    i1 = (i1 | (i1 << 2)) & 0x33333333;
    i1 = (i1 | (i1 << 1)) & 0x55555555;

    (i1 << 1) | i0
}

#[cfg(test)]
mod test {
    use super::*;

    use std::fs::read;

    use bytemuck::cast_slice;

    #[test]
    fn tmp() {
        let path = "/Users/kyle/github/kylebarron/flatbush-rs/benches/bounds.raw";
        let x = read(path).unwrap();
        let boxes: Vec<f64> = cast_slice(&x).to_vec();

        let mut builder = FlatbushBuilder::new(boxes.len() / 4);
        for box_ in boxes.chunks(4) {
            let min_x = box_[0];
            let min_y = box_[1];
            let max_x = box_[2];
            let max_y = box_[3];
            builder.add(min_x, min_y, max_x, max_y);
        }
        let _owned_flatbush = builder.finish();
    }
}
