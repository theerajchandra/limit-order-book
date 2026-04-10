# Limit Order Book with Rust

A high-performance, continuous-trading limit order book engine written from scratch in Rust.

This is a personal project built to develop a deep understanding of order book internals -- how orders are stored, prioritized, matched, and cancelled -- and to explore the data structure and systems design choices that make these operations fast at scale.

It is an in-process matching engine core plus a CLI harness, not a full real-time exchange service. There is no network transport, streaming infrastructure, persistence layer, or distributed runtime in this repository.

## Problem

Traditional limit order book implementations suffer from three performance bottlenecks:

- **Per-order heap allocation.** Every new order and cancel triggers `malloc`/`free` churn, fragmenting memory and pressuring the allocator.
- **Cache-hostile pointer chasing.** Linked lists of heap-allocated nodes scatter data across memory, defeating prefetchers and increasing latency.
- **Lock contention.** Multi-threaded matching engines pay heavy synchronization costs on every operation in the hot path.

This implementation addresses all three:

1. **Slab/arena allocation.** Orders live in a flat `Vec` with a free-list. Inserts and removes are O(1) with zero heap churn.
2. **Intrusive doubly-linked lists.** FIFO queues within a price level use indices into the slab instead of heap pointers, keeping data compact and cache-friendly.
3. **Single-writer determinism.** The matching loop is single-threaded and fully deterministic for a given event sequence, eliminating locks entirely.

## Architecture

```
                          OrderBook
  +----------+   +----------+   +------------------+
  | BookSide |   | BookSide |   | HashMap<OrderId, |
  |  (Bids)  |   |  (Asks)  |   |   OrderHandle>   |
  +----+-----+   +----+-----+   +------------------+
       |              |
  BTreeMap<Price, PriceLevel>
       |              |
  +----+--------------+------------------------+
  |              OrderSlab (arena)              |
  |  [ Order | Order | FREE | Order | FREE ]   |
  |    <->      <->             <->             |
  |  intrusive prev/next links (slab indices)   |
  +---------------------------------------------+
```

### Complexity

| Operation              | Time Complexity  | Notes                           |
|------------------------|------------------|---------------------------------|
| New limit (no match)   | O(log P)         | BTreeMap insert                 |
| New limit (with match) | O(log P + M)     | M = levels swept                |
| Cancel                 | O(log P)         | HashMap lookup + BTreeMap       |
| Modify (qty decrease)  | O(1)             | In-place update, keeps priority |
| Modify (price change)  | O(log P)         | Cancel + re-insert              |
| Top-of-book query      | O(1)             | BTreeMap first/last             |

P = number of distinct price levels. M = number of levels swept during matching.

## Project Structure

```
crates/
  lob_core/           Library: book, matching engine, data structures
    src/
      lib.rs           Public API and module declarations
      types.rs         Price, Qty, OrderId, Side, snapshots
      events.rs        Command (input) and Event (output) enums
      slab.rs          Arena allocator with free-list
      level.rs         PriceLevel: intrusive FIFO queue
      side.rs          BookSide: BTreeMap<Price, PriceLevel>
      match_engine.rs  Price-time priority matching
      book.rs          OrderBook: top-level orchestrator
    tests/
      book_tests.rs          20 integration tests
      proptest_invariants.rs 4 property-based tests
  lob_cli/            CLI: generate, replay, demo
  lob_bench/          Criterion benchmarks
```

## Usage

### Build

```
cargo build --release
```

### Test

```
cargo test -p lob_core
```

Runs 5 unit tests, 20 integration tests, and 4 property-based tests (via proptest).

### Demo

```
cargo run --release -p lob_cli -- demo --count 1000000
```

Generates a synthetic event stream (1M commands, 30% cancel ratio) and replays it through the engine, printing top-of-book and throughput statistics.

```
cargo run --release -p lob_cli -- generate --count 100000 --output events.jsonl
cargo run --release -p lob_cli -- replay --input events.jsonl
```

### Benchmarks

```
cargo bench -p lob_bench
```

## Benchmark Results

Single-threaded, release build, Apple Silicon (M-series):

| Benchmark               |   Size    | Throughput       | Avg Latency |
|-------------------------|-----------|------------------|-------------|
| new_order (no match)    |  100,000  | ~7.8M ops/sec    | ~128 ns     |
| new_order (with match)  |  100,000  | ~6.0M ops/sec    | ~167 ns     |
| cancel                  |  100,000  | ~9.0M ops/sec    | ~111 ns     |
| mixed workload          | 1,000,000 | ~6.7M ops/sec    | ~148 ns     |

## Real-Time Goals

The current repository focuses on the deterministic matching core: a single-process, in-memory engine with a replay/demo CLI around it. The longer-term goal is to use this core as the matching hot path inside a real-time trading system.

That future system would likely add:

- A low-latency ingress layer for orders and cancels (for example FIX, WebSocket, or an internal gateway).
- A sequenced command stream so each instrument can still be processed by a single writer.
- Durable event logging for recovery and deterministic replay.
- Market data fanout for trades, top-of-book, and depth updates.
- Risk checks, account controls, observability, and operational tooling around the core engine.

In other words, the real-time objective is not to make the matching loop itself distributed. It is to keep the matching loop small and deterministic, then place transport, persistence, and fanout infrastructure around it.

## Design Decisions

**Integer prices and quantities.** Floats introduce rounding errors and are slower for comparisons. All prices are in ticks, quantities in lots.

**Slab arena.** A flat `Vec<Slot>` with a free-list gives O(1) allocation/deallocation, stable indices, and excellent cache locality. No `Box<Order>`.

**Intrusive linked lists.** Each `Order` stores `prev`/`next` indices (not pointers) into the slab. This avoids separate `VecDeque` allocations per price level and allows O(1) removal of any order from the middle of the queue.

**BTreeMap for price levels.** Gives O(log P) ordered access to the best price. For a typical book with ~100 levels, this is 6-7 comparisons.

**Single-writer model.** The matching engine is deliberately single-threaded. In production systems, concurrency is handled at the edges (e.g. an SPSC ring buffer feeding commands). This avoids all locking overhead in the hot path.

**Cancel+replace for price changes.** Modifying a price always loses time priority (industry standard). Only quantity decreases preserve priority.

## Correctness

The following properties are covered by tests:

- Property-based tests verify that the book never crosses, trade quantity never exceeds incoming quantity, order counts remain consistent with snapshots, and snapshot levels stay sorted with positive quantities and counts.
- Integration tests verify FIFO within a level, duplicate `OrderId` rejection, zero-quantity rejection, cancel behavior, modify behavior, partial fills, and multi-level sweeps.

## License

MIT
