use crate::r#type::IndexableNum;
use crate::FlatbushIndex;
use core::mem::take;
use std::marker::PhantomData;

#[derive(Clone)]
pub struct Node<'a, N: IndexableNum, T: FlatbushIndex<N>> {
    tree: &'a T,
    index: usize,
    phantom: PhantomData<N>,
}

impl<'a, N: IndexableNum, T: FlatbushIndex<N>> Node<'a, N, T> {
    pub fn new(tree: &'a T, index: usize) -> Self {
        Self {
            tree,
            index,
            phantom: PhantomData,
        }
    }

    pub fn from_root(tree: &'a T) -> Self {
        let root_index = tree.boxes().len() - 4;
        Self {
            tree,
            index: root_index,
            phantom: PhantomData,
        }
    }

    pub fn min_x(&self) -> N {
        self.tree.boxes()[self.index]
    }

    pub fn min_y(&self) -> N {
        self.tree.boxes()[self.index + 1]
    }

    pub fn max_x(&self) -> N {
        self.tree.boxes()[self.index + 2]
    }

    pub fn max_y(&self) -> N {
        self.tree.boxes()[self.index + 3]
    }

    pub fn is_leaf(&self) -> bool {
        self.index >= self.tree.num_items() * 4
    }

    pub fn is_parent(&self) -> bool {
        !self.is_leaf()
    }

    pub fn intersects<T2: FlatbushIndex<N>>(&self, other: &Node<N, T2>) -> bool {
        if self.max_x() < other.min_x() {
            return false;
        }

        if self.max_y() < other.min_y() {
            return false;
        }

        if self.min_x() > other.max_x() {
            return false;
        }

        if self.min_y() > other.max_y() {
            return false;
        }

        true
    }

    pub fn children(&self) -> impl Iterator<Item = Node<'_, N, T>> {
        // find the end index of the node
        let end = (self.index + self.tree.node_size() * 4)
            .min(upper_bound(self.index, self.tree.level_bounds()));

        // yield child nodes
        (self.index..end)
            .step_by(4)
            .map(|pos| Node::new(self.tree, pos))
    }
}

// This is copied from rstar under the MIT/Apache 2 license
// https://github.com/georust/rstar/blob/6c23af0f3acc0c4668ce6c368820e0fa986a65b4/rstar/src/algorithm/intersection_iterator.rs
pub struct IntersectionIterator<'a, N, T1, T2>
where
    N: IndexableNum,
    T1: FlatbushIndex<N>,
    T2: FlatbushIndex<N>,
{
    left: &'a T1,
    right: &'a T2,
    todo_list: Vec<(usize, usize)>,
    candidates: Vec<usize>,
    phantom: PhantomData<N>,
}

impl<'a, N, T1, T2> IntersectionIterator<'a, N, T1, T2>
where
    N: IndexableNum,
    T1: FlatbushIndex<N>,
    T2: FlatbushIndex<N>,
{
    pub(crate) fn from_trees(root1: &'a T1, root2: &'a T2) -> Self {
        let mut intersections = IntersectionIterator {
            left: root1,
            right: root2,
            todo_list: Vec::new(),
            candidates: Vec::new(),
            phantom: PhantomData,
        };
        intersections.add_intersecting_children(&root1.root(), &root2.root());
        intersections
    }

    #[allow(dead_code)]
    pub(crate) fn new(root1: &'a Node<N, T1>, root2: &'a Node<N, T2>) -> Self {
        let mut intersections = IntersectionIterator {
            left: root1.tree,
            right: root2.tree,
            todo_list: Vec::new(),
            candidates: Vec::new(),
            phantom: PhantomData,
        };
        intersections.add_intersecting_children(root1, root2);
        intersections
    }

    fn push_if_intersecting(&mut self, node1: &'_ Node<N, T1>, node2: &'_ Node<N, T2>) {
        if node1.intersects(node2) {
            self.todo_list.push((node1.index, node2.index));
        }
    }

    fn add_intersecting_children(&mut self, parent1: &'_ Node<N, T1>, parent2: &'_ Node<N, T2>) {
        if !parent1.intersects(parent2) {
            return;
        }

        let children1 = parent1.children().filter(|c1| c1.intersects(parent2));

        let mut children2 = take(&mut self.candidates);
        children2.extend(
            parent2
                .children()
                .filter(|c2| c2.intersects(parent1))
                .map(|c| c.index),
        );

        for child1 in children1 {
            for child2 in &children2 {
                self.push_if_intersecting(&child1, &Node::new(self.right, *child2));
            }
        }

        children2.clear();
        self.candidates = children2;
    }
}

impl<'a, N, T1, T2> Iterator for IntersectionIterator<'a, N, T1, T2>
where
    N: IndexableNum,
    T1: FlatbushIndex<N>,
    T2: FlatbushIndex<N>,
{
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((left_index, right_index)) = self.todo_list.pop() {
            let left = Node::new(self.left, left_index);
            let right = Node::new(self.right, right_index);
            match (left.is_leaf(), right.is_leaf()) {
                (true, true) => return Some((left.index, right.index)),
                (true, false) => right
                    .children()
                    .for_each(|c| self.push_if_intersecting(&left, &c)),
                (false, true) => left
                    .children()
                    .for_each(|c| self.push_if_intersecting(&c, &right)),
                (false, false) => self.add_intersecting_children(&left, &right),
            }
        }
        None
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
