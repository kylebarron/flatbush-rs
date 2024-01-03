use std::marker::PhantomData;

use bytemuck::cast_slice;

use crate::flatbush::constants::VERSION;
use crate::flatbush::error::FlatbushError;
use crate::flatbush::r#trait::FlatbushIndex;
use crate::flatbush::util::compute_num_nodes;
use crate::indices::Indices;
use crate::r#type::IndexableNum;

#[derive(Debug, Clone, PartialEq)]
pub struct OwnedFlatbush<N: IndexableNum> {
    pub(crate) buffer: Vec<u8>,
    pub(crate) node_size: usize,
    pub(crate) num_items: usize,
    pub(crate) num_nodes: usize,
    pub(crate) level_bounds: Vec<usize>,
    pub(crate) phantom: PhantomData<N>,
}

impl<N: IndexableNum> OwnedFlatbush<N> {
    pub fn into_inner(self) -> Vec<u8> {
        self.buffer
    }

    pub fn as_flatbush(&self) -> FlatbushRef<N> {
        FlatbushRef {
            boxes: self.boxes(),
            indices: self.indices().into_owned(),
            node_size: self.node_size,
            num_items: self.num_items,
            num_nodes: self.num_nodes,
            level_bounds: self.level_bounds.clone(),
        }
    }
}

impl<N: IndexableNum> AsRef<[u8]> for OwnedFlatbush<N> {
    fn as_ref(&self) -> &[u8] {
        &self.buffer
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FlatbushRef<'a, N: IndexableNum> {
    pub(crate) boxes: &'a [N],
    pub(crate) indices: Indices<'a>,
    pub(crate) node_size: usize,
    pub(crate) num_items: usize,
    pub(crate) num_nodes: usize,
    pub(crate) level_bounds: Vec<usize>,
}

impl<'a, N: IndexableNum> FlatbushRef<'a, N> {
    pub fn try_new<T: AsRef<[u8]>>(data: &'a T) -> Result<Self, FlatbushError> {
        let data = data.as_ref();
        // TODO: validate length of slice?

        let magic = data[0];
        if magic != 0xfb {
            return Err(FlatbushError::General(
                "Data does not appear to be in a Flatbush format.".to_string(),
            ));
        }

        let version_and_type = data[1];
        let version = version_and_type >> 4;
        if version != VERSION {
            return Err(FlatbushError::General(
                format!("Got v{} data when expected v{}.", version, VERSION).to_string(),
            ));
        }

        let node_size: u16 = cast_slice(&data[2..4])[0];
        let num_items: u32 = cast_slice(&data[4..8])[0];
        let node_size = node_size as usize;
        let num_items = num_items as usize;

        let (num_nodes, level_bounds) = compute_num_nodes(num_items, node_size);

        let indices_bytes_per_element = if num_nodes < 16384 { 2 } else { 4 };
        let nodes_byte_length = num_nodes * 4 * N::BYTES_PER_ELEMENT;
        let indices_byte_length = num_nodes * indices_bytes_per_element;

        // TODO: assert length of `data` matches expected
        let boxes = cast_slice(&data[8..8 + nodes_byte_length]);
        let indices_buf = &data[8 + nodes_byte_length..8 + nodes_byte_length + indices_byte_length];
        let indices = Indices::new(indices_buf, num_nodes);

        Ok(Self {
            boxes,
            indices,
            node_size,
            num_items,
            num_nodes,
            level_bounds,
        })
    }

    pub fn search(&self, min_x: N, min_y: N, max_x: N, max_y: N) -> Vec<usize> {
        let mut outer_node_index = Some(self.boxes.len() - 4);

        let mut queue = vec![];
        let mut results = vec![];

        while let Some(node_index) = outer_node_index {
            // find the end index of the node
            let end =
                (node_index + self.node_size * 4).min(upper_bound(node_index, &self.level_bounds));

            // search through child nodes
            for pos in (node_index..end).step_by(4) {
                // check if node bbox intersects with query bbox
                if max_x < self.boxes[pos] {
                    continue; // maxX < nodeMinX
                }
                if max_y < self.boxes[pos + 1] {
                    continue; // maxY < nodeMinY
                }
                if min_x > self.boxes[pos + 2] {
                    continue; // minX > nodeMaxX
                }
                if min_y > self.boxes[pos + 3] {
                    continue; // minY > nodeMaxY
                }

                let index = self.indices.get(pos >> 2);

                if node_index >= self.num_items * 4 {
                    queue.push(index); // node; add it to the search queue
                } else {
                    results.push(index); // leaf item
                }
            }

            outer_node_index = queue.pop();
        }

        results
    }
}

/**
 * Binary search for the first value in the array bigger than the given.
 * @param {number} value
 * @param {number[]} arr
 */
fn upper_bound(value: usize, arr: &[usize]) -> usize {
    let mut i = 0;
    let mut j = arr.len() - 1;

    while i < j {
        let m = (i + j) >> 1;
        if arr[m] > value {
            j = m;
        } else {
            i = m + 1;
        }
    }

    arr[i]
}
