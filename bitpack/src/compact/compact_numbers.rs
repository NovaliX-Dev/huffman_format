use std::io;

use crate::{BitReadable, BitWritable};

trait NumberInfo {
    fn required_number_of_bytes(&self) -> u8;
}

impl NumberInfo for u64 {
    fn required_number_of_bytes(&self) -> u8 {
        let mut n = 1;
        loop {
            let max = u64::checked_shl(1, n as u32 * u8::BITS)
                .unwrap_or(0)
                .wrapping_sub(1);
            if max >= *self {
                return n;
            }

            n += 1;
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CompactNumberU64(pub u64);

impl BitWritable for CompactNumberU64 {
    fn write<W: crate::BitWrite>(&self, writer: &mut W) -> std::io::Result<()> {
        // TODO : That can be optimized further

        let bytes_required = self.0.required_number_of_bytes();
        writer.write_byte(bytes_required)?;

        let bytes = self.0.to_le_bytes();
        writer.write_bytes(&bytes.as_ref()[..bytes_required as usize], None)?;

        Ok(())
    }
}

impl BitReadable for CompactNumberU64 {
    fn read<R: crate::BitRead>(reader: &mut R) -> std::io::Result<Self> {
        let bytes_required = reader.read_byte()?;

        const MAX_BYTES_REQUIRED: usize = (u64::BITS / u8::BITS) as usize;
        if bytes_required as usize > MAX_BYTES_REQUIRED {
            return Err(io::ErrorKind::InvalidData.into())
        } 

        let mut bytes = [0u8; (u64::BITS / u8::BITS) as usize];
        reader.read_bytes(&mut bytes[..bytes_required as usize], None)?;

        Ok(Self(u64::from_le_bytes(bytes)))
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use crate::{test::TestOutputGeneric, BitWrite};

    use super::CompactNumberU64;

    #[test]
    fn write_u8_number() {
        let output =
            crate::test::get_test_write_output(|writer| writer.write_writable(CompactNumberU64(0)))
                .unwrap();

        assert_eq!(&output.vec, &[1, 0]);

        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64((1 << 8) - 1))
        })
        .unwrap();

        assert_eq!(&output.vec, &[1, 255]);
    }

    #[test]
    fn read_u8_number() {
        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[1, 0]).unwrap();

        assert_eq!(output.result, CompactNumberU64(0));

        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[1, 255]).unwrap();

        assert_eq!(output.result, CompactNumberU64(255));
    }

    #[test]
    fn write_u16_number() {
        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64(1 << 8))
        })
        .unwrap();

        assert_eq!(&output.vec, &[2, 0, 1]);

        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64((1 << 16) - 1))
        })
        .unwrap();

        assert_eq!(&output.vec, &[2, 255, 255]);
    }

    #[test]
    fn read_u16_number() {
        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[2, 0, 1]).unwrap();

        assert_eq!(output.result, CompactNumberU64(1 << 8));

        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[2, 255, 255]).unwrap();

        assert_eq!(output.result, CompactNumberU64((1 << 16) - 1));
    }

    #[test]
    fn write_u24_number() {
        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64(1 << 16))
        })
        .unwrap();

        assert_eq!(&output.vec, &[3, 0, 0, 1]);

        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64((1 << 24) - 1))
        })
        .unwrap();

        assert_eq!(&output.vec, &[3, 255, 255, 255]);
    }

    #[test]
    fn read_u24_number() {
        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[3, 0, 0, 1]).unwrap();

        assert_eq!(output.result, CompactNumberU64(1 << 16));

        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[3, 255, 255, 255]).unwrap();

        assert_eq!(output.result, CompactNumberU64((1 << 24) - 1));
    }

    #[test]
    fn write_u32_number() {
        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64(1 << 24))
        })
        .unwrap();

        assert_eq!(&output.vec, &[4, 0, 0, 0, 1]);

        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64((1 << 32) - 1))
        })
        .unwrap();

        assert_eq!(&output.vec, &[4, 255, 255, 255, 255]);
    }

    #[test]
    fn read_u32_number() {
        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[4, 0, 0, 0, 1]).unwrap();

        assert_eq!(output.result, CompactNumberU64(1 << 24));

        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[4, 255, 255, 255, 255]).unwrap();

        assert_eq!(output.result, CompactNumberU64((1 << 32) - 1));
    }

    #[test]
    fn write_u64_number() {
        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64(1 << 32))
        })
        .unwrap();

        assert_eq!(&output.vec, &[5, 0, 0, 0, 0, 1]);

        let output = crate::test::get_test_write_output(|writer| {
            writer.write_writable(CompactNumberU64(!0))
        })
        .unwrap();

        assert_eq!(&output.vec, &[8, 255, 255, 255, 255, 255, 255, 255, 255]);
    }

    #[test]
    fn read_u64_number() {
        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[5, 0, 0, 0, 0, 1]).unwrap();

        assert_eq!(output.result, CompactNumberU64(1 << 32));

        let output: TestOutputGeneric<CompactNumberU64> =
            crate::test::get_test_read_readable_output(&[
                8, 255, 255, 255, 255, 255, 255, 255, 255,
            ])
            .unwrap();

        assert_eq!(output.result, CompactNumberU64(!0));
    }

    mod malformed {
        use crate::{compact::CompactNumberU64, test::TestOutputGeneric};

        #[test]
        #[should_panic]
        fn size_is_bigger_than_the_maximum_amount_of_bytes() {
            let _: TestOutputGeneric<CompactNumberU64> =
                crate::test::get_test_read_readable_output(&[9, 0, 0, 0, 0, 1]).unwrap();
        }
    }
}
