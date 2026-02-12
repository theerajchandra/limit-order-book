use crate::slab::OrderSlab;
use crate::types::{BookLevel, Price, Qty};

#[derive(Debug)]
pub struct PriceLevel {
    pub price: Price,
    pub total_qty: Qty,
    pub order_count: usize,
    head: Option<usize>,
    tail: Option<usize>,
}

impl PriceLevel {
    pub fn new(price: Price) -> Self {
        Self {
            price,
            total_qty: 0,
            order_count: 0,
            head: None,
            tail: None,
        }
    }

    pub fn push_back(&mut self, slab: &mut OrderSlab, idx: usize) {
        let order = slab.get(idx).expect("push_back: invalid slab index");
        let qty = order.qty;

        if let Some(old_tail) = self.tail {
            slab.get_mut(old_tail).expect("broken tail").next = Some(idx);
            slab.get_mut(idx).expect("missing new order").prev = Some(old_tail);
        } else {
            self.head = Some(idx);
        }
        self.tail = Some(idx);
        self.total_qty += qty;
        self.order_count += 1;
    }

    pub fn pop_front(&mut self, slab: &mut OrderSlab) -> Option<usize> {
        let head_idx = self.head?;
        self.unlink(slab, head_idx);
        Some(head_idx)
    }

    pub fn unlink(&mut self, slab: &mut OrderSlab, idx: usize) {
        let order = slab.get(idx).expect("unlink: invalid slab index");
        let prev = order.prev;
        let next = order.next;
        let qty = order.qty;

        if let Some(p) = prev {
            slab.get_mut(p).expect("broken prev link").next = next;
        } else {
            self.head = next;
        }
        if let Some(n) = next {
            slab.get_mut(n).expect("broken next link").prev = prev;
        } else {
            self.tail = prev;
        }

        let order = slab.get_mut(idx).expect("missing order");
        order.prev = None;
        order.next = None;

        self.total_qty -= qty;
        self.order_count -= 1;
    }

    #[inline]
    pub fn front(&self) -> Option<usize> {
        self.head
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.order_count == 0
    }

    pub fn snapshot(&self) -> BookLevel {
        BookLevel {
            price: self.price,
            qty: self.total_qty,
            order_count: self.order_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Order, Side};

    fn make_order(id: u64, qty: u64) -> Order {
        Order {
            id,
            side: Side::Bid,
            price: 100,
            qty,
            seq: id,
            prev: None,
            next: None,
        }
    }

    #[test]
    fn push_and_pop_fifo() {
        let mut slab = OrderSlab::with_capacity(8);
        let mut level = PriceLevel::new(100);

        let i0 = slab.insert(make_order(1, 10));
        let i1 = slab.insert(make_order(2, 20));
        let i2 = slab.insert(make_order(3, 30));

        level.push_back(&mut slab, i0);
        level.push_back(&mut slab, i1);
        level.push_back(&mut slab, i2);

        assert_eq!(level.total_qty, 60);
        assert_eq!(level.order_count, 3);

        assert_eq!(level.pop_front(&mut slab), Some(i0));
        assert_eq!(level.total_qty, 50);

        assert_eq!(level.pop_front(&mut slab), Some(i1));
        assert_eq!(level.pop_front(&mut slab), Some(i2));
        assert!(level.is_empty());
        assert_eq!(level.pop_front(&mut slab), None);
    }

    #[test]
    fn unlink_middle() {
        let mut slab = OrderSlab::with_capacity(8);
        let mut level = PriceLevel::new(100);

        let i0 = slab.insert(make_order(1, 10));
        let i1 = slab.insert(make_order(2, 20));
        let i2 = slab.insert(make_order(3, 30));

        level.push_back(&mut slab, i0);
        level.push_back(&mut slab, i1);
        level.push_back(&mut slab, i2);

        level.unlink(&mut slab, i1);
        assert_eq!(level.total_qty, 40);
        assert_eq!(level.order_count, 2);

        assert_eq!(level.pop_front(&mut slab), Some(i0));
        assert_eq!(level.pop_front(&mut slab), Some(i2));
        assert!(level.is_empty());
    }
}
