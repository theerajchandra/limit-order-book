use std::collections::HashMap;

use crate::events::{Command, Event};
use crate::match_engine;
use crate::side::BookSide;
use crate::slab::OrderSlab;
use crate::types::*;

#[derive(Debug, Clone, Copy)]
struct OrderHandle {
    slab_idx: usize,
    price: Price,
    side: Side,
}

#[derive(Debug, Clone)]
pub struct OrderBookConfig {
    pub initial_capacity: usize,
}

impl Default for OrderBookConfig {
    fn default() -> Self {
        Self {
            initial_capacity: 1 << 16,
        }
    }
}

pub struct OrderBook {
    bids: BookSide,
    asks: BookSide,
    slab: OrderSlab,
    index: HashMap<OrderId, OrderHandle>,
    next_seq: SeqNo,
}

impl OrderBook {
    pub fn new(config: OrderBookConfig) -> Self {
        Self {
            bids: BookSide::new(Side::Bid),
            asks: BookSide::new(Side::Ask),
            slab: OrderSlab::with_capacity(config.initial_capacity),
            index: HashMap::with_capacity(config.initial_capacity),
            next_seq: 0,
        }
    }

    pub fn process(&mut self, cmd: Command) -> Vec<Event> {
        match cmd {
            Command::NewOrder {
                id,
                side,
                price,
                qty,
            } => self.submit_limit(id, side, price, qty),
            Command::Cancel { id } => self.cancel(id),
            Command::Modify {
                id,
                new_qty,
                new_price,
            } => self.modify(id, new_qty, new_price),
        }
    }

    pub fn submit_limit(
        &mut self,
        id: OrderId,
        side: Side,
        price: Price,
        qty: Qty,
    ) -> Vec<Event> {
        if qty == 0 {
            return vec![Event::Rejected {
                id,
                reason: "qty must be > 0",
            }];
        }
        if self.index.contains_key(&id) {
            return vec![Event::Rejected {
                id,
                reason: "duplicate order id",
            }];
        }

        let mut events = Vec::new();

        let contra = match side {
            Side::Bid => &mut self.asks,
            Side::Ask => &mut self.bids,
        };
        let match_result =
            match_engine::try_match(id, side, price, qty, contra, &mut self.slab);

        for event in &match_result.events {
            if let Event::Filled { id: passive_id } = event {
                self.index.remove(passive_id);
            }
        }

        events.extend(match_result.events);

        let remaining = match_result.remaining_qty;

        if remaining == 0 {
            events.push(Event::Filled { id });
            return events;
        }

        let seq = self.next_seq;
        self.next_seq += 1;

        let order = Order {
            id,
            side,
            price,
            qty: remaining,
            seq,
            prev: None,
            next: None,
        };

        let slab_idx = self.slab.insert(order);

        let book_side = match side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };
        let level = book_side.get_or_create_level(price);
        level.push_back(&mut self.slab, slab_idx);

        self.index.insert(
            id,
            OrderHandle {
                slab_idx,
                price,
                side,
            },
        );

        if remaining < qty {
            events.push(Event::PartialFill {
                id,
                remaining_qty: remaining,
            });
        }
        events.push(Event::Accepted { id });

        events
    }

    pub fn cancel(&mut self, id: OrderId) -> Vec<Event> {
        let handle = match self.index.remove(&id) {
            Some(h) => h,
            None => {
                return vec![Event::Rejected {
                    id,
                    reason: "unknown order id",
                }]
            }
        };

        let book_side = match handle.side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };

        if let Some(level) = book_side.get_level_mut(handle.price) {
            level.unlink(&mut self.slab, handle.slab_idx);
            if level.is_empty() {
                book_side.remove_level(handle.price);
            }
        }

        self.slab.remove(handle.slab_idx);

        vec![Event::Cancelled { id }]
    }

    pub fn modify(
        &mut self,
        id: OrderId,
        new_qty: Qty,
        new_price: Option<Price>,
    ) -> Vec<Event> {
        if new_qty == 0 {
            return self.cancel(id);
        }

        let handle = match self.index.get(&id) {
            Some(h) => *h,
            None => {
                return vec![Event::Rejected {
                    id,
                    reason: "unknown order id",
                }]
            }
        };

        let price_change = new_price.is_some_and(|p| p != handle.price);
        let order = self.slab.get(handle.slab_idx).expect("dangling handle");
        let qty_increase = new_qty > order.qty;

        if price_change || qty_increase {
            let side = handle.side;
            let final_price = new_price.unwrap_or(handle.price);
            let mut events = self.cancel(id);
            events.extend(self.submit_limit(id, side, final_price, new_qty));
            return events;
        }

        let old_qty = order.qty;
        let delta = old_qty - new_qty;

        let order = self.slab.get_mut(handle.slab_idx).expect("dangling handle");
        order.qty = new_qty;

        let book_side = match handle.side {
            Side::Bid => &mut self.bids,
            Side::Ask => &mut self.asks,
        };
        if let Some(level) = book_side.get_level_mut(handle.price) {
            level.total_qty -= delta;
        }

        vec![Event::Modified { id, new_qty }]
    }

    pub fn top_of_book(&self) -> TopOfBook {
        TopOfBook {
            best_bid: self.bids.best_level().map(|l| l.snapshot()),
            best_ask: self.asks.best_level().map(|l| l.snapshot()),
        }
    }

    pub fn snapshot(&self, depth: usize) -> BookSnapshot {
        BookSnapshot {
            bids: self.bids.snapshot(depth),
            asks: self.asks.snapshot(depth),
        }
    }

    #[inline]
    pub fn order_count(&self) -> usize {
        self.index.len()
    }

    #[inline]
    pub fn bid_level_count(&self) -> usize {
        self.bids.level_count()
    }

    #[inline]
    pub fn ask_level_count(&self) -> usize {
        self.asks.level_count()
    }
}

impl OrderBook {
    #[cfg(debug_assertions)]
    pub fn assert_not_crossed(&self) {
        if let (Some(bid), Some(ask)) = (self.bids.best_price(), self.asks.best_price()) {
            debug_assert!(
                bid < ask,
                "crossed book! best bid {} >= best ask {}",
                bid,
                ask
            );
        }
    }
}
