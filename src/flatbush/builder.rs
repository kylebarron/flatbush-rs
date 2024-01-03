use std::marker::PhantomData;

use bytemuck::cast_slice_mut;

use crate::flatbush::constants::VERSION;
use crate::flatbush::index::OwnedFlatbush;
use crate::flatbush::sort::{Sort, SortParams};
use crate::flatbush::util::compute_num_nodes;
use crate::indices::MutableIndices;
use crate::r#type::IndexableNum;

pub struct FlatbushBuilder<N: IndexableNum> {
    /// data buffer
    data: Vec<u8>,
    num_items: usize,
    node_size: usize,
    num_nodes: usize,
    level_bounds: Vec<usize>,
    nodes_byte_size: usize,
    indices_byte_size: usize,

    pos: usize,

    min_x: N,
    min_y: N,
    max_x: N,
    max_y: N,
}

impl<N: IndexableNum> FlatbushBuilder<N> {
    pub fn new(num_items: usize) -> Self {
        Self::new_with_node_size(num_items, 16)
    }

    pub fn new_with_node_size(num_items: usize, node_size: usize) -> Self {
        assert!((2..=65535).contains(&node_size));
        assert!(num_items <= u32::MAX.try_into().unwrap());

        let (num_nodes, level_bounds) = compute_num_nodes(num_items, node_size);

        let indices_bytes_per_element = if num_nodes < 16384 { 2 } else { 4 };
        let nodes_byte_size = num_nodes * 4 * N::BYTES_PER_ELEMENT;
        let indices_byte_size = num_nodes * indices_bytes_per_element;

        let data_buffer_length = 8 + nodes_byte_size + indices_byte_size;
        let mut data = vec![0; data_buffer_length];

        // Set data header
        data[0] = 0xfb;
        data[1] = (VERSION << 4) + N::TYPE_INDEX;
        cast_slice_mut(&mut data[2..4])[0] = node_size as u16;
        cast_slice_mut(&mut data[4..8])[0] = num_items as u32;

        Self {
            data,
            num_items,
            num_nodes,
            node_size,
            level_bounds,
            nodes_byte_size,
            indices_byte_size,
            pos: 0,
            min_x: N::max_value(),
            min_y: N::max_value(),
            max_x: N::min_value(),
            max_y: N::min_value(),
        }
    }

    /// Add a given rectangle to the index.
    #[inline]
    pub fn add(&mut self, min_x: N, min_y: N, max_x: N, max_y: N) -> usize {
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

    pub fn finish<S: Sort<N>>(mut self) -> OwnedFlatbush<N> {
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
                phantom: PhantomData,
            };
        }

        let mut sort_params = SortParams {
            num_items: self.num_items,
            node_size: self.node_size,
            min_x: self.min_x,
            min_y: self.min_y,
            max_x: self.max_x,
            max_y: self.max_y,
        };
        S::sort(&mut sort_params, boxes, &mut indices);

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

                        if boxes[pos] < node_min_x {
                            node_min_x = boxes[pos];
                        }
                        pos += 1;
                        if boxes[pos] < node_min_y {
                            node_min_y = boxes[pos];
                        }
                        pos += 1;
                        if boxes[pos] > node_max_x {
                            node_max_x = boxes[pos]
                        }
                        pos += 1;
                        if boxes[pos] > node_max_y {
                            node_max_y = boxes[pos]
                        }
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
            phantom: PhantomData,
        }
    }
}

/// Mutable borrow of boxes and indices
fn split_data_borrow<N: IndexableNum>(
    data: &mut [u8],
    num_nodes: usize,
    nodes_byte_size: usize,
    indices_byte_size: usize,
) -> (&mut [N], MutableIndices) {
    let (boxes_buf, indices_buf) = data[8..].split_at_mut(nodes_byte_size);
    debug_assert_eq!(indices_buf.len(), indices_byte_size);

    let boxes = cast_slice_mut(boxes_buf);
    let indices = MutableIndices::new(indices_buf, num_nodes);
    (boxes, indices)
}
