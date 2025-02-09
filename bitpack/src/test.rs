use std::io::{self, Cursor};

use crate::{BitRead, BitReadable, BitReader, BitWrite, BitWriter};

pub struct TestOutput {
    pub vec: Vec<u8>,
    pub cursor_position: usize,
}

pub fn get_test_write_output<F: FnOnce(&mut BitWriter<Cursor<Vec<u8>>>) -> io::Result<()>>(
    function: F,
) -> io::Result<TestOutput> {
    let bytes = Vec::<u8>::new();
    let cursor = Cursor::new(bytes);
    let mut bit_writer = BitWriter::new(cursor);

    function(&mut bit_writer)?;

    let cursor_position = bit_writer.bit_cursor();

    bit_writer.flush().unwrap();
    let final_vec = bit_writer.into_inner().into_inner();

    Ok(TestOutput {
        vec: final_vec,
        cursor_position,
    })
}

pub struct TestBitReader<Br: BitRead> {
    inner: Br,
    bits_read: Vec<u8>,
}

impl<Br: BitRead> TestBitReader<Br> {
    fn new(inner: Br) -> Self {
        Self {
            inner,
            bits_read: Vec::new(),
        }
    }

    pub fn try_read_bytes(
        &mut self,
        byte_amount: usize,
        last_byte_amount: Option<usize>,
    ) -> std::io::Result<()> {
        let mut vec = vec![0; byte_amount];

        self.inner.read_bytes(&mut vec, last_byte_amount)?;

        self.bits_read.extend(vec);

        Ok(())
    }
}

impl<Br: BitRead> BitRead for TestBitReader<Br> {
    fn try_read_byte(&mut self) -> std::io::Result<Option<u8>> {
        let Some(byte) = self.inner.try_read_byte()? else {
            return Ok(None);
        };

        self.bits_read.push(byte);

        Ok(Some(0))
    }

    fn try_read_bits(&mut self, amount: usize) -> std::io::Result<Option<u8>> {
        let Some(byte) = self.inner.try_read_bits(amount)? else {
            return Ok(None);
        };

        self.bits_read.push(byte);

        Ok(Some(0))
    }
}

pub fn get_test_read_bytes_output<
    F: FnOnce(&mut TestBitReader<BitReader<Cursor<&[u8]>>>) -> io::Result<()>,
>(
    input_bytes: &[u8],
    test: F,
) -> io::Result<TestOutput> {
    let cursor = Cursor::new(input_bytes);
    let bit_reader = BitReader::new(cursor);
    let mut test_bit_reader = TestBitReader::new(bit_reader);

    test(&mut test_bit_reader)?;

    let test_output = TestOutput {
        vec: test_bit_reader.bits_read,
        cursor_position: test_bit_reader.inner.bit_cursor(),
    };

    Ok(test_output)
}

pub struct TestOutputGeneric<R> {
    pub result: R,
    pub cursor_position: usize,
}

pub fn get_test_read_readable_output<R: BitReadable>(
    input_bytes: &[u8],
) -> io::Result<TestOutputGeneric<R>> {
    let cursor = Cursor::new(input_bytes);
    let mut bit_reader = BitReader::new(cursor);

    let result = bit_reader.read_readable()?;

    let test_output = TestOutputGeneric {
        result,
        cursor_position: bit_reader.bit_cursor(),
    };

    Ok(test_output)
}

pub fn get_test_read_custom_readable_output<
    'l,
    R,
    F: FnOnce(&mut BitReader<Cursor<&'l [u8]>>) -> io::Result<R>,
>(
    input_bytes: &'l [u8],
    read_function: F,
) -> io::Result<TestOutputGeneric<R>> {
    let cursor = Cursor::new(input_bytes);
    let mut bit_reader = BitReader::new(cursor);

    let result = read_function(&mut bit_reader)?;

    let test_output = TestOutputGeneric {
        result,
        cursor_position: bit_reader.bit_cursor(),
    };

    Ok(test_output)
}
