use std::io::{self, BufRead};

pub const BYTE_TABLE_LEN: usize = u8::MAX as usize + 1;

pub type ByteTable = [u64; BYTE_TABLE_LEN];

pub fn get_byte_table<R: BufRead>(reader: &mut R) -> io::Result<ByteTable> {
    let mut byte_table = [0; BYTE_TABLE_LEN];

    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            break;
        }

        for byte in buf {
            byte_table[*byte as usize] += 1;
        }

        let n = buf.len();
        reader.consume(n);
    }

    Ok(byte_table)
}

pub fn compute_entropy(table: ByteTable) -> f32 {
    let total_count: u64 = table.iter().sum();

    let entropy: f32 = table.into_iter()
        .filter(|count| *count > 0)
        .map(|count| {
            let p = count as f32 / total_count as f32;
            p * p.log2()
        })
        .sum();

    -entropy
}
