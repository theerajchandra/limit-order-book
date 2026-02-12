use crate::types::Order;

#[derive(Debug)]
enum Slot {
    Occupied(Order),
    Vacant { next_free: Option<usize> },
}

#[derive(Debug)]
pub struct OrderSlab {
    entries: Vec<Slot>,
    free_head: Option<usize>,
    len: usize,
}

impl OrderSlab {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Vec::with_capacity(capacity),
            free_head: None,
            len: 0,
        }
    }

    pub fn insert(&mut self, order: Order) -> usize {
        self.len += 1;
        if let Some(idx) = self.free_head {
            match &self.entries[idx] {
                Slot::Vacant { next_free } => {
                    self.free_head = *next_free;
                }
                Slot::Occupied(_) => unreachable!(),
            }
            self.entries[idx] = Slot::Occupied(order);
            idx
        } else {
            let idx = self.entries.len();
            self.entries.push(Slot::Occupied(order));
            idx
        }
    }

    pub fn remove(&mut self, idx: usize) -> Order {
        debug_assert!(idx < self.entries.len());
        let old = std::mem::replace(
            &mut self.entries[idx],
            Slot::Vacant {
                next_free: self.free_head,
            },
        );
        self.free_head = Some(idx);
        self.len -= 1;
        match old {
            Slot::Occupied(order) => order,
            Slot::Vacant { .. } => panic!("tried to remove a vacant slab slot"),
        }
    }

    #[inline]
    pub fn get(&self, idx: usize) -> Option<&Order> {
        match self.entries.get(idx)? {
            Slot::Occupied(order) => Some(order),
            Slot::Vacant { .. } => None,
        }
    }

    #[inline]
    pub fn get_mut(&mut self, idx: usize) -> Option<&mut Order> {
        match self.entries.get_mut(idx)? {
            Slot::Occupied(order) => Some(order),
            Slot::Vacant { .. } => None,
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Side;

    fn make_order(id: u64) -> Order {
        Order {
            id,
            side: Side::Bid,
            price: 100,
            qty: 10,
            seq: id,
            prev: None,
            next: None,
        }
    }

    #[test]
    fn insert_and_get() {
        let mut slab = OrderSlab::with_capacity(4);
        let i0 = slab.insert(make_order(1));
        let i1 = slab.insert(make_order(2));
        assert_eq!(slab.get(i0).unwrap().id, 1);
        assert_eq!(slab.get(i1).unwrap().id, 2);
        assert_eq!(slab.len(), 2);
    }

    #[test]
    fn remove_and_reuse() {
        let mut slab = OrderSlab::with_capacity(4);
        let i0 = slab.insert(make_order(1));
        let _i1 = slab.insert(make_order(2));
        let removed = slab.remove(i0);
        assert_eq!(removed.id, 1);
        assert_eq!(slab.len(), 1);
        assert!(slab.get(i0).is_none());

        let i2 = slab.insert(make_order(3));
        assert_eq!(i2, i0);
        assert_eq!(slab.get(i2).unwrap().id, 3);
    }

    #[test]
    #[should_panic]
    fn remove_vacant_panics() {
        let mut slab = OrderSlab::with_capacity(4);
        let i = slab.insert(make_order(1));
        slab.remove(i);
        slab.remove(i);
    }
}
