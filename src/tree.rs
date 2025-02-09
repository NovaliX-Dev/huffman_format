use std::io;

use crate::table::{ByteTable, BYTE_TABLE_LEN};

use bitpack::{BitRead, BitReadable, BitWritable};
use consts::{LEAF_FLAG, PAIR_FLAG};

pub type HuffmanCodeTable = [Option<HuffmanCode>; BYTE_TABLE_LEN];

#[derive(Debug, PartialEq, Eq)]
pub struct HuffmanCode(Vec<u8>, usize);

impl BitWritable for HuffmanCode {
    fn write<W: bitpack::BitWrite>(&self, writer: &mut W) -> io::Result<()> {
        if self.0.is_empty() {
            return Ok(())
        }

        writer.write_bits(self.0[0], self.1)?;
        for byte in &self.0[1..] {
            writer.write_byte(*byte)?;
        }

        Ok(())
    }
}

struct HuffmanCodeBuilder {
    bit_cursor: usize,
    bytes: Vec<u8>,
    bit_buff: u8,
}

impl HuffmanCodeBuilder {
    fn new() -> Self {
        Self {
            bytes: Vec::new(),
            bit_cursor: 0,
            bit_buff: 0,
        }
    }

    fn write_bit(&mut self, bit: u8) {
        assert!(bit <= 1);

        if self.bit_cursor == u8::BITS as usize {
            self.bytes.push(self.bit_buff);
            self.bit_buff = 0;
            self.bit_cursor = 0;
        }

        self.bit_buff <<= 1;
        self.bit_buff |= bit;
        self.bit_cursor += 1;
    }

    fn finish(mut self) -> HuffmanCode {
        if self.bit_cursor != 0 {
            self.bytes.push(self.bit_buff);
        }

        self.bytes.reverse();

        HuffmanCode(self.bytes, self.bit_cursor)
    }
}

pub mod consts {
    pub const LEAF_FLAG: u8 = 0b0;
    pub const PAIR_FLAG: u8 = 0b1;

    pub const TYPE_FLAG_SIZE: usize = 1;

    pub const LEFT_BIT: u8 = 0b0;
    pub const RIGHT_BIT: u8 = 0b1;
}

#[derive(Debug, PartialEq, Eq)]
pub enum HeapNode {
    Leaf(u8),
    Pair {
        left: Box<HeapNode>,
        right: Box<HeapNode>,
    },
    Empty,
}

impl HeapNode {
    pub fn try_read_root<Br: BitRead>(reader: &mut Br) -> io::Result<Option<Self>> {
        let Some(type_flag) = reader.try_read_bits(1)? else {
            return Ok(None);
        };

        let mut tree_root = match type_flag {
            LEAF_FLAG => Self::Leaf(reader.read_byte()?),
            PAIR_FLAG => Self::Pair {
                left: Box::new(Self::read(reader)?),
                right: Box::new(Self::read(reader)?),
            },

            _ => unreachable!(),
        };

        if matches!(&tree_root, HeapNode::Leaf(_)) {
            tree_root = HeapNode::Pair {
                left: Box::new(tree_root),
                right: Box::new(HeapNode::Empty),
            }
        }

        Ok(Some(tree_root))
    }
}

impl BitWritable for HeapNode {
    fn write<W: bitpack::BitWrite>(&self, writer: &mut W) -> io::Result<()> {
        match self {
            Self::Leaf(byte) => {
                writer.write_bits(consts::LEAF_FLAG, consts::TYPE_FLAG_SIZE)?;
                writer.write_byte(*byte)?;
            }
            Self::Pair { left, right } => {
                writer.write_bits(consts::PAIR_FLAG, consts::TYPE_FLAG_SIZE)?;
                left.write(writer)?;
                right.write(writer)?;
            }
            Self::Empty => panic!("Empty leaf representation are only allowed when reading."),
        }

        Ok(())
    }
}

impl BitReadable for HeapNode {
    fn read<R: bitpack::BitRead>(reader: &mut R) -> io::Result<Self> {
        let type_flag = reader.read_bits(consts::TYPE_FLAG_SIZE)?;

        let node = match type_flag {
            LEAF_FLAG => Self::Leaf(reader.read_byte()?),
            PAIR_FLAG => {
                let left = Self::read(reader)?;
                let right = Self::read(reader)?;

                Self::Pair {
                    left: Box::new(left),
                    right: Box::new(right),
                }
            }

            _ => unreachable!(),
        };

        Ok(node)
    }
}

fn write_bit_to_node(
    node: &HeapNode,
    bit: u8,
    binary_repr_builders: &mut [HuffmanCodeBuilder; BYTE_TABLE_LEN],
) {
    match node {
        HeapNode::Leaf(byte) => binary_repr_builders[*byte as usize].write_bit(bit),
        HeapNode::Pair { left, right } => {
            write_bit_to_node(left, bit, binary_repr_builders);
            write_bit_to_node(right, bit, binary_repr_builders);
        }
        HeapNode::Empty => panic!("Empty node should only be used when reading")
    }
}

pub fn get_huffman_tree_and_codes(byte_table: ByteTable) -> Option<(HeapNode, HuffmanCodeTable)> {
    let mut binary_repr_builders = core::array::from_fn(|_| HuffmanCodeBuilder::new());

    let mut nodes = byte_table
        .into_iter()
        .enumerate()
        .filter(|(_, count)| *count != 0)
        .map(|(byte, count)| (count, HeapNode::Leaf(u8::try_from(byte).unwrap())))
        .collect::<Vec<_>>();

    if nodes.is_empty() {
        return None;
    }

    while nodes.len() > 1 {
        nodes.sort_by_key(|(count, _)| std::cmp::Reverse(*count));

        let (right_count, right_node) = nodes.pop().unwrap();
        let (left_count, left_node) = nodes.pop().unwrap();

        write_bit_to_node(&left_node, consts::LEFT_BIT, &mut binary_repr_builders);
        write_bit_to_node(&right_node, consts::RIGHT_BIT, &mut binary_repr_builders);

        let pair = HeapNode::Pair {
            left: Box::new(left_node),
            right: Box::new(right_node),
        };
        nodes.push((left_count + right_count, pair));
    }

    let (_, root) = nodes.pop().unwrap();
    if matches!(&root, HeapNode::Leaf(_)) {
        write_bit_to_node(&root, consts::LEFT_BIT, &mut binary_repr_builders);
    }

    let mut reprs = [const { None }; BYTE_TABLE_LEN];
    for (index, repr) in binary_repr_builders.into_iter().enumerate() {
        let repr = repr.finish();

        if !repr.0.is_empty() {
            reprs[index] = Some(repr)
        }
    }

    Some((root, reprs))
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod test {
    use crate::table::{ByteTable, BYTE_TABLE_LEN};

    use super::{get_huffman_tree_and_codes, HeapNode, HuffmanCode, HuffmanCodeBuilder, HuffmanCodeTable};

    macro_rules! create_byte_table {
        ($($index: literal : $count: literal),*) => {{
            #[allow(unused_mut)]
            let mut table: ByteTable = [Default::default(); BYTE_TABLE_LEN];

            $(table[$index] = $count;)*

            table
        }};
    }

    macro_rules! create_huffman_code_table {
        ($($index: literal : $slice: expr, $last_count: literal),*) => {{
            #[allow(unused_mut)]
            let mut table: HuffmanCodeTable = core::array::from_fn(|_| Default::default());

            $(table[$index] = Some(HuffmanCode($slice.into(), $last_count));)*

            table
        }};
    }

    #[test]
    fn empty_table_should_not_give_tree() {
        let byte_table = create_byte_table!();

        let opt = get_huffman_tree_and_codes(byte_table);
        assert!(opt.is_none())
    }

    #[test]
    fn single_byte_should_give_leaf_tree() {
        let byte_table = create_byte_table! {
            0: 1
        };

        let (tree, repr) = get_huffman_tree_and_codes(byte_table).unwrap();
        
        let expected = HeapNode::Leaf(0);
        let expected_code_table = create_huffman_code_table! {
            0: [0b0], 1
        };

        assert_eq!(tree, expected);
        assert_eq!(repr, expected_code_table);
    }

    #[test]
    fn two_byte_should_give_pair_tree() {
        let byte_table = create_byte_table! {
            0: 1,
            1: 1
        };

        let (tree, repr) = get_huffman_tree_and_codes(byte_table).unwrap();
        
        let expected = HeapNode::Pair { left: Box::new(HeapNode::Leaf(0)), right: Box::new(HeapNode::Leaf(1)) };
        let expected_code_table = create_huffman_code_table! {
            0: [0b0], 1,
            1: [0b1], 1
        };

        assert_eq!(tree, expected);
        assert_eq!(repr, expected_code_table);
    }

    #[test]
    fn balanced_tree_with_four_bytes() {
        let byte_table = create_byte_table! {
            0: 1,
            1: 1,
            2: 1,
            3: 1
        };

        let (tree, repr) = get_huffman_tree_and_codes(byte_table).unwrap();
        
        let expected = HeapNode::Pair { 
            left: Box::new(HeapNode::Pair { 
                left: Box::new(HeapNode::Leaf(2)), 
                right: Box::new(HeapNode::Leaf(3))
            }),
            right: Box::new(HeapNode::Pair { 
                left: Box::new(HeapNode::Leaf(0)), 
                right: Box::new(HeapNode::Leaf(1))
            })
        };
        let expected_code_table = create_huffman_code_table! {
            0: [0b01], 2,
            1: [0b11], 2,
            2: [0b00], 2,
            3: [0b10], 2
        };

        assert_eq!(tree, expected);
        assert_eq!(repr, expected_code_table);
    }

    #[test]
    fn test_huffman_code_builder_can_build_more_than_eight_bits() {
        let mut builder =  HuffmanCodeBuilder::new();

        for _ in 0..10 {
            builder.write_bit(1);
        }

        let output = builder.finish();
        assert_eq!(
            output,
            HuffmanCode(vec![0b11, 0b11111111], 2)
        )
    }

    #[test]
    fn test_huffman_code_builder_can_build_exactly_eight_bits() {
        let mut builder =  HuffmanCodeBuilder::new();

        for _ in 0..8 {
            builder.write_bit(1);
        }

        let output = builder.finish();
        assert_eq!(
            output,
            HuffmanCode(vec![0b11111111], 8)
        )
    }

    mod read {
        use crate::tree::HeapNode;

        #[test]
        fn try_read_from_empty_array_returns_none() {
            let output =
                bitpack::test::get_test_read_custom_readable_output(&[], HeapNode::try_read_root)
                    .unwrap();

            assert!(output.result.is_none());
            assert_eq!(output.cursor_position, 0);
        }

        #[test]
        fn single_leaf_is_correctly_read() {
            let output = bitpack::test::get_test_read_custom_readable_output(
                &[0b1110000_0, 0b1],
                HeapNode::try_read_root,
            )
            .unwrap();

            let root = output.result.unwrap();
            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Leaf(0b11110000)),
                right: Box::new(HeapNode::Empty),
            };

            assert_eq!(root, expected);
            assert_eq!(output.cursor_position, 1);
        }

        #[test]
        fn try_read_single_pair_of_node() {
            let output = bitpack::test::get_test_read_custom_readable_output(
                &[0b110000_0_1, 0b10011_0_11, 0b001],
                HeapNode::try_read_root,
            )
            .unwrap();

            let root = output.result.unwrap();
            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Leaf(0b11110000)),
                right: Box::new(HeapNode::Leaf(0b00110011)),
            };

            assert_eq!(root, expected);
            assert_eq!(output.cursor_position, 3);
        }

        #[test]
        fn try_read_a_two_level_complete_binary_tree() {
            let output = bitpack::test::get_test_read_custom_readable_output(
                &[
                    0b10000_0_1_1,
                    0b0011_0_111,
                    0b11_0_1_0011,
                    0b0_0_110001,
                    0b1010101,
                ],
                HeapNode::try_read_root,
            )
            .unwrap();

            let root = output.result.unwrap();
            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Pair {
                    left: Box::new(HeapNode::Leaf(0b11110000)),
                    right: Box::new(HeapNode::Leaf(0b00110011)),
                }),
                right: Box::new(HeapNode::Pair {
                    left: Box::new(HeapNode::Leaf(0b11000111)),
                    right: Box::new(HeapNode::Leaf(0b10101010)),
                }),
            };

            assert_eq!(root, expected);
            assert_eq!(output.cursor_position, 7);
        }

        #[test]
        fn try_read_a_two_level_not_complete_binary_tree() {
            let output = bitpack::test::get_test_read_custom_readable_output(
                &[0b10000_0_1_1, 0b0011_0_111, 0b111_0_0011, 0b11000],
                HeapNode::try_read_root,
            )
            .unwrap();

            let root = output.result.unwrap();
            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Pair {
                    left: Box::new(HeapNode::Leaf(0b11110000)),
                    right: Box::new(HeapNode::Leaf(0b00110011)),
                }),
                right: Box::new(HeapNode::Leaf(0b11000111)),
            };

            assert_eq!(root, expected);
            assert_eq!(output.cursor_position, 5);

            let output = bitpack::test::get_test_read_custom_readable_output(
                &[0b000111_0_1, 0b0000_0_1_11, 0b011_0_1111, 0b00110],
                HeapNode::try_read_root,
            )
            .unwrap();

            let root = output.result.unwrap();
            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Leaf(0b11000111)),
                right: Box::new(HeapNode::Pair {
                    left: Box::new(HeapNode::Leaf(0b11110000)),
                    right: Box::new(HeapNode::Leaf(0b00110011)),
                }),
            };

            assert_eq!(root, expected);
            assert_eq!(output.cursor_position, 5);
        }
    }

    mod write {
        use bitpack::BitWrite;

        use crate::tree::HeapNode;

        #[test]
        fn single_leaf_is_correctly_written() {
            let expected = HeapNode::Leaf(0b11110000);

            let output =
                bitpack::test::get_test_write_output(|writer| writer.write_writable(&expected))
                    .unwrap();

            assert_eq!(&output.vec, &[0b1110000_0, 0b1]);
            assert_eq!(output.cursor_position, 1);
        }

        #[test]
        fn try_write_single_pair_of_node() {
            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Leaf(0b11110000)),
                right: Box::new(HeapNode::Leaf(0b00110011)),
            };

            let output =
                bitpack::test::get_test_write_output(|writer| writer.write_writable(&expected))
                    .unwrap();

            assert_eq!(&output.vec, &[0b110000_0_1, 0b10011_0_11, 0b001]);
            assert_eq!(output.cursor_position, 3);
        }

        #[test]
        fn try_write_a_two_level_complete_binary_tree() {
            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Pair {
                    left: Box::new(HeapNode::Leaf(0b11110000)),
                    right: Box::new(HeapNode::Leaf(0b00110011)),
                }),
                right: Box::new(HeapNode::Pair {
                    left: Box::new(HeapNode::Leaf(0b11000111)),
                    right: Box::new(HeapNode::Leaf(0b10101010)),
                }),
            };

            let output =
                bitpack::test::get_test_write_output(|writer| writer.write_writable(&expected))
                    .unwrap();

            assert_eq!(
                &output.vec,
                &[
                    0b10000_0_1_1,
                    0b0011_0_111,
                    0b11_0_1_0011,
                    0b0_0_110001,
                    0b1010101
                ]
            );
            assert_eq!(output.cursor_position, 7);
        }

        #[test]
        fn try_write_a_two_level_not_complete_binary_tree() {
            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Pair {
                    left: Box::new(HeapNode::Leaf(0b11110000)),
                    right: Box::new(HeapNode::Leaf(0b00110011)),
                }),
                right: Box::new(HeapNode::Leaf(0b11000111)),
            };

            let output =
                bitpack::test::get_test_write_output(|writer| writer.write_writable(&expected))
                    .unwrap();

            assert_eq!(
                &output.vec,
                &[0b10000_0_1_1, 0b0011_0_111, 0b111_0_0011, 0b11000]
            );
            assert_eq!(output.cursor_position, 5);

            let expected = HeapNode::Pair {
                left: Box::new(HeapNode::Leaf(0b11000111)),
                right: Box::new(HeapNode::Pair {
                    left: Box::new(HeapNode::Leaf(0b11110000)),
                    right: Box::new(HeapNode::Leaf(0b00110011)),
                }),
            };

            let output =
                bitpack::test::get_test_write_output(|writer| writer.write_writable(&expected))
                    .unwrap();

            assert_eq!(
                &output.vec,
                &[0b000111_0_1, 0b0000_0_1_11, 0b011_0_1111, 0b00110]
            );
            assert_eq!(output.cursor_position, 5);
        }
    }
}
