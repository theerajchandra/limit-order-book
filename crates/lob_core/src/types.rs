use serde::{Deserialize, Serialize};
use std::fmt;

pub type Price = u64;
pub type Qty = u64;
pub type OrderId = u64;
pub type SeqNo = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Side {
    Bid,
    Ask,
}

impl fmt::Display for Side {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Side::Bid => write!(f, "BID"),
            Side::Ask => write!(f, "ASK"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Order {
    pub id: OrderId,
    pub side: Side,
    pub price: Price,
    pub qty: Qty,
    pub seq: SeqNo,
    pub prev: Option<usize>,
    pub next: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookLevel {
    pub price: Price,
    pub qty: Qty,
    pub order_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopOfBook {
    pub best_bid: Option<BookLevel>,
    pub best_ask: Option<BookLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BookSnapshot {
    pub bids: Vec<BookLevel>,
    pub asks: Vec<BookLevel>,
}
