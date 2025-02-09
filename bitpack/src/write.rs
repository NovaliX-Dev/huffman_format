use std::io::{self, Write};

use super::u8_mask;

pub trait BitWritable {
    fn write<W: BitWrite>(&self, writer: &mut W) -> io::Result<()>;
}

impl<Bw: BitWritable> BitWritable for &Bw {
    fn write<W: BitWrite>(&self, writer: &mut W) -> io::Result<()> {
        (*self).write(writer)
    }
}

pub trait BitWrite: Sized {
    fn write_writable<Bw: BitWritable>(&mut self, writable: Bw) -> io::Result<()> {
        writable.write(self)
    }

    fn write_bytes(&mut self, bytes: &[u8], last_byte_amount: Option<usize>) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }

        for byte in &bytes[..bytes.len() - 1] {
            self.write_byte(*byte)?;
        }

        if let Some(amount) = last_byte_amount {
            self.write_bits(bytes[bytes.len() - 1], amount)?;
        } else {
            self.write_byte(bytes[bytes.len() - 1])?;
        }

        Ok(())
    }

    fn write_bits(&mut self, bits: u8, amount: usize) -> io::Result<()>;
    fn write_byte(&mut self, byte: u8) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
}

pub struct BitWriter<W: Write> {
    inner: W,
    bit_buff: u8,
    bit_cursor: usize,
}

impl<W: Write> BitWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            bit_buff: 0,
            bit_cursor: 0,
        }
    }

    pub fn into_inner(self) -> W {
        self.inner
    }

    pub fn bit_cursor(&self) -> usize {
        self.bit_cursor
    }
}

impl<W: Write> BitWrite for BitWriter<W> {
    fn write_byte(&mut self, byte: u8) -> io::Result<()> {
        let bits_to_consume = u8::BITS as usize - self.bit_cursor;

        let mask = u8_mask(bits_to_consume as u32);
        let byte_to_send = self.bit_buff | (byte & mask) << self.bit_cursor;

        self.inner.write_all(&[byte_to_send])?;

        self.bit_buff = byte.checked_shr(bits_to_consume as u32).unwrap_or(0);

        Ok(())
    }

    fn write_bits(&mut self, bits: u8, amount: usize) -> io::Result<()> {
        assert!(amount <= u8::BITS as usize);

        if amount == u8::BITS as usize {
            return self.write_byte(bits);
        }

        let remaining_bits = u8::BITS as usize - self.bit_cursor;
        let bits_to_consume = remaining_bits.min(amount);

        let mask = u8_mask(bits_to_consume as u32);
        self.bit_buff |= (bits & mask) << self.bit_cursor;

        // we don't store the value directly in bit_cursor, because try_read_one_byte can fail, which
        // would let the cursor with a normally impossible value.
        let mut new_bit_cursor = self.bit_cursor + amount;
        if new_bit_cursor >= u8::BITS as usize {
            self.inner.write_all(&[self.bit_buff])?;

            new_bit_cursor -= u8::BITS as usize;

            let mask = u8_mask(new_bit_cursor as u32);
            self.bit_buff = bits.checked_shr(bits_to_consume as u32).unwrap_or(0) & mask;
        }

        self.bit_cursor = new_bit_cursor;

        Ok(())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.bit_cursor != 0 {
            self.inner.write_all(&[self.bit_buff])?;
            self.bit_buff = 0;
            self.bit_cursor = 0;
        }

        Ok(())
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use super::BitWrite;

    #[test]
    fn test_write_bytes_empty_array() {
        let test_output = crate::test::get_test_write_output(|bit_writer| {
            bit_writer.write_bytes(&[], None)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[]);
    }

    #[test]
    fn test_write_one_byte_aligned() {
        let test_output = crate::test::get_test_write_output(|bit_writer| {
            bit_writer.write_byte(5)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[5]);
    }

    #[test]
    fn test_write_multiples_bytes_byte_by_byte_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_byte(1)?;
            writer.write_byte(2)?;
            writer.write_byte(3)?;
            writer.write_byte(4)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[1, 2, 3, 4])
    }

    #[test]
    fn test_write_multiples_bytes_byte_by_byte_not_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0, 4)?;

            writer.write_byte(0b00001111)?;
            writer.write_byte(0b00110011)?;
            writer.write_byte(0b11001100)?;
            writer.write_byte(0b10101010)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(
            &test_output.vec,
            &[0b11110000, 0b00110000, 0b11000011, 0b10101100, 0b1010]
        );
        assert_eq!(test_output.cursor_position, 4)
    }

    #[test]
    fn test_write_multiples_bytes_one_call_aligned() {
        let test_output =
            crate::test::get_test_write_output(|writer| writer.write_bytes(&[1, 2, 3, 4], None))
                .unwrap();

        assert_eq!(&test_output.vec, &[1, 2, 3, 4])
    }

    #[test]
    fn test_write_bits_write_no_bits() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0, 0)?;
            writer.write_bits(0, 0)?;
            writer.write_bits(0, 0)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_write_bits_write_no_bits_on_end_of_byte() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1010101, 7)?;
            writer.write_bits(0, 0)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b1010101]);
        assert_eq!(test_output.cursor_position, 7);
    }

    #[test]
    fn test_write_bits_write_no_bits_on_start_of_new_byte() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b00001111, 8)?;
            writer.write_bits(0, 0)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b00001111]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_write_one_bit() {
        let test_output =
            crate::test::get_test_write_output(|writer| writer.write_bits(0b1, 1)).unwrap();

        assert_eq!(&test_output.vec, &[0b1]);
        assert_eq!(test_output.cursor_position, 1);
    }

    #[test]
    fn test_write_multiple_bits() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b10101]);
        assert_eq!(test_output.cursor_position, 5);
    }

    #[test]
    fn test_write_multiple_bits_variable_amount() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b101, 3)?;
            writer.write_bits(0b01, 2)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b01101]);
        assert_eq!(test_output.cursor_position, 5);
    }

    #[test]
    fn test_write_multiple_bits_full_byte() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b01010101]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_write_multiple_bits_variable_amount_full_byte() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b101, 3)?;
            writer.write_bits(0b01, 2)?;
            writer.write_bits(0b001, 3)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b00101101]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_write_multiple_bits_multiple_bytes_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;
            writer.write_bits(0b0, 1)?;

            writer.write_bits(0b0, 1)?;
            writer.write_bits(0b1, 1)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b01010101, 0b10]);
        assert_eq!(test_output.cursor_position, 2);
    }

    #[test]
    fn test_write_multiple_bits_variable_amount_multiple_byte_not_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b111, 3)?;
            writer.write_bits(0b00, 2)?;
            writer.write_bits(0b1111, 4)?;
            writer.write_bits(0b000, 3)?;
            writer.write_bits(0b11, 2)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b11100111, 0b110001]);
        assert_eq!(test_output.cursor_position, 6);
    }

    #[test]
    fn test_write_multiple_bits_variable_amount_multiple_byte_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b101, 3)?;
            writer.write_bits(0b01, 2)?;
            writer.write_bits(0b001, 3)?;

            writer.write_bits(0b011, 3)?;
            writer.write_bits(0b01, 2)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b00101101, 0b01011]);
        assert_eq!(test_output.cursor_position, 5);
    }

    #[test]
    fn test_write_fewer_bits_than_value() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1101, 3)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b101]);
        assert_eq!(test_output.cursor_position, 3);
    }

    #[test]
    fn test_write_fewer_bits_than_value_multiple_times() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1101, 3)?;
            writer.write_bits(0b1101, 3)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b101101]);
        assert_eq!(test_output.cursor_position, 6);
    }

    #[test]
    fn test_write_fewer_bits_than_value_fill_byte() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1101, 3)?;
            writer.write_bits(0b1101, 3)?;
            writer.write_bits(0b1101, 2)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b01101101]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_write_fewer_bits_than_value_multiple_bytes_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1101, 3)?;
            writer.write_bits(0b1101, 3)?;
            writer.write_bits(0b1101, 2)?;

            writer.write_bits(0b1100, 3)?;
            writer.write_bits(0b1101, 2)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b01101101, 0b01100]);
        assert_eq!(test_output.cursor_position, 5);
    }

    #[test]
    fn test_write_fewer_bits_than_value_multiple_bytes_not_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b1101, 3)?;
            writer.write_bits(0b1101, 3)?;
            writer.write_bits(0b11101, 4)?;

            writer.write_bits(0b1100, 3)?;
            writer.write_bits(0b1101, 2)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b01101101, 0b0110011]);
        assert_eq!(test_output.cursor_position, 7);
    }

    #[test]
    fn test_write_bytes_last_byte_not_full_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bytes(&[0b00011100, 0b011], Some(3)).unwrap();

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b00011100, 0b011]);
        assert_eq!(test_output.cursor_position, 3);
    }

    #[test]
    fn test_write_bytes_last_byte_not_full_not_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bits(0b0, 4)?;
            writer.write_bytes(&[0b00011100, 0b011], Some(3)).unwrap();

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b11000000, 0b0110001]);
        assert_eq!(test_output.cursor_position, 7);
    }

    #[test]
    fn test_write_bytes_aligned() {
        let test_output = crate::test::get_test_write_output(|writer| {
            writer.write_bytes(&[0b00011100, 0b011], None).unwrap();

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b00011100, 0b011]);
        assert_eq!(test_output.cursor_position, 0);
    }
}
