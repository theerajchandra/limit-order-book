# Limit Order Book with Rust

A high-performance, continuous-trading limit order book engine written from scratch in Rust.

This is a personal project built to develop a deep understanding of order book internals -- how orders are stored, prioritized, matched, and cancelled -- and to explore the data structure and systems design choices that make these operations fast at scale.

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

Runs 5 unit tests, 20 integration tests, 4 property-based tests (via proptest), and 1 doc-test.

### Demo

```
cargo run --release -p lob_cli -- demo --count 1000000
```

Generates a synthetic event stream (1M commands, 30% cancel ratio) and replays it through the engine, printing trades, top-of-book, and throughput statistics.

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

## Design Decisions

**Integer prices and quantities.** Floats introduce rounding errors and are slower for comparisons. All prices are in ticks, quantities in lots.

**Slab arena.** A flat `Vec<Slot>` with a free-list gives O(1) allocation/deallocation, stable indices, and excellent cache locality. No `Box<Order>`.

**Intrusive linked lists.** Each `Order` stores `prev`/`next` indices (not pointers) into the slab. This avoids separate `VecDeque` allocations per price level and allows O(1) removal of any order from the middle of the queue.

**BTreeMap for price levels.** Gives O(log P) ordered access to the best price. For a typical book with ~100 levels, this is 6-7 comparisons.

**Single-writer model.** The matching engine is deliberately single-threaded. In production systems, concurrency is handled at the edges (e.g. an SPSC ring buffer feeding commands). This avoids all locking overhead in the hot path.

**Cancel+replace for price changes.** Modifying a price always loses time priority (industry standard). Only quantity decreases preserve priority.

## Correctness

The following invariants are enforced and verified by property-based tests across random event sequences:

- No crossed book: after every operation, best bid < best ask.
- FIFO within level: orders match in arrival order.
- OrderId uniqueness: duplicate IDs are rejected.
- No zero quantities: rejected at submission.
- Aggregate consistency: snapshot quantities equal summed order quantities.

## License

MIT
