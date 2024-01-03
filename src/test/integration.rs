use std::fs::read;

use bytemuck::cast_slice;

use crate::flatbush::HilbertSort;
use crate::{FlatbushBuilder, FlatbushRef, OwnedFlatbush};

fn create_flatbush_from_data_path(data_path: &str) -> OwnedFlatbush<f64> {
    let buffer = read(data_path).unwrap();
    let boxes_buf: &[f64] = cast_slice(&buffer);

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

fn check_buffer_equality(js_buf: &[u8], rs_buf: &[u8]) {
    // Comment to dig into why buffers are different
    assert_eq!(js_buf, rs_buf);

    assert_eq!(js_buf.len(), rs_buf.len(), "should have same length");

    let header_byte_length = 8;
    assert_eq!(
        js_buf[0..header_byte_length],
        rs_buf[0..header_byte_length],
        "should have same header bytes"
    );

    let js_flatbush = FlatbushRef::<f64>::try_new(&js_buf).unwrap();
    let rs_flatbush = FlatbushRef::<f64>::try_new(&rs_buf).unwrap();

    assert_eq!(js_flatbush.num_items, rs_flatbush.num_items);
    assert_eq!(js_flatbush.node_size, rs_flatbush.node_size);
}

pub(crate) fn flatbush_js_test_data() -> Vec<f64> {
    let buffer = read("fixtures/data1_input.raw").unwrap();
    let boxes_buf: &[f64] = cast_slice(&buffer);
    boxes_buf.to_vec()
}

pub(crate) fn flatbush_js_test_index() -> OwnedFlatbush<f64> {
    create_flatbush_from_data_path("fixtures/data1_input.raw")
}

#[test]
fn test_flatbush_js_test_data() {
    let flatbush_js_buf = read("fixtures/data1_flatbush_js.raw").unwrap();
    let flatbush_rs_buf = create_flatbush_from_data_path("fixtures/data1_input.raw").into_inner();
    check_buffer_equality(&flatbush_js_buf, &flatbush_rs_buf);
}

#[test]
fn test_utah_buildings() {
    let flatbush_js_buf = read("fixtures/utah_flatbush_js.raw").unwrap();
    let flatbush_rs_buf = create_flatbush_from_data_path("fixtures/utah_input.raw").into_inner();
    check_buffer_equality(&flatbush_js_buf, &flatbush_rs_buf);
}
