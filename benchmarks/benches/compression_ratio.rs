use std::io::Cursor;

use criterion::{criterion_group, criterion_main, measurement::{Measurement, ValueFormatter}, BenchmarkId, Criterion};
use flate2::Compression;
use rand::distr::Distribution;

struct CompressionRatio;
impl Measurement for CompressionRatio {
    type Intermediate = ();
    type Value = f64;

    fn start(&self) -> Self::Intermediate {
        ()
    }
    fn end(&self, _i: Self::Intermediate) -> Self::Value {
        0.0
    }
    fn add(&self, v1: &Self::Value, v2: &Self::Value) -> Self::Value {
        *v1 + *v2
    }
    fn zero(&self) -> Self::Value {
        0.0
    }

    fn to_f64(&self, value: &Self::Value) -> f64 {
        *value
    }
    fn formatter(&self) -> &dyn criterion::measurement::ValueFormatter {
        &CompressionRatioFormatter
    }
}

struct CompressionRatioFormatter;
impl ValueFormatter for CompressionRatioFormatter {
    fn scale_for_machines(&self, _values: &mut [f64]) -> &'static str {
        "input_size / output_size"
    }
    fn scale_throughputs(
        &self,
        _typical_value: f64,
        _throughput: &criterion::Throughput,
        _values: &mut [f64],
    ) -> &'static str {
        "input_size / output_size"
    }
    fn scale_values(&self, _typical_value: f64, _values: &mut [f64]) -> &'static str {
        "input_size / output_size"
    }
}

mod entropy {
    use rand::distr::weighted::WeightedIndex;

    const BITS: usize = 8;

    fn binary_entropy(p: f64) -> f64 {
        // special case because p.log2() returns nan otherwise.
        if p == 0.0 {
            return 0.0
        }

        -p * p.log2() - (1.0 - p) * (1.0 - p).log2()
    }

    fn binary_entropy_derivative(p: f64) -> f64 {
        -(p / (1.0 - p)).log2()
    }

    fn binary_entropy_inverse_approximation(entropy: f64) -> f64 {
        let mut approx = 0.5 * entropy / (1.0 + (1.0 - entropy).sqrt());

        const NEWTON_ITERATIONS: usize = 5;
        for _ in 0..NEWTON_ITERATIONS {
            if binary_entropy(approx) == entropy {
                break;
            }

            approx -= (binary_entropy(approx) - entropy) / binary_entropy_derivative(approx);
        }
        approx
    }

    pub fn generate_distribution(entropy: f64) -> WeightedIndex<f64> {
        let bit_entropy = entropy / BITS as f64;
        
        let bit_probability = binary_entropy_inverse_approximation(bit_entropy);
        assert!(bit_probability >= 0.0);
        assert!(bit_probability <= 1.0);

        let mut probability_per_bit_count = [0.0; BITS + 1];
        for i in 0..probability_per_bit_count.len() {
            probability_per_bit_count[i] = bit_probability.powi(i as i32) * (1.0 - bit_probability).powi((BITS - i) as i32);
        }

        let mut probabilities = [0.0; u8::MAX as usize + 1];
        for byte in 0..=u8::MAX {
            let one_counts = byte.count_ones();

            probabilities[byte as usize] = probability_per_bit_count[one_counts as usize];
        }

        WeightedIndex::new(probabilities).unwrap()
    }
}

fn benchmark_compression_ratio(c: &mut Criterion<CompressionRatio>) {
    let mut group = c.benchmark_group("compression ratio");

    const SAMPLE_SIZE: usize = 1 << 16;

    const MIN_ENTROPY: f64 = 0.0;
    const MAX_ENTROPY: f64 = 8.0; // 256.log2()
    const ENTROPY_STEP: f64 = 0.5;

    let mut entropy = MIN_ENTROPY;
    while entropy <= MAX_ENTROPY {
        let distribution = entropy::generate_distribution(entropy);

        group.bench_with_input(BenchmarkId::new("huffman::pack", entropy), &distribution, |bencher, distribution| {
            bencher.iter_custom(|_| {
                let mut input_buf = Vec::<u8>::with_capacity(SAMPLE_SIZE);
                let mut rng = rand::rng();
                for _ in 0..SAMPLE_SIZE {
                    input_buf.push(u8::try_from(distribution.sample(&mut rng)).unwrap());
                }
                // dbg!(&input_buf);

                let mut read_cursor = Cursor::new(input_buf);

                let output_vec = Vec::<u8>::with_capacity(SAMPLE_SIZE);
                let mut output_cursor = Cursor::new(output_vec);

                huffman_format::pack_file(&mut read_cursor, &mut output_cursor).unwrap();

                let compression_ratio = read_cursor.get_ref().len() as f64 / output_cursor.get_ref().len() as f64;
                // dbg!(compression_ratio);

                compression_ratio
            });
        });

        group.bench_with_input(BenchmarkId::new("gzip", entropy), &distribution, |bencher, distribution| {
            bencher.iter_custom(|_| {
                let mut input_buf = Vec::<u8>::with_capacity(SAMPLE_SIZE);
                let mut rng = rand::rng();
                for _ in 0..SAMPLE_SIZE {
                    input_buf.push(u8::try_from(distribution.sample(&mut rng)).unwrap());
                }
                // dbg!(&input_buf);

                let read_cursor = Cursor::new(input_buf);
                let mut read = flate2::read::GzEncoder::new(read_cursor, Compression::best());

                let output_vec = Vec::<u8>::with_capacity(SAMPLE_SIZE);
                let mut output_cursor = Cursor::new(output_vec);

                std::io::copy(&mut read, &mut output_cursor).unwrap();                

                let compression_ratio = read.get_ref().get_ref().len() as f64 / output_cursor.get_ref().len() as f64;
                // dbg!(compression_ratio);

                compression_ratio
            });
        });

        group.bench_with_input(BenchmarkId::new("xz (level 6)", entropy), &distribution, |bencher, distribution| {
            bencher.iter_custom(|_| {
                let mut input_buf = Vec::<u8>::with_capacity(SAMPLE_SIZE);
                let mut rng = rand::rng();
                for _ in 0..SAMPLE_SIZE {
                    input_buf.push(u8::try_from(distribution.sample(&mut rng)).unwrap());
                }
                // dbg!(&input_buf);

                let read_cursor = Cursor::new(input_buf);

                let output_vec = Vec::<u8>::with_capacity(SAMPLE_SIZE);
                let mut output_cursor = Cursor::new(output_vec);

                let mut read = xz2::read::XzEncoder::new(read_cursor, 6);
                std::io::copy(&mut read, &mut output_cursor).unwrap();

                let compression_ratio = read.get_ref().get_ref().len() as f64 / output_cursor.get_ref().len() as f64;
                // dbg!(compression_ratio);

                compression_ratio
            });
        });

        entropy += ENTROPY_STEP;
    }
}

criterion_group!(
    name = benches;
    config = Criterion::default().with_measurement(CompressionRatio);
    targets = benchmark_compression_ratio
);
criterion_main!(benches);