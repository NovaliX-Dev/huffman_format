#![no_main]

use std::io::Cursor;

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let buff = Vec::<u8>::with_capacity(data.len());
    let mut cursor = Cursor::new(buff);

    let _ = huffman_format::unpack_file(Cursor::new(data), &mut cursor);
});
