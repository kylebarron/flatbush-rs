use crate::indices::MutableIndices;
use crate::r#type::IndexableNum;

pub struct SortParams<N: IndexableNum> {
    pub(crate) num_items: usize,
    pub(crate) node_size: usize,
    pub(crate) min_x: N,
    pub(crate) min_y: N,
    pub(crate) max_x: N,
    pub(crate) max_y: N,
}

pub trait Sort<N: IndexableNum> {
    fn sort(sort_params: &mut SortParams<N>, boxes: &mut [N], indices: &mut MutableIndices);
}
