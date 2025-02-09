use std::io::{self, Read};

use crate::u8_mask;

pub trait BitTryReadable: Sized {
    fn try_read<R: BitRead>(reader: &mut R) -> io::Result<Option<Self>>;
}

pub trait BitReadable: Sized {
    fn read<R: BitRead>(reader: &mut R) -> io::Result<Self>;
}

pub trait BitRead: Sized {
    fn try_read_readable<Btr: BitTryReadable>(&mut self) -> io::Result<Option<Btr>> {
        Btr::try_read(self)
    }

    fn read_readable<Br: BitReadable>(&mut self) -> io::Result<Br> {
        Br::read(self)
    }

    fn read_bytes(&mut self, bytes: &mut [u8], last_byte_amount: Option<usize>) -> io::Result<()> {
        if bytes.is_empty() {
            return Ok(());
        }

        for i in 0..bytes.len() - 1 {
            bytes[i] = self.read_byte()?;
        }

        let byte = if let Some(amount) = last_byte_amount {
            self.read_bits(amount)?
        } else {
            self.read_byte()?
        };
        bytes[bytes.len() - 1] = byte;

        Ok(())
    }

    fn read_byte(&mut self) -> io::Result<u8> {
        let Some(byte) = self.try_read_byte()? else {
            return Err(io::ErrorKind::UnexpectedEof.into());
        };
        Ok(byte)
    }

    fn read_bits(&mut self, amount: usize) -> io::Result<u8> {
        let Some(bits) = self.try_read_bits(amount)? else {
            return Err(io::ErrorKind::UnexpectedEof.into());
        };
        Ok(bits)
    }

    fn try_read_byte(&mut self) -> io::Result<Option<u8>>;
    fn try_read_bits(&mut self, amount: usize) -> io::Result<Option<u8>>;
}

pub struct BitReader<R: Read> {
    inner: R,
    bit_buff: Option<u8>,
    bit_cursor: usize,
}

impl<R: Read> BitReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            bit_buff: None,
            bit_cursor: 0,
        }
    }

    pub fn into_inner(self) -> R {
        self.inner
    }

    pub fn bit_cursor(&self) -> usize {
        self.bit_cursor
    }

    fn fill_buff(&mut self) -> io::Result<Option<u8>> {
        if self.bit_buff.is_none() {
            self.bit_buff = try_read_one_byte(&mut self.inner)?;
            if self.bit_buff.is_none() {
                return Ok(None);
            }
        }
        Ok(self.bit_buff)
    }
}

impl<R: Read> BitRead for BitReader<R> {
    fn try_read_byte(&mut self) -> io::Result<Option<u8>> {
        let Some(bit_buff) = self.fill_buff()? else {
            return Ok(None);
        };

        fn extract_part(buff: u8, size: u32, offset: u32) -> u8 {
            let mask = u8_mask(size);
            buff.checked_shr(offset).unwrap_or(0) & mask
        }

        let bottom_size = u8::BITS as usize - self.bit_cursor;
        let mut byte = extract_part(bit_buff, bottom_size as u32, self.bit_cursor as u32);

        self.bit_buff = try_read_one_byte(&mut self.inner)?;

        if bottom_size != u8::BITS as usize {
            if let Some(bit_buff) = self.bit_buff {
                let top_part = extract_part(bit_buff, self.bit_cursor as u32, 0);
                byte |= top_part << bottom_size;
            } else {
                return Ok(None);
            }
        }

        Ok(Some(byte))
    }

    fn try_read_bits(&mut self, amount: usize) -> io::Result<Option<u8>> {
        assert!(amount <= u8::BITS as usize);
        if amount == u8::BITS as usize {
            return self.try_read_byte();
        }

        let Some(bit_buff) = self.fill_buff()? else {
            return Ok(None);
        };

        let bits_remaining = u8::BITS as usize - self.bit_cursor;
        let bottom_size = bits_remaining.min(amount);

        let mask = u8_mask(bottom_size as u32);
        let mut byte = bit_buff.checked_shr(self.bit_cursor as u32).unwrap_or(0) & mask;

        // we don't store the value directly in bit_cursor, because try_read_one_byte can fail, which
        // would let the cursor with a normally impossible value.
        let mut new_bit_cursor = self.bit_cursor + amount;
        if new_bit_cursor >= u8::BITS as usize {
            new_bit_cursor -= u8::BITS as usize;
            self.bit_buff = None;

            if new_bit_cursor > 0 {
                let Some(buf_byte) = try_read_one_byte(&mut self.inner)? else {
                    return Ok(None);
                };
                self.bit_buff = Some(buf_byte);

                let mask = u8_mask(new_bit_cursor as u32);
                byte |= (buf_byte & mask) << bottom_size;
            }
        }

        self.bit_cursor = new_bit_cursor;

        Ok(Some(byte))
    }
}

fn try_read_one_byte<R: Read>(reader: &mut R) -> io::Result<Option<u8>> {
    let mut tmp = [0u8; 1];
    let n = reader.read(&mut tmp)?;
    if n == 0 {
        return Ok(None);
    }

    Ok(Some(tmp[0]))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use super::BitRead;

    #[test]
    #[should_panic]
    fn test_read_bits_on_empty_array_should_fail() {
        crate::test::get_test_read_bytes_output(&[], |tester| {
            tester.read_bits(1)?;

            Ok(())
        })
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn test_read_byte_on_empty_array_should_fail() {
        crate::test::get_test_read_bytes_output(&[], |tester| {
            tester.read_byte()?;

            Ok(())
        })
        .unwrap();
    }

    #[test]
    #[should_panic]
    fn test_read_bytes_on_empty_array_should_fail() {
        crate::test::get_test_read_bytes_output(&[], |tester| {
            tester.try_read_bytes(2, None)?;

            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_read_nothing_empty_array() {
        let test_output = crate::test::get_test_read_bytes_output(&[], |tester| {
            tester.try_read_bytes(0, None)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_nothing() {
        let test_output = crate::test::get_test_read_bytes_output(&[1], |tester| {
            tester.try_read_bytes(0, None)?;
            tester.try_read_bytes(0, None)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_one_byte_aligned() {
        let test_output = crate::test::get_test_read_bytes_output(&[1], |tester| {
            tester.read_byte()?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[1]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_one_byte_not_aligned() {
        let test_output =
            crate::test::get_test_read_bytes_output(&[0b11110000, 0b1111], |tester| {
                tester.read_bits(4)?;
                tester.read_byte()?;

                Ok(())
            })
            .unwrap();

        assert_eq!(&test_output.vec, &[0, 0xFF]);
        assert_eq!(test_output.cursor_position, 4);
    }

    #[test]
    fn test_read_multiple_byte_multiple_calls_aligned() {
        let test_output = crate::test::get_test_read_bytes_output(&[1, 2, 3, 4], |tester| {
            tester.read_byte()?;
            tester.read_byte()?;
            tester.read_byte()?;
            tester.read_byte()?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[1, 2, 3, 4]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_multiple_byte_multiple_calls_not_aligned() {
        let test_output = crate::test::get_test_read_bytes_output(
            &[0b11110000, 0b00111111, 0b01010011, 0b0101],
            |tester| {
                tester.read_bits(4)?;
                tester.read_byte()?;
                tester.read_byte()?;
                tester.read_byte()?;

                Ok(())
            },
        )
        .unwrap();

        assert_eq!(&test_output.vec, &[0, 0xFF, 0x33, 0x55]);
        assert_eq!(test_output.cursor_position, 4);
    }

    #[test]
    fn test_read_multiple_byte_one_call_aligned() {
        let test_output = crate::test::get_test_read_bytes_output(&[1, 2, 3, 4], |tester| {
            tester.try_read_bytes(4, None)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[1, 2, 3, 4]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_multiple_byte_single_call_not_aligned() {
        let test_output = crate::test::get_test_read_bytes_output(
            &[0b11110000, 0b00111111, 0b01010011, 0b0101],
            |tester| {
                tester.read_bits(4)?;
                tester.try_read_bytes(3, None)?;

                Ok(())
            },
        )
        .unwrap();

        assert_eq!(&test_output.vec, &[0, 0xFF, 0x33, 0x55]);
        assert_eq!(test_output.cursor_position, 4);
    }

    #[test]
    fn test_read_bytes_on_one_byte() {
        let test_output = crate::test::get_test_read_bytes_output(
            &[0b11110000],
            |tester| {
                tester.try_read_bytes(1, None)?;

                Ok(())
            },
        )
        .unwrap();

        assert_eq!(&test_output.vec, &[0xF0]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_bytes_on_one_byte_with_amount_specified_is_equivalent_to_read_bits() {
        let test_output = crate::test::get_test_read_bytes_output(
            &[0b11111100],
            |tester| {
                tester.try_read_bytes(1, Some(4))?;

                Ok(())
            },
        )
        .unwrap();
        let test_output2 = crate::test::get_test_read_bytes_output(
            &[0b11111100],
            |tester| {
                tester.read_bits(4)?;

                Ok(())
            },
        )
        .unwrap();

        assert_eq!(&test_output.vec, &test_output2.vec);
        assert_eq!(test_output.cursor_position, 4);
    }

    #[test]
    fn test_read_bytes_last_byte_on_not_aligned() {
        let test_output = crate::test::get_test_read_bytes_output(
            &[0b11000000, 0b10],
            |tester| {
                let _ = tester.read_bits(6);
                tester.try_read_bytes(1, Some(4))?;

                Ok(())
            },
        )
        .unwrap();

        assert_eq!(&test_output.vec, &[0, 0b1011]);
        assert_eq!(test_output.cursor_position, 2);
    }

    #[test]
    #[should_panic]
    fn test_read_bytes_not_enough_bits_for_last_byte() {
        crate::test::get_test_read_bytes_output(
            &[0b11000000],
            |tester| {
                let _ = tester.read_bits(6);
                tester.try_read_bytes(1, Some(3))?;

                Ok(())
            },
        )
        .unwrap();
    }

    #[test]
    fn test_read_zero_bits() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b1], |tester| {
            tester.read_bits(0)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_one_bit() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b1], |tester| {
            tester.read_bits(1)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[1]);
        assert_eq!(test_output.cursor_position, 1);
    }

    #[test]
    fn test_read_multiple_bits_multiple_calls() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b10011], |tester| {
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b1, 0b1, 0b0, 0b0, 0b1]);
        assert_eq!(test_output.cursor_position, 5);
    }

    #[test]
    fn test_read_multiple_bits_multiple_calls_full_byte() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b10010011], |tester| {
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b1, 0b1, 0b0, 0b0, 0b1, 0b0, 0b0, 0b1]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_multiple_bits_multiple_calls_multiple_bytes() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b10010011, 0b111], |tester| {
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;

            tester.read_bits(1)?;
            tester.read_bits(1)?;
            tester.read_bits(1)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(
            &test_output.vec,
            &[0b1, 0b1, 0b0, 0b0, 0b1, 0b0, 0b0, 0b1, 0b1, 0b1, 0b1]
        );
        assert_eq!(test_output.cursor_position, 3);
    }

    #[test]
    fn test_read_multiple_bits_on_call() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b10011], |tester| {
            tester.read_bits(5)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b10011]);
        assert_eq!(test_output.cursor_position, 5);
    }

    #[test]

    fn test_read_multiple_bits_on_call_full_byte() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b10010011], |tester| {
            tester.read_bits(8)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b10010011]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_variable_amount_of_bits() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b10011], |tester| {
            tester.read_bits(3)?;
            tester.read_bits(2)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b011, 0b10]);
        assert_eq!(test_output.cursor_position, 5);
    }

    #[test]
    fn test_read_variable_amount_of_bits_full_byte() {
        let test_output = crate::test::get_test_read_bytes_output(&[0b10010011], |tester| {
            tester.read_bits(3)?;
            tester.read_bits(2)?;
            tester.read_bits(3)?;

            Ok(())
        })
        .unwrap();

        assert_eq!(&test_output.vec, &[0b011, 0b10, 0b100]);
        assert_eq!(test_output.cursor_position, 0);
    }

    #[test]
    fn test_read_variable_amount_of_bits_multiple_bytes_aligned() {
        let test_output =
            crate::test::get_test_read_bytes_output(&[0b10010011, 0b110011], |tester| {
                tester.read_bits(3)?;
                tester.read_bits(2)?;
                tester.read_bits(3)?;

                tester.read_bits(3)?;
                tester.read_bits(3)?;

                Ok(())
            })
            .unwrap();

        assert_eq!(&test_output.vec, &[0b011, 0b10, 0b100, 0b011, 0b110]);
        assert_eq!(test_output.cursor_position, 6);
    }

    #[test]
    fn test_read_variable_amount_of_bits_multiple_bytes_not_aligned() {
        let test_output =
            crate::test::get_test_read_bytes_output(&[0b10010011, 0b110011], |tester| {
                tester.read_bits(5)?;
                tester.read_bits(6)?;

                Ok(())
            })
            .unwrap();

        assert_eq!(&test_output.vec, &[0b10011, 0b011100]);
        assert_eq!(test_output.cursor_position, 3);
    }

    mod fail {
        use crate::BitRead;

        #[test]
        #[should_panic]
        fn read_byte_from_empty_array_should_fail() {
            crate::test::get_test_read_bytes_output(&[], |tester| {
                tester.read_byte()?;

                Ok(())
            })
            .unwrap();
        }

        #[test]
        #[should_panic]
        fn read_bits_from_empty_array_should_fail() {
            crate::test::get_test_read_bytes_output(&[], |tester| {
                tester.read_bits(1)?;

                Ok(())
            })
            .unwrap();
        }

        #[test]
        #[should_panic]
        fn read_bytes_from_empty_array_should_fail() {
            crate::test::get_test_read_bytes_output(&[], |tester| {
                tester.try_read_bytes(1, None)?;

                Ok(())
            })
            .unwrap();
        }

        #[test]
        #[should_panic]
        fn read_byte_from_array_eof_should_fail() {
            crate::test::get_test_read_bytes_output(&[1], |tester| {
                let _ = tester.read_byte();
                tester.read_byte()?;

                Ok(())
            })
            .unwrap();
        }

        #[test]
        #[should_panic]
        fn read_bits_from_array_eof_should_fail() {
            crate::test::get_test_read_bytes_output(&[1], |tester| {
                let _ = tester.read_byte();
                tester.read_bits(1)?;

                Ok(())
            })
            .unwrap();
        }

        #[test]
        #[should_panic]
        fn read_bytes_from_array_shorted_than_asked_should_fail() {
            crate::test::get_test_read_bytes_output(&[1], |tester| {
                tester.try_read_bytes(2, None)?;

                Ok(())
            })
            .unwrap();
        }

        #[test]
        #[should_panic]
        fn read_byte_when_there_is_less_than_an_byte_left_should_fail() {
            crate::test::get_test_read_bytes_output(&[1], |tester| {
                let _ = tester.read_bits(4);
                tester.read_byte()?;

                Ok(())
            })
            .unwrap();
        }

        #[test]
        #[should_panic]
        fn read_bits_when_there_is_less_than_asked_left_should_fail() {
            crate::test::get_test_read_bytes_output(&[1], |tester| {
                let _ = tester.read_bits(4);
                tester.read_bits(5)?;

                Ok(())
            })
            .unwrap();
        }
    }
}
