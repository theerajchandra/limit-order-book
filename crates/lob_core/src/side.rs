use std::collections::BTreeMap;

use crate::level::PriceLevel;
use crate::types::{BookLevel, Price, Side};

#[derive(Debug)]
pub struct BookSide {
    side: Side,
    levels: BTreeMap<Price, PriceLevel>,
}

impl BookSide {
    pub fn new(side: Side) -> Self {
        Self {
            side,
            levels: BTreeMap::new(),
        }
    }

    pub fn get_or_create_level(&mut self, price: Price) -> &mut PriceLevel {
        self.levels
            .entry(price)
            .or_insert_with(|| PriceLevel::new(price))
    }

    pub fn remove_if_empty(&mut self, price: Price) {
        if let std::collections::btree_map::Entry::Occupied(e) = self.levels.entry(price) {
            if e.get().is_empty() {
                e.remove();
            }
        }
    }

    pub fn best_price(&self) -> Option<Price> {
        match self.side {
            Side::Bid => self.levels.keys().next_back().copied(),
            Side::Ask => self.levels.keys().next().copied(),
        }
    }

    pub fn best_level_mut(&mut self) -> Option<&mut PriceLevel> {
        match self.side {
            Side::Bid => self.levels.values_mut().next_back(),
            Side::Ask => self.levels.values_mut().next(),
        }
    }

    pub fn best_level(&self) -> Option<&PriceLevel> {
        match self.side {
            Side::Bid => self.levels.values().next_back(),
            Side::Ask => self.levels.values().next(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }

    pub fn snapshot(&self, depth: usize) -> Vec<BookLevel> {
        match self.side {
            Side::Bid => self
                .levels
                .values()
                .rev()
                .take(depth)
                .map(|l| l.snapshot())
                .collect(),
            Side::Ask => self
                .levels
                .values()
                .take(depth)
                .map(|l| l.snapshot())
                .collect(),
        }
    }

    pub fn remove_level(&mut self, price: Price) {
        self.levels.remove(&price);
    }

    pub fn get_level_mut(&mut self, price: Price) -> Option<&mut PriceLevel> {
        self.levels.get_mut(&price)
    }

    #[inline]
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }
}
