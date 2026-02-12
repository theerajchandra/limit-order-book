use serde::{Deserialize, Serialize};

use crate::types::{OrderId, Price, Qty, Side};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Command {
    NewOrder {
        id: OrderId,
        side: Side,
        price: Price,
        qty: Qty,
    },
    Cancel {
        id: OrderId,
    },
    Modify {
        id: OrderId,
        new_qty: Qty,
        new_price: Option<Price>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Event {
    Accepted {
        id: OrderId,
    },
    Trade {
        aggressor_id: OrderId,
        passive_id: OrderId,
        price: Price,
        qty: Qty,
        side: Side,
    },
    Filled {
        id: OrderId,
    },
    PartialFill {
        id: OrderId,
        remaining_qty: Qty,
    },
    Cancelled {
        id: OrderId,
    },
    Modified {
        id: OrderId,
        new_qty: Qty,
    },
    Rejected {
        id: OrderId,
        reason: &'static str,
    },
}
