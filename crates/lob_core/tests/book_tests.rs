use lob_core::book::{OrderBook, OrderBookConfig};
use lob_core::events::{Command, Event};
use lob_core::types::Side;

fn new_book() -> OrderBook {
    OrderBook::new(OrderBookConfig {
        initial_capacity: 256,
    })
}

fn has_trade(events: &[Event], aggressor: u64, passive: u64, qty: u64) -> bool {
    events.iter().any(|e| {
        matches!(e, Event::Trade { aggressor_id, passive_id, qty: q, .. }
            if *aggressor_id == aggressor && *passive_id == passive && *q == qty)
    })
}

fn has_accepted(events: &[Event], id: u64) -> bool {
    events.iter().any(|e| matches!(e, Event::Accepted { id: i } if *i == id))
}

fn has_filled(events: &[Event], id: u64) -> bool {
    events.iter().any(|e| matches!(e, Event::Filled { id: i } if *i == id))
}

fn has_cancelled(events: &[Event], id: u64) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::Cancelled { id: i } if *i == id))
}

fn has_rejected(events: &[Event], id: u64) -> bool {
    events
        .iter()
        .any(|e| matches!(e, Event::Rejected { id: i, .. } if *i == id))
}

#[test]
fn place_single_bid() {
    let mut book = new_book();
    let events = book.submit_limit(1, Side::Bid, 100, 10);
    assert!(has_accepted(&events, 1));
    assert_eq!(book.order_count(), 1);

    let tob = book.top_of_book();
    assert_eq!(tob.best_bid.as_ref().unwrap().price, 100);
    assert_eq!(tob.best_bid.as_ref().unwrap().qty, 10);
    assert!(tob.best_ask.is_none());
}

#[test]
fn place_single_ask() {
    let mut book = new_book();
    let events = book.submit_limit(1, Side::Ask, 105, 20);
    assert!(has_accepted(&events, 1));
    let tob = book.top_of_book();
    assert!(tob.best_bid.is_none());
    assert_eq!(tob.best_ask.as_ref().unwrap().price, 105);
}

#[test]
fn exact_match_bid_then_ask() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    let events = book.submit_limit(2, Side::Ask, 100, 10);

    assert!(has_trade(&events, 2, 1, 10));
    assert!(has_filled(&events, 1));
    assert!(has_filled(&events, 2));
    assert_eq!(book.order_count(), 0);
}

#[test]
fn exact_match_ask_then_bid() {
    let mut book = new_book();
    book.submit_limit(1, Side::Ask, 100, 10);
    let events = book.submit_limit(2, Side::Bid, 100, 10);

    assert!(has_trade(&events, 2, 1, 10));
    assert!(has_filled(&events, 1));
    assert!(has_filled(&events, 2));
    assert_eq!(book.order_count(), 0);
}

#[test]
fn partial_fill_aggressor_larger() {
    let mut book = new_book();
    book.submit_limit(1, Side::Ask, 100, 5);
    let events = book.submit_limit(2, Side::Bid, 100, 10);

    assert!(has_trade(&events, 2, 1, 5));
    assert!(has_filled(&events, 1));
    assert!(has_accepted(&events, 2));
    assert_eq!(book.order_count(), 1);

    let tob = book.top_of_book();
    assert_eq!(tob.best_bid.as_ref().unwrap().qty, 5);
    assert!(tob.best_ask.is_none());
}

#[test]
fn partial_fill_passive_larger() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    let events = book.submit_limit(2, Side::Ask, 100, 3);

    assert!(has_trade(&events, 2, 1, 3));
    assert!(has_filled(&events, 2));
    assert_eq!(book.order_count(), 1);

    let tob = book.top_of_book();
    assert_eq!(tob.best_bid.as_ref().unwrap().qty, 7);
}

#[test]
fn multi_level_sweep() {
    let mut book = new_book();
    book.submit_limit(1, Side::Ask, 100, 5);
    book.submit_limit(2, Side::Ask, 101, 5);
    book.submit_limit(3, Side::Ask, 102, 5);

    let events = book.submit_limit(4, Side::Bid, 102, 12);

    assert!(has_trade(&events, 4, 1, 5));
    assert!(has_trade(&events, 4, 2, 5));
    assert!(has_trade(&events, 4, 3, 2));
    assert!(has_filled(&events, 1));
    assert!(has_filled(&events, 2));

    assert!(has_filled(&events, 4));
    assert_eq!(book.order_count(), 1);

    let tob = book.top_of_book();
    assert_eq!(tob.best_ask.as_ref().unwrap().price, 102);
    assert_eq!(tob.best_ask.as_ref().unwrap().qty, 3);
}

#[test]
fn fifo_within_level() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    book.submit_limit(2, Side::Bid, 100, 10);
    book.submit_limit(3, Side::Bid, 100, 10);

    let events = book.submit_limit(4, Side::Ask, 100, 15);

    assert!(has_trade(&events, 4, 1, 10));
    assert!(has_trade(&events, 4, 2, 5));
    assert!(has_filled(&events, 1));
    assert!(events.iter().any(|e| matches!(e,
        Event::PartialFill { id: 2, remaining_qty: 5 }
    )));
    assert_eq!(book.order_count(), 2);
}

#[test]
fn price_improvement() {
    let mut book = new_book();
    book.submit_limit(1, Side::Ask, 100, 10);

    let events = book.submit_limit(2, Side::Bid, 105, 10);
    assert!(events.iter().any(|e| matches!(e,
        Event::Trade { price: 100, qty: 10, .. }
    )));
}

#[test]
fn no_cross_bid_below_ask() {
    let mut book = new_book();
    book.submit_limit(1, Side::Ask, 100, 10);
    let events = book.submit_limit(2, Side::Bid, 99, 10);

    assert!(!events.iter().any(|e| matches!(e, Event::Trade { .. })));
    assert!(has_accepted(&events, 2));
    assert_eq!(book.order_count(), 2);
}

#[test]
fn cancel_resting_order() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    let events = book.cancel(1);
    assert!(has_cancelled(&events, 1));
    assert_eq!(book.order_count(), 0);
    assert!(book.top_of_book().best_bid.is_none());
}

#[test]
fn cancel_unknown_id() {
    let mut book = new_book();
    let events = book.cancel(999);
    assert!(has_rejected(&events, 999));
}

#[test]
fn cancel_middle_of_level() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    book.submit_limit(2, Side::Bid, 100, 10);
    book.submit_limit(3, Side::Bid, 100, 10);

    book.cancel(2);
    assert_eq!(book.order_count(), 2);

    let events = book.submit_limit(4, Side::Ask, 100, 15);
    assert!(has_trade(&events, 4, 1, 10));
    assert!(has_trade(&events, 4, 3, 5));
}

#[test]
fn modify_qty_decrease() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    let events = book.modify(1, 5, None);
    assert!(events.iter().any(|e| matches!(e,
        Event::Modified { id: 1, new_qty: 5 }
    )));
    assert_eq!(book.top_of_book().best_bid.as_ref().unwrap().qty, 5);
    assert_eq!(book.order_count(), 1);
}

#[test]
fn modify_to_zero_cancels() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    let events = book.modify(1, 0, None);
    assert!(has_cancelled(&events, 1));
    assert_eq!(book.order_count(), 0);
}

#[test]
fn modify_price_change_cancel_replace() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    let events = book.modify(1, 10, Some(99));
    assert!(has_cancelled(&events, 1));
    assert!(has_accepted(&events, 1));
    assert_eq!(book.top_of_book().best_bid.as_ref().unwrap().price, 99);
}

#[test]
fn reject_zero_qty() {
    let mut book = new_book();
    let events = book.submit_limit(1, Side::Bid, 100, 0);
    assert!(has_rejected(&events, 1));
    assert_eq!(book.order_count(), 0);
}

#[test]
fn reject_duplicate_id() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    let events = book.submit_limit(1, Side::Ask, 200, 5);
    assert!(has_rejected(&events, 1));
    assert_eq!(book.order_count(), 1);
}

#[test]
fn snapshot_depth() {
    let mut book = new_book();
    book.submit_limit(1, Side::Bid, 100, 10);
    book.submit_limit(2, Side::Bid, 99, 20);
    book.submit_limit(3, Side::Ask, 101, 5);
    book.submit_limit(4, Side::Ask, 102, 15);

    let snap = book.snapshot(10);
    assert_eq!(snap.bids.len(), 2);
    assert_eq!(snap.asks.len(), 2);

    assert_eq!(snap.bids[0].price, 100);
    assert_eq!(snap.bids[1].price, 99);

    assert_eq!(snap.asks[0].price, 101);
    assert_eq!(snap.asks[1].price, 102);
}

#[test]
fn process_command_interface() {
    let mut book = new_book();
    let events = book.process(Command::NewOrder {
        id: 1,
        side: Side::Bid,
        price: 100,
        qty: 10,
    });
    assert!(has_accepted(&events, 1));

    let events = book.process(Command::Cancel { id: 1 });
    assert!(has_cancelled(&events, 1));
}
