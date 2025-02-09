use std::io::{Cursor, Seek};

use criterion::{
    criterion_group, criterion_main, measurement::WallTime, BatchSize, BenchmarkGroup, BenchmarkId,
    Criterion,
};
use flate2::Compression;
use rand::Rng;

fn benchmark_pack_for_function<R, S: Fn(Cursor<Vec<u8>>) -> R, F: Fn(&mut R, &mut Cursor<Vec<u8>>)>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    size: usize,
    function_id: &str,
    setup_reader: S,
    function: F,
) {
    group.bench_with_input(
        BenchmarkId::new(function_id, size),
        &size,
        |bencher, size| {
            bencher.iter_batched_ref(
                || {
                    let mut input_buf = Vec::<u8>::with_capacity(*size);
                    let mut rng = rand::rng();
                    for _ in 0..*size {
                        input_buf.push(rng.random());
                    }
                    // dbg!(&input_buf);

                    let read_cursor = Cursor::new(input_buf);
                    let read = setup_reader(read_cursor);

                    let output_vec = Vec::<u8>::with_capacity(*size);
                    let output_cursor = Cursor::new(output_vec);

                    (read, output_cursor)
                },
                |(reader, writer)| function(reader, writer),
                BatchSize::PerIteration,
            );
        },
    );
}

fn benchmark_pack_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("huffman::pack");
    for i in 8..=16 {
        let size = 1usize << i;

        benchmark_pack_for_function(
            &mut group,
            size, 
            "huffman::pack", 
            |reader| reader,
            |reader, writer| {
                huffman_format::pack_file(reader, writer).unwrap();
            }
        );
        
        benchmark_pack_for_function(
            &mut group,
            size, 
            "gzip", 
            |reader| flate2::read::GzEncoder::new(reader, Compression::best()),
            |reader, writer| {
                std::io::copy(reader, writer).unwrap();
            }
        );

        benchmark_pack_for_function(
            &mut group,
            size, 
            "xz (level 6)", 
            |reader| xz2::read::XzEncoder::new(reader, 6),
            |reader, writer| {
                std::io::copy(reader, writer).unwrap();
            }
        );
    }
    group.finish();
}

fn benchmark_unpack_for_function<R, I: Fn(&mut Cursor<Vec<u8>>, &mut Cursor<Vec<u8>>), S: Fn(Cursor<Vec<u8>>) -> R, F: Fn(&mut R, &mut Cursor<Vec<u8>>)>(
    group: &mut BenchmarkGroup<'_, WallTime>,
    size: usize,
    function_id: &str,
    setup_input: I,
    setup_reader: S,
    function: F,
) {
    group.bench_with_input(
        BenchmarkId::new(function_id, size),
        &size,
        |bencher, size| {
            bencher.iter_batched_ref(
                || {
                    let mut input_buf = Vec::<u8>::with_capacity(*size);
                    let mut rng = rand::rng();
                    for _ in 0..*size {
                        input_buf.push(rng.random());
                    }
                    let input_buf = std::hint::black_box(input_buf);
                    // dbg!(&input_buf);

                    let mut read_cursor = Cursor::new(input_buf);

                    let compressed_vec = Vec::<u8>::with_capacity(*size);
                    let mut compressed_cursor = Cursor::new(compressed_vec);

                    setup_input(&mut read_cursor, &mut compressed_cursor);
                    compressed_cursor.rewind().unwrap();

                    let reader = setup_reader(compressed_cursor);

                    let output_vec = Vec::<u8>::with_capacity(*size);
                    let output_cursor = Cursor::new(output_vec);

                    (reader, output_cursor)
                },
                |(reader, writer)| function(reader, writer),
                BatchSize::PerIteration,
            );
        },
    );
}

fn benchmark_unpack_speed(c: &mut Criterion) {
    let mut group = c.benchmark_group("huffman::unpack");
    for i in 8..=16 {
        let size = 1usize << i;
        
        benchmark_unpack_for_function(
            &mut group, 
            size, 
            "huffman::unpack", 
            |input_reader, output_writer| {
                huffman_format::pack_file(input_reader, output_writer).unwrap();
            }, 
            |reader| reader, 
            |reader, writer| {
                huffman_format::unpack_file(reader, writer).unwrap();
            }
        );

        benchmark_unpack_for_function(
            &mut group, 
            size, 
            "gzip", 
            |input_reader, output_writer| {
                let mut reader = flate2::read::GzEncoder::new(input_reader, Compression::best());
                std::io::copy(&mut reader, output_writer).unwrap();
            }, 
            |reader| flate2::read::GzDecoder::new(reader), 
            |reader, writer| {
                std::io::copy(reader, writer).unwrap();
            }
        );


        benchmark_unpack_for_function(
            &mut group, 
            size, 
            "xz (level 6)", 
            |input_reader, output_writer| {
                let mut reader = xz2::read::XzEncoder::new(input_reader, 6);
                std::io::copy(&mut reader, output_writer).unwrap();
            }, 
            |reader| xz2::read::XzDecoder::new(reader), 
            |reader, writer| {
                std::io::copy(reader, writer).unwrap();
            }
        );
    }
    group.finish();
}

criterion_group!(benches, benchmark_pack_speed, benchmark_unpack_speed);
criterion_main!(benches);
