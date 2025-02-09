#![no_main]

use std::io::{Cursor, Seek};

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let buff = Vec::<u8>::with_capacity(data.len());
    let mut cursor = Cursor::new(buff);

    huffman_format::pack_file(Cursor::new(data), &mut cursor).unwrap();

    let buff = Vec::<u8>::with_capacity(data.len());
    let mut output_cursor = Cursor::new(buff);

    cursor.rewind().unwrap();
    huffman_format::unpack_file(&mut cursor, &mut output_cursor).unwrap();

    assert_eq!(data, output_cursor.get_ref())
});
