use std::borrow::Cow;

use bytemuck::cast_slice;

use crate::flatbush::index::{FlatbushRef, OwnedFlatbush};
use crate::flatbush::traversal::{IntersectionIterator, Node};
use crate::indices::Indices;
use crate::r#type::IndexableNum;

pub trait FlatbushIndex<N: IndexableNum>: Sized {
    fn boxes(&self) -> &[N];
    fn indices(&self) -> Cow<'_, Indices>;
    fn num_items(&self) -> usize;
    fn num_nodes(&self) -> usize;
    fn node_size(&self) -> usize;
    fn level_bounds(&self) -> &[usize];

    fn search(&self, min_x: N, min_y: N, max_x: N, max_y: N) -> Vec<usize> {
        let boxes = self.boxes();
        let indices = self.indices();

        let mut outer_node_index = Some(boxes.len() - 4);

        let mut queue = vec![];
        let mut results = vec![];

        while let Some(node_index) = outer_node_index {
            // find the end index of the node
            let end = (node_index + self.node_size() * 4)
                .min(upper_bound(node_index, self.level_bounds()));

            // search through child nodes
            for pos in (node_index..end).step_by(4) {
                // check if node bbox intersects with query bbox
                if max_x < boxes[pos] {
                    continue; // maxX < nodeMinX
                }
                if max_y < boxes[pos + 1] {
                    continue; // maxY < nodeMinY
                }
                if min_x > boxes[pos + 2] {
                    continue; // minX > nodeMaxX
                }
                if min_y > boxes[pos + 3] {
                    continue; // minY > nodeMaxY
                }

                let index = indices.get(pos >> 2);

                if node_index >= self.num_items() * 4 {
                    queue.push(index); // node; add it to the search queue
                } else {
                    results.push(index); // leaf item
                }
            }

            outer_node_index = queue.pop();
        }

        results
    }

    #[allow(unused_mut, unused_labels, unused_variables)]
    fn neighbors(&self, x: N, y: N, max_distance: Option<N>) -> Vec<usize> {
        let boxes = self.boxes();
        let indices = self.indices();
        let max_distance = max_distance.unwrap_or(N::max_value());

        let mut outer_node_index = Some(boxes.len() - 4);

        let mut results = vec![];
        let max_dist_squared = max_distance * max_distance;

        'outer: while let Some(node_index) = outer_node_index {
            // find the end index of the node
            let end = (node_index + self.node_size() * 4)
                .min(upper_bound(node_index, self.level_bounds()));

            // add child nodes to the queue
            for pos in (node_index..end).step_by(4) {
                let index = indices.get(pos >> 2);

                let dx = axis_dist(x, boxes[pos], boxes[pos + 2]);
                let dy = axis_dist(y, boxes[pos + 1], boxes[pos + 3]);
                let dist = dx * dx + dy * dy;
                if dist > max_dist_squared {
                    continue;
                }
            }

            // break 'outer;
        }

        results
    }

    fn intersection_candidates_with_other_tree<'a>(
        &'a self,
        other: &'a impl FlatbushIndex<N>,
    ) -> impl Iterator<Item = (usize, usize)> + 'a {
        IntersectionIterator::from_trees(self, other)
    }

    fn root(&self) -> Node<'_, N, Self> {
        Node::from_root(self)
    }
}

impl<N: IndexableNum> FlatbushIndex<N> for OwnedFlatbush<N> {
    fn boxes(&self) -> &[N] {
        let data = &self.buffer;

        let nodes_byte_length = self.num_nodes * 4 * N::BYTES_PER_ELEMENT;
        cast_slice(&data[8..8 + nodes_byte_length])
    }

    fn indices(&self) -> Cow<'_, Indices> {
        let data = &self.buffer;

        let indices_bytes_per_element = if self.num_nodes < 16384 { 2 } else { 4 };
        let nodes_byte_length = self.num_nodes * 4 * N::BYTES_PER_ELEMENT;
        let indices_byte_length = self.num_nodes * indices_bytes_per_element;
        let indices_buf = &data[8 + nodes_byte_length..8 + nodes_byte_length + indices_byte_length];

        Cow::Owned(Indices::new(indices_buf, self.num_nodes()))
    }

    fn num_nodes(&self) -> usize {
        self.num_nodes
    }

    fn level_bounds(&self) -> &[usize] {
        &self.level_bounds
    }

    fn node_size(&self) -> usize {
        self.node_size
    }

    fn num_items(&self) -> usize {
        self.num_items
    }
}

impl<N: IndexableNum> FlatbushIndex<N> for FlatbushRef<'_, N> {
    fn boxes(&self) -> &[N] {
        self.boxes
    }

    fn indices(&self) -> Cow<'_, Indices> {
        Cow::Borrowed(&self.indices)
    }

    fn level_bounds(&self) -> &[usize] {
        &self.level_bounds
    }

    fn node_size(&self) -> usize {
        self.node_size
    }

    fn num_items(&self) -> usize {
        self.num_items
    }

    fn num_nodes(&self) -> usize {
        self.num_nodes
    }
}

/**
 * Binary search for the first value in the array bigger than the given.
 * @param {number} value
 * @param {number[]} arr
 */
#[inline]
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

/**
 * 1D distance from a value to a range.
 * @param {number} k
 * @param {number} min
 * @param {number} max
 */
#[inline]
fn axis_dist<N: IndexableNum>(k: N, min: N, max: N) -> N {
    if k < min {
        min - k
    } else if k <= max {
        N::zero()
    } else {
        k - max
    }
}

#[cfg(test)]
mod test {
    // Replication of tests from flatbush js
    mod js {
        use crate::test::{flatbush_js_test_data, flatbush_js_test_index};
        use crate::FlatbushIndex;

        #[test]
        fn performs_bbox_search() {
            let data = flatbush_js_test_data();
            let index = flatbush_js_test_index();
            let ids = index.search(40., 40., 60., 60.);

            let mut results: Vec<usize> = vec![];
            for id in ids {
                results.push(data[4 * id] as usize);
                results.push(data[4 * id + 1] as usize);
                results.push(data[4 * id + 2] as usize);
                results.push(data[4 * id + 3] as usize);
            }

            results.sort();

            let mut expected = vec![
                57, 59, 58, 59, 48, 53, 52, 56, 40, 42, 43, 43, 43, 41, 47, 43,
            ];
            expected.sort();

            assert_eq!(results, expected);
        }
    }
}
