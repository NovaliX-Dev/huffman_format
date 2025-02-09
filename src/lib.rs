#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use std::io::{self, BufRead, BufReader, Read, Seek, Write};

use bitpack::{compact::CompactNumberU64, BitRead, BitReader, BitWrite, BitWriter};
use log::*;

mod table;
mod tree;
use tree::HeapNode;

struct ByteCounter<W: Write> {
    inner: W,
    byte_count: u64, // TODO: find size of byte count
}

impl<W: Write> ByteCounter<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            byte_count: 0,
        }
    }
}

impl<W: Write> Write for ByteCounter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.byte_count += n as u64;

        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub fn pack_file<R: Read + Seek, W: Write>(reader: R, writer: W) -> io::Result<u64> {
    let mut buf_reader = BufReader::new(reader);
    let mut bit_writer = BitWriter::new(ByteCounter::new(writer));

    info!("Computing byte table...");
    
    let byte_table = table::get_byte_table(&mut buf_reader)?;
    let total_byte_count = byte_table.iter().sum();
    info!("File infos : \n - size : {} bytes\n - entropy : {}", total_byte_count, table::compute_entropy(byte_table));

    info!("Computing huffman tree...");
    let Some((tree_root, code_table)) = tree::get_huffman_tree_and_codes(byte_table) else {
        return Ok(0);
    };
    // dbg!(&tree_root);

    // dbg!(total_byte_count);

    buf_reader.rewind()?;

    info!("Writing file headers...");

    bit_writer.write_writable(tree_root)?;
    bit_writer.write_writable(CompactNumberU64(total_byte_count))?;

    info!("Writing data...");

    loop {
        let buf = buf_reader.fill_buf()?;
        if buf.is_empty() {
            break;
        }

        for byte in buf {
            let code = code_table[*byte as usize].as_ref().unwrap();
            bit_writer.write_writable(code)?;
        }

        let n = buf.len();
        buf_reader.consume(n);
    }

    bit_writer.flush()?;

    Ok(bit_writer.into_inner().byte_count)
}

pub fn unpack_file<R: Read + Seek, W: Write>(reader: R, mut writer: W) -> io::Result<u64> {
    let buf_reader = BufReader::new(reader);
    let mut bit_reader = BitReader::new(buf_reader);

    info!("Reading file headers...");

    let Some(tree_root): Option<HeapNode> = HeapNode::try_read_root(&mut bit_reader)? else {
        return Ok(0);
    };
    // dbg!(&tree_root);

    let CompactNumberU64(total_byte_count) = bit_reader.read_readable()?;
    // dbg!(total_byte_count);

    info!("Reading file data...");

    let mut bytes_read = 0;
    while bytes_read < total_byte_count {
        let mut current_node = &tree_root;

        loop {
            match current_node {
                HeapNode::Leaf(byte) => {
                    writer.write_all(&[*byte])?;
                    bytes_read += 1;

                    break;
                }
                HeapNode::Pair { left, right } => {
                    let child_bit = bit_reader.read_bits(1)?;

                    match child_bit {
                        tree::consts::LEFT_BIT => current_node = left,
                        tree::consts::RIGHT_BIT => current_node = right,

                        _ => unreachable!(),
                    }
                }

                HeapNode::Empty => return Err(io::ErrorKind::InvalidData.into()),
            }
        }
    }

    Ok(bytes_read)
}
