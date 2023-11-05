use std::fs::read;

use bytemuck::cast_slice;

use crate::{FlatbushBuilder, OwnedFlatbush};

fn create_flatbush_from_data_path(data_path: &str) -> OwnedFlatbush {
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
    builder.finish()
}

fn check_buffer_equality(js_buf: &[u8], rs_buf: &[u8]) {
    assert_eq!(js_buf.len(), rs_buf.len(), "should have same length");

    let header_byte_length = 8;
    assert_eq!(
        js_buf[0..header_byte_length],
        rs_buf[0..header_byte_length],
        "should have same header bytes"
    );
}

#[test]
fn test() {
    let flatbush_js_buf = read("fixtures/data1_flatbush_js.raw").unwrap();
    let flatbush_rs_buf = create_flatbush_from_data_path("fixtures/data1_input.raw").into_inner();
    check_buffer_equality(&flatbush_js_buf, &flatbush_rs_buf);

    dbg!("hi");
}