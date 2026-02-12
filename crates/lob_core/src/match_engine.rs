use crate::events::Event;
use crate::side::BookSide;
use crate::slab::OrderSlab;
use crate::types::{OrderId, Price, Qty, Side};

pub struct MatchResult {
    pub remaining_qty: Qty,
    pub events: Vec<Event>,
}

pub fn try_match(
    aggressor_id: OrderId,
    aggressor_side: Side,
    aggressor_price: Price,
    mut aggressor_qty: Qty,
    contra_side: &mut BookSide,
    slab: &mut OrderSlab,
) -> MatchResult {
    let mut events: Vec<Event> = Vec::new();

    while aggressor_qty > 0 {
        let contra_price = match contra_side.best_price() {
            Some(p) => p,
            None => break,
        };

        let crosses = match aggressor_side {
            Side::Bid => aggressor_price >= contra_price,
            Side::Ask => aggressor_price <= contra_price,
        };
        if !crosses {
            break;
        }

        let level = contra_side
            .best_level_mut()
            .expect("best_price returned Some but no level");

        while aggressor_qty > 0 {
            let passive_idx = match level.front() {
                Some(idx) => idx,
                None => break,
            };

            let passive = slab.get(passive_idx).expect("dangling slab index");
            let passive_id = passive.id;
            let trade_price = passive.price;
            let passive_qty = passive.qty;

            let fill_qty = aggressor_qty.min(passive_qty);

            events.push(Event::Trade {
                aggressor_id,
                passive_id,
                price: trade_price,
                qty: fill_qty,
                side: aggressor_side,
            });

            aggressor_qty -= fill_qty;

            if fill_qty == passive_qty {
                level.pop_front(slab);
                events.push(Event::Filled { id: passive_id });
                slab.remove(passive_idx);
            } else {
                let remaining = passive_qty - fill_qty;
                level.total_qty -= fill_qty;
                let order = slab.get_mut(passive_idx).expect("missing passive order");
                order.qty = remaining;
                events.push(Event::PartialFill {
                    id: passive_id,
                    remaining_qty: remaining,
                });
            }
        }

        contra_side.remove_if_empty(contra_price);
    }

    MatchResult {
        remaining_qty: aggressor_qty,
        events,
    }
}
