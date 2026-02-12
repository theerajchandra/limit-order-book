use lob_core::book::{OrderBook, OrderBookConfig};
use lob_core::events::Event;
use lob_core::types::Side;
use proptest::prelude::*;

#[derive(Debug, Clone)]
enum FuzzAction {
    NewBid { id: u64, price: u64, qty: u64 },
    NewAsk { id: u64, price: u64, qty: u64 },
    Cancel { id: u64 },
}

fn fuzz_action_strategy() -> impl Strategy<Value = FuzzAction> {
    prop_oneof![
        (1u64..200, 90u64..110, 1u64..50)
            .prop_map(|(id, price, qty)| FuzzAction::NewBid { id, price, qty }),
        (1u64..200, 90u64..110, 1u64..50)
            .prop_map(|(id, price, qty)| FuzzAction::NewAsk { id, price, qty }),
        (1u64..200).prop_map(|id| FuzzAction::Cancel { id }),
    ]
}

fn fuzz_sequence_strategy() -> impl Strategy<Value = Vec<FuzzAction>> {
    proptest::collection::vec(fuzz_action_strategy(), 1..300)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn book_never_crossed(actions in fuzz_sequence_strategy()) {
        let mut book = OrderBook::new(OrderBookConfig { initial_capacity: 512 });

        for action in &actions {
            match action {
                FuzzAction::NewBid { id, price, qty } => {
                    let _ = book.submit_limit(*id, Side::Bid, *price, *qty);
                }
                FuzzAction::NewAsk { id, price, qty } => {
                    let _ = book.submit_limit(*id, Side::Ask, *price, *qty);
                }
                FuzzAction::Cancel { id } => {
                    let _ = book.cancel(*id);
                }
            }

            let tob = book.top_of_book();
            if let (Some(bid), Some(ask)) = (&tob.best_bid, &tob.best_ask) {
                prop_assert!(
                    bid.price < ask.price,
                    "Crossed book! bid={} >= ask={}",
                    bid.price,
                    ask.price,
                );
            }
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn order_count_consistent(actions in fuzz_sequence_strategy()) {
        let mut book = OrderBook::new(OrderBookConfig { initial_capacity: 512 });

        for action in &actions {
            match action {
                FuzzAction::NewBid { id, price, qty } => {
                    let _ = book.submit_limit(*id, Side::Bid, *price, *qty);
                }
                FuzzAction::NewAsk { id, price, qty } => {
                    let _ = book.submit_limit(*id, Side::Ask, *price, *qty);
                }
                FuzzAction::Cancel { id } => {
                    let _ = book.cancel(*id);
                }
            }
        }

        let snap = book.snapshot(1000);
        let snap_order_count: usize = snap
            .bids
            .iter()
            .chain(snap.asks.iter())
            .map(|l| l.order_count)
            .sum();
        prop_assert_eq!(
            book.order_count(),
            snap_order_count,
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn trade_qty_bounded(actions in fuzz_sequence_strategy()) {
        let mut book = OrderBook::new(OrderBookConfig { initial_capacity: 512 });

        for action in &actions {
            let (qty, events) = match action {
                FuzzAction::NewBid { id, price, qty } => {
                    (*qty, book.submit_limit(*id, Side::Bid, *price, *qty))
                }
                FuzzAction::NewAsk { id, price, qty } => {
                    (*qty, book.submit_limit(*id, Side::Ask, *price, *qty))
                }
                FuzzAction::Cancel { id } => (0, book.cancel(*id)),
            };

            let total_traded: u64 = events
                .iter()
                .filter_map(|e| match e {
                    Event::Trade { qty: q, .. } => Some(*q),
                    _ => None,
                })
                .sum();

            prop_assert!(
                total_traded <= qty,
                "Traded {} but incoming qty was {}",
                total_traded,
                qty,
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn snapshot_consistent(actions in fuzz_sequence_strategy()) {
        let mut book = OrderBook::new(OrderBookConfig { initial_capacity: 512 });

        for action in &actions {
            match action {
                FuzzAction::NewBid { id, price, qty } => {
                    let _ = book.submit_limit(*id, Side::Bid, *price, *qty);
                }
                FuzzAction::NewAsk { id, price, qty } => {
                    let _ = book.submit_limit(*id, Side::Ask, *price, *qty);
                }
                FuzzAction::Cancel { id } => {
                    let _ = book.cancel(*id);
                }
            }
        }

        let snap = book.snapshot(1000);

        for w in snap.bids.windows(2) {
            prop_assert!(w[0].price > w[1].price);
        }
        for w in snap.asks.windows(2) {
            prop_assert!(w[0].price < w[1].price);
        }

        for level in snap.bids.iter().chain(snap.asks.iter()) {
            prop_assert!(level.qty > 0);
            prop_assert!(level.order_count > 0);
        }
    }
}
