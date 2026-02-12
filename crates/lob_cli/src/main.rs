use std::fs::File;
use std::io::{self, BufRead, BufReader, Write as IoWrite};
use std::time::Instant;

use clap::{Parser, Subcommand};
use rand::prelude::*;

use lob_core::book::{OrderBook, OrderBookConfig};
use lob_core::events::{Command, Event};
use lob_core::types::Side;

#[derive(Parser)]
#[command(name = "lob", about = "High-performance Limit Order Book CLI")]
struct Cli {
    #[command(subcommand)]
    command: SubCmd,
}

#[derive(Subcommand)]
enum SubCmd {
    Generate {
        #[arg(short, long, default_value = "100000")]
        count: usize,
        #[arg(short, long)]
        output: Option<String>,
        #[arg(short, long, default_value = "42")]
        seed: u64,
    },
    Replay {
        #[arg(short, long)]
        input: Option<String>,
        #[arg(long, default_value = "false")]
        verbose: bool,
    },
    Demo {
        #[arg(short, long, default_value = "1000000")]
        count: usize,
        #[arg(long, default_value = "0.3")]
        cancel_ratio: f64,
        #[arg(short, long, default_value = "42")]
        seed: u64,
    },
}

fn generate_commands(count: usize, seed: u64, cancel_ratio: f64) -> Vec<Command> {
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    let mut rng = StdRng::seed_from_u64(seed);
    let mut cmds = Vec::with_capacity(count);
    let mut next_id: u64 = 1;
    let mut live_ids: Vec<u64> = Vec::new();

    let mid: u64 = 10_000;
    let spread: u64 = 50;

    for _ in 0..count {
        let r: f64 = rng.gen();
        if r < cancel_ratio && !live_ids.is_empty() {
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
            let offset: u64 = rng.gen_range(0..=spread);
            let price = match side {
                Side::Bid => mid - offset,
                Side::Ask => mid + offset,
            };
            let qty: u64 = rng.gen_range(1..=100);
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

struct ReplayStats {
    commands: usize,
    trades: usize,
    total_trade_qty: u64,
    fills: usize,
    cancels: usize,
    rejects: usize,
    elapsed: std::time::Duration,
}

fn replay_commands(
    book: &mut OrderBook,
    commands: impl Iterator<Item = Command>,
    verbose: bool,
) -> ReplayStats {
    let mut stats = ReplayStats {
        commands: 0,
        trades: 0,
        total_trade_qty: 0,
        fills: 0,
        cancels: 0,
        rejects: 0,
        elapsed: std::time::Duration::ZERO,
    };

    let start = Instant::now();

    for cmd in commands {
        let events = book.process(cmd);
        stats.commands += 1;

        for event in &events {
            match event {
                Event::Trade { qty, .. } => {
                    stats.trades += 1;
                    stats.total_trade_qty += qty;
                    if verbose {
                        eprintln!("  TRADE: {:?}", event);
                    }
                }
                Event::Filled { .. } => stats.fills += 1,
                Event::Cancelled { .. } => stats.cancels += 1,
                Event::Rejected { .. } => stats.rejects += 1,
                _ => {}
            }
        }
    }

    stats.elapsed = start.elapsed();
    stats
}

fn print_stats(stats: &ReplayStats, book: &OrderBook) {
    let tob = book.top_of_book();
    let elapsed_us = stats.elapsed.as_micros();
    let throughput = if elapsed_us > 0 {
        (stats.commands as f64 / stats.elapsed.as_secs_f64()) as u64
    } else {
        0
    };
    let avg_latency_ns = if stats.commands > 0 {
        stats.elapsed.as_nanos() / stats.commands as u128
    } else {
        0
    };

    println!("============================================================");
    println!("                   LOB Replay Summary");
    println!("============================================================");
    println!("  Commands processed : {}", stats.commands);
    println!("  Trades             : {}", stats.trades);
    println!("  Total trade qty    : {}", stats.total_trade_qty);
    println!("  Fills              : {}", stats.fills);
    println!("  Cancels            : {}", stats.cancels);
    println!("  Rejects            : {}", stats.rejects);
    println!("------------------------------------------------------------");
    println!("  Elapsed            : {:.3} ms", stats.elapsed.as_secs_f64() * 1000.0);
    println!("  Throughput         : {} commands/sec", throughput);
    println!("  Avg latency        : {} ns/command", avg_latency_ns);
    println!("------------------------------------------------------------");
    println!("  Resting orders     : {}", book.order_count());
    println!("  Bid levels         : {}", book.bid_level_count());
    println!("  Ask levels         : {}", book.ask_level_count());
    if let Some(bid) = &tob.best_bid {
        println!("  Best bid           : {} (qty {}, {} orders)", bid.price, bid.qty, bid.order_count);
    } else {
        println!("  Best bid           : --");
    }
    if let Some(ask) = &tob.best_ask {
        println!("  Best ask           : {} (qty {}, {} orders)", ask.price, ask.qty, ask.order_count);
    } else {
        println!("  Best ask           : --");
    }
    println!("============================================================");

    let snap = book.snapshot(5);
    if !snap.bids.is_empty() || !snap.asks.is_empty() {
        println!();
        println!("  {:>10}  {:>10}  |  {:<10}  {:<10}", "BID QTY", "BID PX", "ASK PX", "ASK QTY");
        println!("  {:>10}  {:>10}  |  {:<10}  {:<10}", "-------", "------", "------", "-------");
        let depth = snap.bids.len().max(snap.asks.len());
        for i in 0..depth {
            let bid_str = snap
                .bids
                .get(i)
                .map(|l| format!("{:>10}  {:>10}", l.qty, l.price))
                .unwrap_or_else(|| format!("{:>10}  {:>10}", "", ""));
            let ask_str = snap
                .asks
                .get(i)
                .map(|l| format!("{:<10}  {:<10}", l.price, l.qty))
                .unwrap_or_else(|| format!("{:<10}  {:<10}", "", ""));
            println!("  {}  |  {}", bid_str, ask_str);
        }
        println!();
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        SubCmd::Generate {
            count,
            output,
            seed,
        } => {
            let cmds = generate_commands(count, seed, 0.3);
            let mut writer: Box<dyn IoWrite> = match output {
                Some(path) => Box::new(File::create(&path).expect("cannot create output file")),
                None => Box::new(io::stdout().lock()),
            };
            for cmd in &cmds {
                let line = serde_json::to_string(cmd).expect("serialize");
                writeln!(writer, "{}", line).expect("write");
            }
            eprintln!("Generated {} commands", cmds.len());
        }
        SubCmd::Replay { input, verbose } => {
            let reader: Box<dyn BufRead> = match input {
                Some(path) => {
                    Box::new(BufReader::new(File::open(&path).expect("cannot open input")))
                }
                None => Box::new(BufReader::new(io::stdin())),
            };

            let commands = reader.lines().map(|line| {
                let line = line.expect("read line");
                serde_json::from_str::<Command>(&line).expect("parse command")
            });

            let mut book = OrderBook::new(OrderBookConfig::default());
            let stats = replay_commands(&mut book, commands, verbose);
            print_stats(&stats, &book);
        }
        SubCmd::Demo {
            count,
            cancel_ratio,
            seed,
        } => {
            eprintln!(
                "Generating {} commands (cancel_ratio={:.0}%, seed={}) ...",
                count,
                cancel_ratio * 100.0,
                seed
            );
            let cmds = generate_commands(count, seed, cancel_ratio);
            let mut book = OrderBook::new(OrderBookConfig::default());
            let stats = replay_commands(&mut book, cmds.into_iter(), false);
            print_stats(&stats, &book);
        }
    }
}
