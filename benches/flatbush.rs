use bytemuck::cast_slice;
use criterion::{criterion_group, criterion_main, Criterion};
use flatbush::flatbush::HilbertSort;
use flatbush::{FlatbushBuilder, FlatbushIndex, OwnedFlatbush};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{RTree, AABB};
use std::fs::read;

fn load_data() -> Vec<f64> {
    let buf = read("benches/bounds.raw").unwrap();
    cast_slice(&buf).to_vec()
}

fn construct_flatbush(boxes_buf: &[f64]) -> OwnedFlatbush {
    let mut builder = FlatbushBuilder::new(boxes_buf.len() / 4);
    for box_ in boxes_buf.chunks(4) {
        let min_x = box_[0];
        let min_y = box_[1];
        let max_x = box_[2];
        let max_y = box_[3];
        builder.add(min_x, min_y, max_x, max_y);
    }
    builder.finish::<HilbertSort>()
}

fn construct_rstar(
    rect_vec: Vec<GeomWithData<Rectangle<(f64, f64)>, usize>>,
) -> RTree<GeomWithData<Rectangle<(f64, f64)>, usize>> {
    RTree::bulk_load(rect_vec)
}

pub fn criterion_benchmark(c: &mut Criterion) {
    let boxes_buf = load_data();
    let aabb_vec: Vec<AABB<(f64, f64)>> = boxes_buf
        .chunks(4)
        .map(|box_| AABB::from_corners((box_[0], box_[1]), (box_[2], box_[3])))
        .collect();
    let rect_vec: Vec<GeomWithData<Rectangle<_>, usize>> = aabb_vec
        .into_iter()
        .enumerate()
        .map(|(idx, aabb)| GeomWithData::new(aabb.into(), idx))
        .collect();

    c.bench_function("construction (flatbush)", |b| {
        b.iter(|| construct_flatbush(&boxes_buf))
    });

    c.bench_function("construction (rstar bulk)", |b| {
        b.iter(|| construct_rstar(rect_vec.to_vec()))
    });

    let flatbush_tree = construct_flatbush(&boxes_buf);
    let rstar_tree = construct_rstar(rect_vec.to_vec());
    let (min_x, min_y, max_x, max_y) = (-112.007493, 40.633799, -111.920964, 40.694228);

    let flatbush_search_results = flatbush_tree.search(min_x, min_y, max_x, max_y);
    let rstar_search_results = {
        let aabb = AABB::from_corners((min_x, min_y), (max_x, max_y));
        rstar_tree
            .locate_in_envelope_intersecting(&aabb)
            .collect::<Vec<_>>()
    };

    assert_eq!(flatbush_search_results.len(), rstar_search_results.len());
    println!(
        "search() results in {} items",
        flatbush_search_results.len()
    );

    c.bench_function("search (flatbush)", |b| {
        b.iter(|| flatbush_tree.search(min_x, min_y, max_x, max_y))
    });

    c.bench_function("search (rstar)", |b| {
        b.iter(|| {
            let aabb = AABB::from_corners((min_x, min_y), (max_x, max_y));
            rstar_tree
                .locate_in_envelope_intersecting(&aabb)
                .collect::<Vec<_>>()
        })
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
