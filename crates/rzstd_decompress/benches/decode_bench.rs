use std::io::Cursor;
use std::{hint::black_box, time::Duration};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use rzstd_decompress::{self, MAX_BLOCK_SIZE};

fn bench_silesia_corpus(c: &mut Criterion) {
    let mut group = c.benchmark_group("Silesia Corpus");
    group.measurement_time(Duration::from_secs(7));

    macro_rules! bench_entry {
        ($file:ident) => {
            let compressed: &[u8] =
                include_bytes!(concat!("silesia_corpus/", stringify!($file), ".zst"));
            let expected: &[u8] =
                include_bytes!(concat!("silesia_corpus/", stringify!($file)));
            let file_name = stringify!($file);
            let input = (compressed, expected);

            group.throughput(Throughput::Bytes(expected.len() as u64));

            group.bench_with_input(
                BenchmarkId::new("rzstd", file_name),
                &input,
                |b, &(compressed, expected)| {
                    b.iter(|| {
                        let window_size = 100 * 1024 * 1024 + MAX_BLOCK_SIZE as usize;
                        let mut window_buffer = vec![0u8; window_size];
                        let mut output_buffer = Vec::with_capacity(expected.len());
                        let mut decoder = rzstd_decompress::Decoder::new(
                            black_box(compressed),
                            &mut window_buffer,
                            window_size,
                        );
                        decoder.decode(&mut output_buffer).unwrap();
                        assert_eq!(output_buffer, expected);
                    })
                },
            );

            group.bench_with_input(
                BenchmarkId::new("ruzstd", file_name),
                &input,
                |b, &(compressed, expected)| {
                    b.iter(|| {
                        let mut decoder = ruzstd::decoding::StreamingDecoder::new(
                            Cursor::new(black_box(compressed)),
                        )
                        .unwrap();
                        let mut output_buffer = Vec::with_capacity(expected.len());
                        std::io::copy(&mut decoder, &mut output_buffer).unwrap();
                        assert_eq!(output_buffer, expected);
                    })
                },
            );

            group.bench_with_input(
                BenchmarkId::new("libzstd", file_name),
                &input,
                |b, &(compressed, expected)| {
                    b.iter(|| {
                        let mut output_buffer = Vec::with_capacity(compressed.len());
                        let mut decoder =
                            zstd::stream::read::Decoder::new(black_box(compressed))
                                .unwrap();
                        std::io::copy(&mut decoder, &mut output_buffer).unwrap();
                        assert_eq!(output_buffer, expected);
                    })
                },
            );
        };
    }

    bench_entry!(dickens);
    // bench_entry!(mozilla);
    bench_entry!(nci);
    bench_entry!(ooffice);
    bench_entry!(osdb);
    bench_entry!(reymont);
    bench_entry!(samba);
    bench_entry!(sao);
    bench_entry!(webster);
    bench_entry!(xml);
    bench_entry!(x_ray);

    group.finish();
}

criterion_group!(benches, bench_silesia_corpus);
criterion_main!(benches);
