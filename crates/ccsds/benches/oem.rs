use std::{hint::black_box, io::Cursor};

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use orskit_ccsds::{parse_oem_kvn, parse_oem_kvn_parallel, OemKvnReader};

const TARGET_BYTES: usize = 100 * 1024 * 1024;

fn large_oem() -> String {
    let header = "CCSDS_OEM_VERS = 3.0\n\
CREATION_DATE = 2024-01-01T00:00:00\n\
ORIGINATOR = ORSKIT-BENCH\n\
META_START\n\
OBJECT_NAME = BENCHMARK\n\
OBJECT_ID = 2024-001A\n\
CENTER_NAME = EARTH\n\
REF_FRAME = GCRF\n\
TIME_SYSTEM = UTC\n\
START_TIME = 2024-01-01T00:00:00\n\
STOP_TIME = 2024-01-01T00:01:00\n\
META_STOP\n";
    let record = "2024-01-01T00:00:00.000000000 7000.0 0.0 0.0 0.0 7.5 0.0\n";
    let count = TARGET_BYTES
        .saturating_sub(header.len())
        .div_ceil(record.len());
    let mut input = String::with_capacity(header.len() + count * record.len());
    input.push_str(header);
    for _ in 0..count {
        input.push_str(record);
    }
    input
}

fn benchmark_oem(c: &mut Criterion) {
    let input = large_oem();
    let mut group = c.benchmark_group("oem_kvn_100_mib");
    group.throughput(Throughput::Bytes(input.len() as u64));
    group.sample_size(10);

    group.bench_function("streaming", |bencher| {
        bencher.iter(|| {
            let reader = OemKvnReader::new(Cursor::new(black_box(input.as_bytes())));
            let mut states = 0usize;
            for event in reader {
                if matches!(
                    event.expect("generated benchmark OEM is valid"),
                    orskit_ccsds::OemEvent::Coordinates(_)
                ) {
                    states += 1;
                }
            }
            black_box(states)
        });
    });

    group.bench_function("rayon_collect", |bencher| {
        bencher.iter(|| {
            black_box(
                parse_oem_kvn_parallel(black_box(&input))
                    .expect("generated benchmark OEM is valid"),
            )
        });
    });

    group.bench_function("sequential_collect", |bencher| {
        bencher.iter(|| {
            black_box(parse_oem_kvn(black_box(&input)).expect("generated benchmark OEM is valid"))
        });
    });
    group.finish();
}

criterion_group!(benches, benchmark_oem);
criterion_main!(benches);
