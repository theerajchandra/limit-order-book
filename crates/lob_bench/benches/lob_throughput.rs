use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rand::prelude::*;
use rand::rngs::StdRng;

use lob_core::book::{OrderBook, OrderBookConfig};
use lob_core::events::Command;
use lob_core::types::Side;

fn make_book() -> OrderBook {
    OrderBook::new(OrderBookConfig {
        initial_capacity: 1 << 17,
    })
}

fn preloaded_book(n: usize, seed: u64) -> (OrderBook, Vec<u64>) {
    let mut book = make_book();
    let mut rng = StdRng::seed_from_u64(seed);
    let mid = 10_000u64;
    let spread = 50u64;
    let mut ids = Vec::with_capacity(n);

    for id in 1..=(n as u64) {
        let side = if rng.gen::<bool>() {
            Side::Bid
        } else {
            Side::Ask
        };
        let price = match side {
            Side::Bid => mid - rng.gen_range(1..=spread),
            Side::Ask => mid + rng.gen_range(1..=spread),
        };
        let qty = rng.gen_range(1u64..=100);
        book.submit_limit(id, side, price, qty);
        ids.push(id);
    }

    (book, ids)
}

fn non_crossing_commands(n: usize, start_id: u64, seed: u64) -> Vec<Command> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mid = 10_000u64;
    let spread = 50u64;
    let mut cmds = Vec::with_capacity(n);

    for i in 0..(n as u64) {
        let id = start_id + i;
        let side = if rng.gen::<bool>() {
            Side::Bid
        } else {
            Side::Ask
        };
        let price = match side {
            Side::Bid => mid - rng.gen_range(1..=spread),
            Side::Ask => mid + rng.gen_range(1..=spread),
        };
        let qty = rng.gen_range(1u64..=100);
        cmds.push(Command::NewOrder {
            id,
            side,
            price,
            qty,
        });
    }

    cmds
}

fn crossing_commands(n: usize, start_id: u64, seed: u64) -> Vec<Command> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mid = 10_000u64;
    let spread = 20u64;
    let mut cmds = Vec::with_capacity(n);

    for i in 0..(n as u64) {
        let id = start_id + i;
        let side = if rng.gen::<bool>() {
            Side::Bid
        } else {
            Side::Ask
        };
        let price = match side {
            Side::Bid => mid + rng.gen_range(0..=spread),
            Side::Ask => mid - rng.gen_range(0..=spread),
        };
        let qty = rng.gen_range(1u64..=20);
        cmds.push(Command::NewOrder {
            id,
            side,
            price,
            qty,
        });
    }

    cmds
}

fn mixed_commands(n: usize, seed: u64) -> Vec<Command> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mid = 10_000u64;
    let spread = 50u64;
    let mut cmds = Vec::with_capacity(n);
    let mut next_id = 1u64;
    let mut live_ids: Vec<u64> = Vec::new();

    for _ in 0..n {
        let r: f64 = rng.gen();
        if r < 0.3 && !live_ids.is_empty() {
            let idx = rng.gen_range(0..live_ids.len());
            let id = live_ids.swap_remove(idx);
            cmds.push(Command::Cancel { id });
        } else {
            let id = next_id;
            next_id += 1;
            let side = if rng.gen::<bool>() {
                Side::Bid
            } else {
                Side::Ask
            };
            let offset = rng.gen_range(0..=spread);
            let price = match side {
                Side::Bid => mid - offset,
                Side::Ask => mid + offset,
            };
            let qty = rng.gen_range(1u64..=100);
            cmds.push(Command::NewOrder {
                id,
                side,
                price,
                qty,
            });
            live_ids.push(id);
        }
    }

    cmds
}

fn bench_new_order_no_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("new_order_no_match");
    for &size in &[1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &n| {
            let cmds = non_crossing_commands(n, 1, 123);
            b.iter(|| {
                let mut book = make_book();
                for cmd in &cmds {
                    black_box(book.process(cmd.clone()));
                }
            });
        });
    }
    group.finish();
}

fn bench_new_order_with_match(c: &mut Criterion) {
    let mut group = c.benchmark_group("new_order_with_match");
    for &size in &[1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &n| {
            let cmds = crossing_commands(n, 1_000_000, 456);
            b.iter(|| {
                let (mut book, _) = preloaded_book(10_000, 42);
                for cmd in &cmds {
                    black_box(book.process(cmd.clone()));
                }
            });
        });
    }
    group.finish();
}

fn bench_cancel(c: &mut Criterion) {
    let mut group = c.benchmark_group("cancel");
    for &size in &[1_000, 10_000, 100_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &n| {
            b.iter_batched(
                || {
                    let (book, ids) = preloaded_book(n, 42);
                    let cancel_cmds: Vec<Command> =
                        ids.iter().map(|&id| Command::Cancel { id }).collect();
                    (book, cancel_cmds)
                },
                |(mut book, cmds)| {
                    for cmd in &cmds {
                        black_box(book.process(cmd.clone()));
                    }
                },
                criterion::BatchSize::LargeInput,
            );
        });
    }
    group.finish();
}

fn bench_mixed_workload(c: &mut Criterion) {
    let mut group = c.benchmark_group("mixed_workload");
    for &size in &[10_000, 100_000, 1_000_000] {
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, &n| {
            let cmds = mixed_commands(n, 789);
            b.iter(|| {
                let mut book = make_book();
                for cmd in &cmds {
                    black_box(book.process(cmd.clone()));
                }
            });
        });
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_new_order_no_match,
    bench_new_order_with_match,
    bench_cancel,
    bench_mixed_workload,
);
criterion_main!(benches);
