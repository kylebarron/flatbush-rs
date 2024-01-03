use float_next_after::NextAfter;

/// calculate the total number of nodes in the R-tree to allocate space for
/// and the index of each tree level (used in search later)
pub fn compute_num_nodes(num_items: usize, node_size: usize) -> (usize, Vec<usize>) {
    let mut n = num_items;
    let mut num_nodes = n;
    let mut level_bounds = vec![n * 4];
    while n != 1 {
        n = (n as f64 / node_size as f64).ceil() as usize;
        num_nodes += n;
        level_bounds.push(num_nodes * 4);
    }
    (num_nodes, level_bounds)
}

#[inline]
pub fn f64_box_to_f32(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> (f32, f32, f32, f32) {
    let new_min_x = (min_x as f32).next_after(f32::NEG_INFINITY);
    let new_min_y = (min_y as f32).next_after(f32::NEG_INFINITY);
    let new_max_x = (max_x as f32).next_after(f32::INFINITY);
    let new_max_y = (max_y as f32).next_after(f32::INFINITY);

    debug_assert!((new_min_x as f64) <= min_x);
    debug_assert!((new_min_y as f64) <= min_y);
    debug_assert!((new_max_x as f64) >= max_x);
    debug_assert!((new_max_y as f64) >= max_y);

    (new_min_x, new_min_y, new_max_x, new_max_y)
}

#[cfg(test)]
mod test {
    use crate::flatbush::util::f64_box_to_f32;

    #[test]
    fn test_f32_box() {
        let min_x = 1.2f64;
        let min_y = 1.3f64;
        let max_x = 2.4f64;
        let max_y = 2.5f64;
        let _new_box = f64_box_to_f32(min_x, min_y, max_x, max_y);
    }
}
