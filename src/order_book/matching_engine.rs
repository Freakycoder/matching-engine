use crate::order_book::{
    orderbook::OrderBook,
    types::{
        BookDepth, CancelOutcome, EngineCancelOrder, EngineModifyOrder, EngineNewOrder, MatchOutcome, ModifyOutcome, OrderBookError, OrderNode, OrderType
    },
};
use anyhow::{anyhow};
use std::{collections::HashMap, time::Instant};

#[derive(Debug)]
pub struct MatchingEngine {
    _book: HashMap<u32, OrderBook>,
}

impl MatchingEngine {
    pub fn new() -> Self {
        Self {
            _book: HashMap::new(),
        }
    }

    fn get_orderbook(&mut self, security_id: u32) -> Option<&mut OrderBook> {
        let Some(book) = self._book.get_mut(&security_id) else {
            return None;
        };
        Some(book)
    }

    pub fn modify(
        &mut self,
        order_id: u64,
        security_id: u32,
        new_price: Option<u32>,
        new_qty: Option<u32>,
        is_buy_side: bool,
    ) -> Result<&'static str, OrderBookError> {
        let orderbook = self
            .get_orderbook(security_id);

        if orderbook.is_none(){
            return Err(OrderBookError::OrderBookNotFound);
        }
        if let Ok(potential_modfication) = orderbook.unwrap().modify_order(
            order_id,
            EngineModifyOrder {
                order_id,
                security_id,
                new_price,
                is_buy_side,
                new_quantity: new_qty,
            },
        ) {
            if let Some(modification_result) = potential_modfication {
                match modification_result {
                    ModifyOutcome::Both {
                        new_price,
                        new_initial_qty,
                        old_current_qty,
                    } => {
                        let _ = self.match_order(EngineNewOrder {
                            engine_order_id: order_id,
                            price: Some(new_price),
                            initial_quantity: new_initial_qty,
                            current_quantity: old_current_qty,
                            is_buy_side,
                            security_id,
                            order_type: OrderType::GoodTillCancel,
                        });
                        return Ok("Both");
                    }
                    ModifyOutcome::Repriced {
                        new_price,
                        old_initial_qty,
                        old_current_qty,
                    } => {
                        let _ = self.match_order(EngineNewOrder {
                            engine_order_id: order_id,
                            price: Some(new_price),
                            initial_quantity: old_initial_qty,
                            current_quantity: old_current_qty,
                            is_buy_side,
                            security_id,
                            order_type: OrderType::GoodTillCancel,
                        });
                        return Ok("Repriced");
                    }
                    ModifyOutcome::Requantized {
                        old_price,
                        new_initial_qty,
                        old_current_qty,
                    } => {
                        let _ = self.match_order(EngineNewOrder {
                            engine_order_id: order_id,
                            price: Some(old_price),
                            initial_quantity: new_initial_qty,
                            current_quantity: old_current_qty,
                            is_buy_side,
                            security_id,
                            order_type: OrderType::GoodTillCancel,
                        });
                        return Ok("Requantized");
                    }
                    ModifyOutcome::Inplace => return Ok("Inplace"),
                }
            }
            return Ok("No potential modification");
        } else {
            return Ok("No modification occured");
        }
    }

    pub fn cancel(
        &mut self,
        order_id: u64,
        security_id: u32,
        is_buy_side: bool,
    ) -> Result<CancelOutcome, OrderBookError> {
        let timer = Instant::now();
        let orderbook = self
            .get_orderbook(security_id);

        if orderbook.is_none(){
            return Err(OrderBookError::OrderBookNotFound);
        }
        if let Err(_) = orderbook.unwrap().cancel_order(
            order_id,
            EngineCancelOrder {
                is_buy_side,
                security_id,
                order_id,
            },
        ) {
            let elapsed_time = timer.elapsed().as_micros() as f64;
            return Ok(CancelOutcome::Failed(elapsed_time));
        };
        let elapsed_time = timer.elapsed().as_micros() as f64;
        return Ok(CancelOutcome::Success(elapsed_time));
    }

    pub fn depth(
        &self,
        security_id: u32,
        levels_count: Option<u32>,
    ) -> Result<BookDepth, anyhow::Error> {
        let Some(order_book) = self._book.get(&security_id) else {
            return Err(anyhow!("order not found"));
        };
        match order_book.depth(levels_count) {
            Ok(book_depth) => Ok(book_depth),
            Err(e) => Err(anyhow!("{}", e)),
        }
    }

    pub fn match_order(&mut self, order: EngineNewOrder) -> Result<MatchOutcome, OrderBookError> {
        let timer = Instant::now();

        let orderbook = match self._book.get_mut(&order.security_id) {
            Some(orderbook) => orderbook,
            None => self
                ._book
                .entry(order.security_id)
                .or_insert(OrderBook::new()),
        };

        if !order.is_buy_side {
            // for ASK order
            match order.order_type {
                OrderType::Market => {
                    // need to immediatly execute the order on the best of other half
                    let mut fill_quantity = order.initial_quantity;
                    let mut levels_consumed = 0;
                    let mut orders_touched = 0;
                    while fill_quantity > 0 {
                        let remove_node: bool;
                        {
                            let Some(mut price_node) = orderbook.bid.price_map.last_entry() else {
                                break;
                            };
                            let price_level = price_node.get_mut();
                            while price_level.total_quantity > 0 && fill_quantity > 0 {
                                if let Some(head_idx) = price_level.head {
                                    match orderbook.bid.order_pool[head_idx].as_mut() {
                                        Some(first_order_node) => {
                                            if fill_quantity >= first_order_node.current_quantity {
                                                fill_quantity -= first_order_node.current_quantity;

                                                price_level.total_quantity = price_level.total_quantity.checked_sub(first_order_node.current_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                let next = first_order_node.next;
                                                orderbook.bid.order_pool[head_idx] = None;
                                                orderbook.bid.free_list.push(head_idx);
                                                orders_touched += 1;
                                                if let Some(next_order_idx) = next {
                                                    price_level.head = Some(next_order_idx);
                                                } else {
                                                    price_level.total_quantity = 0;
                                                    price_level.head = None;
                                                    price_level.tail = None;
                                                    price_level.order_count = 0;
                                                    break;
                                                }
                                            } else {
                                                first_order_node.current_quantity =
                                                    first_order_node
                                                        .current_quantity
                                                        .checked_sub(fill_quantity)
                                                        .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(fill_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                fill_quantity = 0;
                                                orders_touched += 1;
                                            }
                                        }
                                        None => {
                                            return Err(OrderBookError::HeadNotFound);
                                        }
                                    };
                                } else {
                                    // price level has no head. i.e head = None
                                    break;
                                }
                            }
                            remove_node = price_level.total_quantity == 0;
                        }
                        if remove_node {
                            match orderbook.bid.price_map.pop_last() {
                                Some(_) => {
                                    levels_consumed += 1;
                                }
                                None => {
                                    break;
                                }
                            };
                        }
                    }
                    let elapsed_time = timer.elapsed().as_micros() as f64;
                    Ok(MatchOutcome {
                        order_index: None,
                        levels_consumed,
                        orders_touched,
                        timer: elapsed_time,
                    })
                }
                OrderType::ImmediateOrCancel(market_limit) => {
                    let mut fill_quantity = order.initial_quantity;
                    let mut levels_consumed = 0;
                    let mut orders_touched = 0;
                    while fill_quantity > 0 {
                        let remove_node: bool;
                        {
                            let Some(mut price_node) = orderbook.bid.price_map.last_entry() else {
                                break;
                            };

                            if market_limit > *price_node.key() {
                                break;
                            }

                            let price_level = price_node.get_mut();
                            while price_level.total_quantity > 0 && fill_quantity > 0 {
                                if let Some(head_idx) = price_level.head {
                                    match orderbook.bid.order_pool[head_idx].as_mut() {
                                        Some(first_order_node) => {
                                            if fill_quantity >= first_order_node.current_quantity {
                                                fill_quantity -= first_order_node.current_quantity;
                                                price_level.total_quantity = price_level.total_quantity.checked_sub(first_order_node.current_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                let next = first_order_node.next;
                                                orderbook.bid.order_pool[head_idx] = None;
                                                orderbook.bid.free_list.push(head_idx);
                                                orders_touched += 1;
                                                if let Some(next_order_idx) = next {
                                                    price_level.head = Some(next_order_idx);
                                                } else {
                                                    price_level.total_quantity = 0;
                                                    price_level.head = None;
                                                    price_level.tail = None;
                                                    price_level.order_count = 0;
                                                    break;
                                                }
                                            } else {
                                                first_order_node.current_quantity =
                                                    first_order_node
                                                        .current_quantity
                                                        .checked_sub(fill_quantity)
                                                        .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(fill_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                fill_quantity = 0;
                                                orders_touched += 1;
                                            }
                                        }
                                        None => {
                                            return Err(OrderBookError::HeadNotFound);
                                        }
                                    };
                                } else {
                                    break;
                                }
                            }
                            remove_node = price_level.total_quantity == 0;
                        }
                        if remove_node {
                            match orderbook.bid.price_map.pop_last() {
                                Some(_) => {
                                    levels_consumed += 1;
                                }
                                None => {
                                    break;
                                }
                            };
                        }
                    }
                    let elapsed_time = timer.elapsed().as_micros() as f64;
                    Ok(MatchOutcome {
                        order_index: None,
                        levels_consumed,
                        orders_touched,
                        timer: elapsed_time,
                    })
                }
                OrderType::GoodTillCancel => {
                    let mut fill_quantity = order.initial_quantity;
                    let mut levels_consumed = 0;
                    let mut orders_touched = 0;
                    while fill_quantity > 0 {
                        let remove_node: bool;
                        {
                            let Some(mut price_node) = orderbook.bid.price_map.last_entry() else {
                                break;
                            };

                            match order.price {
                                Some(price) => {
                                    if price > *price_node.key() {
                                        break;
                                    }
                                }
                                None => {
                                    return Err(OrderBookError::PriceNotFound);
                                }
                            }
                            let price_level = price_node.get_mut();
                            while price_level.total_quantity > 0 && fill_quantity > 0 {
                                if let Some(head_idx) = price_level.head {
                                    match orderbook.bid.order_pool[head_idx].as_mut() {
                                        Some(first_order_node) => {
                                            if fill_quantity >= first_order_node.current_quantity {
                                                fill_quantity -= first_order_node.current_quantity;
                                                price_level.total_quantity = price_level.total_quantity.checked_sub(first_order_node.current_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                let next = first_order_node.next;
                                                orderbook.bid.order_pool[head_idx] = None;
                                                orderbook.bid.free_list.push(head_idx);
                                                orders_touched += 1;
                                                if let Some(next_order_idx) = next {
                                                    price_level.head = Some(next_order_idx);
                                                } else {
                                                    price_level.total_quantity = 0;
                                                    price_level.head = None;
                                                    price_level.tail = None;
                                                    price_level.order_count = 0;
                                                    break;
                                                }
                                            } else {
                                                first_order_node.current_quantity =
                                                    first_order_node
                                                        .current_quantity
                                                        .checked_sub(fill_quantity)
                                                        .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(fill_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                fill_quantity = 0;
                                                orders_touched += 1;
                                            }
                                        }
                                        None => {
                                            return Err(OrderBookError::HeadNotFound);
                                        }
                                    };
                                } else {
                                    break;
                                }
                            }
                            remove_node = price_level.total_quantity == 0;
                        }
                        if remove_node {
                            match orderbook.bid.price_map.pop_last() {
                                Some(_) => {
                                    levels_consumed += 1;
                                }
                                None => {
                                    break;
                                }
                            };
                        }
                    }
                    if fill_quantity > 0 {
                        let alloted_index = orderbook.create_sell_order(OrderNode {
                            order_id: order.engine_order_id,
                            initial_quantity: order.initial_quantity,
                            current_quantity: fill_quantity,
                            market_limit: order.price.unwrap(),
                            next: None,
                            prev: None,
                        })?;
                        let elapsed_time = timer.elapsed().as_micros() as f64;
                        return Ok(MatchOutcome {
                            order_index: Some(alloted_index as u32),
                            levels_consumed,
                            orders_touched,
                            timer: elapsed_time,
                        });
                    }
                    let elapsed_time = timer.elapsed().as_micros() as f64;
                    Ok(MatchOutcome {
                        order_index: None,
                        levels_consumed,
                        orders_touched,
                        timer: elapsed_time,
                    })
                }
                OrderType::FillOrKill(limit_price) => {
                    let mut available_quantity: u32 = 0;
                    for (level_price, level) in orderbook.bid.price_map.iter().rev() {
                        if limit_price > *level_price {
                            break;
                        }
                        available_quantity =
                            available_quantity
                                .checked_add(level.total_quantity)
                                .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                        if available_quantity >= order.initial_quantity {
                            break;
                        }
                    }

                    if available_quantity < order.initial_quantity {
                        let elapsed_time = timer.elapsed().as_micros() as f64;
                        return Ok(MatchOutcome {
                            order_index: None,
                            levels_consumed: 0,
                            orders_touched: 0,
                            timer: elapsed_time,
                        });
                    }

                    let mut fill_quantity = order.initial_quantity;
                    let mut levels_consumed = 0;
                    let mut orders_touched = 0;

                    while fill_quantity > 0 {
                        let remove_node: bool;
                        {
                            let Some(mut price_node) = orderbook.bid.price_map.last_entry() else {
                                return Err(OrderBookError::PriceLevelEmpty);
                            };

                            if limit_price > *price_node.key() {
                                return Err(OrderBookError::UnexpectedReturn);
                            }

                            let price_level = price_node.get_mut();
                            while price_level.total_quantity > 0 && fill_quantity > 0 {
                                if let Some(head_idx) = price_level.head {
                                    match orderbook.bid.order_pool[head_idx].as_mut() {
                                        Some(first_order_node) => {
                                            if fill_quantity >= first_order_node.current_quantity {
                                                fill_quantity -= first_order_node.current_quantity;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(first_order_node.current_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                let next = first_order_node.next;
                                                orderbook.bid.order_pool[head_idx] = None;
                                                orderbook.bid.free_list.push(head_idx);
                                                orders_touched += 1;
                                                if let Some(next_order_idx) = next {
                                                    price_level.head = Some(next_order_idx);
                                                } else {
                                                    price_level.total_quantity = 0;
                                                    price_level.head = None;
                                                    price_level.tail = None;
                                                    price_level.order_count = 0;
                                                    break;
                                                }
                                            } else {
                                                first_order_node.current_quantity =
                                                    first_order_node
                                                        .current_quantity
                                                        .checked_sub(fill_quantity)
                                                        .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(fill_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                fill_quantity = 0;
                                                orders_touched += 1;
                                            }
                                        }
                                        None => {
                                            return Err(OrderBookError::HeadNotFound);
                                        }
                                    };
                                } else {
                                    break;
                                }
                            }
                            remove_node = price_level.total_quantity == 0;
                        }
                        if remove_node {
                            orderbook.bid.price_map.pop_last();
                            levels_consumed += 1;
                        }
                    }

                    let elapsed_time = timer.elapsed().as_micros() as f64;
                    Ok(MatchOutcome {
                        order_index: None,
                        levels_consumed,
                        orders_touched,
                        timer: elapsed_time,
                    })
                }
            }
        } else {
            match order.order_type {
                OrderType::Market => {
                    // need to immediatly execute the order on the best of other half
                    let mut fill_quantity = order.initial_quantity;
                    let mut levels_consumed = 0;
                    let mut orders_touched = 0;
                    while fill_quantity > 0 {
                        let remove_node: bool;
                        {
                            let Some(mut price_node) = orderbook.ask.price_map.first_entry() else {
                                break;
                            };
                            let price_level = price_node.get_mut();
                            while price_level.total_quantity > 0 && fill_quantity > 0 {
                                if let Some(head_idx) = price_level.head {
                                    match orderbook.ask.order_pool[head_idx].as_mut() {
                                        Some(first_order_node) => {
                                            if fill_quantity >= first_order_node.current_quantity {
                                                fill_quantity -= first_order_node.current_quantity;
                                                price_level.total_quantity = price_level.total_quantity.checked_sub(first_order_node.current_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                let next = first_order_node.next;
                                                orderbook.ask.order_pool[head_idx] = None;
                                                orderbook.ask.free_list.push(head_idx);
                                                orders_touched += 1;
                                                if let Some(next_order_idx) = next {
                                                    price_level.head = Some(next_order_idx);
                                                } else {
                                                    price_level.total_quantity = 0;
                                                    price_level.head = None;
                                                    price_level.tail = None;
                                                    price_level.order_count = 0;
                                                    break;
                                                }
                                            } else {
                                                first_order_node.current_quantity =
                                                    first_order_node
                                                        .current_quantity
                                                        .checked_sub(fill_quantity)
                                                        .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(fill_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                fill_quantity = 0;
                                                orders_touched += 1;
                                            }
                                        }
                                        None => {
                                            return Err(OrderBookError::HeadNotFound);
                                        }
                                    };
                                } else {
                                    break;
                                }
                            }
                            remove_node = price_level.total_quantity == 0;
                        }
                        if remove_node {
                            match orderbook.ask.price_map.pop_first() {
                                Some(_) => {
                                    levels_consumed += 1;
                                }
                                None => {
                                    break;
                                }
                            };
                        }
                    }
                    let elapsed_time = timer.elapsed().as_micros() as f64;
                    Ok(MatchOutcome {
                        order_index: None,
                        levels_consumed,
                        orders_touched,
                        timer: elapsed_time,
                    })
                }
                OrderType::ImmediateOrCancel(market_limit) => {
                    let mut fill_quantity = order.initial_quantity;
                    let mut levels_consumed = 0;
                    let mut orders_touched = 0;
                    while fill_quantity > 0 {
                        let remove_node: bool;
                        {
                            let Some(mut price_node) = orderbook.ask.price_map.first_entry() else {
                                break;
                            };
                            if market_limit < *price_node.key() {
                                break;
                            }

                            let price_level = price_node.get_mut();
                            while price_level.total_quantity > 0 && fill_quantity > 0 {
                                let head_pointer = price_level.head;
                                if let Some(head_idx) = head_pointer {
                                    match orderbook.ask.order_pool[head_idx].as_mut() {
                                        Some(first_order_node) => {
                                            if fill_quantity >= first_order_node.current_quantity {
                                                fill_quantity -= first_order_node.current_quantity;
                                                price_level.total_quantity = price_level.total_quantity.checked_sub(first_order_node.current_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                let next = first_order_node.next;
                                                orderbook.ask.order_pool[head_idx] = None;
                                                orderbook.ask.free_list.push(head_idx);
                                                orders_touched += 1;
                                                if let Some(next_order_idx) = next {
                                                    price_level.head = Some(next_order_idx);
                                                } else {
                                                    price_level.head = None;
                                                    price_level.total_quantity = 0;
                                                    price_level.head = None;
                                                    price_level.tail = None;
                                                    price_level.order_count = 0;
                                                    break;
                                                }
                                            } else {
                                                first_order_node.current_quantity =
                                                    first_order_node
                                                        .current_quantity
                                                        .checked_sub(fill_quantity)
                                                        .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(fill_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                fill_quantity = 0;
                                                orders_touched += 1;
                                            }
                                        }
                                        None => {
                                            return Err(OrderBookError::HeadNotFound);
                                        }
                                    };
                                } else {
                                    break;
                                }
                            }
                            remove_node = price_level.total_quantity == 0;
                        }
                        if remove_node {
                            match orderbook.ask.price_map.pop_first() {
                                Some(_) => {
                                    levels_consumed += 1;
                                }
                                None => {
                                    break;
                                }
                            };
                        }
                    }
                    let elapsed_time = timer.elapsed().as_micros() as f64;
                    Ok(MatchOutcome {
                        order_index: None,
                        levels_consumed,
                        orders_touched,
                        timer: elapsed_time,
                    })
                }
                OrderType::GoodTillCancel => {
                    let mut fill_quantity = order.initial_quantity;
                    let mut levels_consumed = 0;
                    let mut orders_touched = 0;
                    while fill_quantity > 0 {
                        let remove_node: bool;
                        {
                            let Some(mut price_node) = orderbook.ask.price_map.first_entry() else {
                                break;
                            };

                            match order.price {
                                Some(price) => {
                                    if price < *price_node.key() {
                                        break;
                                    }
                                }
                                None => {
                                    return Err(OrderBookError::PriceNotFound);
                                }
                            }
                            let price_level = price_node.get_mut();
                            while price_level.total_quantity > 0 && fill_quantity > 0 {
                                if let Some(head_idx) = price_level.head {
                                    match orderbook.ask.order_pool[head_idx].as_mut() {
                                        Some(first_order_node) => {
                                            if fill_quantity >= first_order_node.current_quantity {
                                                fill_quantity -= first_order_node.current_quantity;
                                                price_level.total_quantity = price_level.total_quantity.checked_sub(first_order_node.current_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                let next = first_order_node.next;
                                                orderbook.ask.order_pool[head_idx] = None;
                                                orderbook.ask.free_list.push(head_idx);
                                                orders_touched += 1;
                                                if let Some(next_order_idx) = next {
                                                    price_level.head = Some(next_order_idx);
                                                } else {
                                                    price_level.total_quantity = 0;
                                                    price_level.head = None;
                                                    price_level.tail = None;
                                                    price_level.order_count = 0;
                                                    break;
                                                }
                                            } else {
                                                first_order_node.current_quantity =
                                                    first_order_node
                                                        .current_quantity
                                                        .checked_sub(fill_quantity)
                                                        .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(fill_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                fill_quantity = 0;
                                                orders_touched += 1;
                                            }
                                        }
                                        None => {
                                            return Err(OrderBookError::HeadNotFound);
                                        }
                                    };
                                } else {
                                    break;
                                }
                            }
                            remove_node = price_level.total_quantity == 0;
                        }
                        if remove_node {
                            match orderbook.ask.price_map.pop_first() {
                                Some(_) => {
                                    levels_consumed += 1;
                                }
                                None => {
                                    break;
                                }
                            };
                        }
                    }
                    if fill_quantity > 0 {
                        let alloted_index = orderbook.create_buy_order(OrderNode {
                            order_id: order.engine_order_id,
                            initial_quantity: order.initial_quantity,
                            current_quantity: fill_quantity,
                            market_limit: order.price.unwrap(),
                            next: None,
                            prev: None,
                        })?;

                        let elapsed_time = timer.elapsed().as_micros() as f64;
                        return Ok(MatchOutcome {
                            order_index: Some(alloted_index as u32),
                            levels_consumed,
                            orders_touched,
                            timer: elapsed_time,
                        });
                    }
                    let elapsed_time = timer.elapsed().as_micros() as f64;
                    Ok(MatchOutcome {
                        order_index: None,
                        levels_consumed,
                        orders_touched,
                        timer: elapsed_time,
                    })
                }
                OrderType::FillOrKill(limit_price) => {
                    let mut available_quantity: u32 = 0;
                    for (level_price, level) in orderbook.ask.price_map.iter() {
                        if limit_price < *level_price {
                            break;
                        }
                        available_quantity =
                            available_quantity
                                .checked_add(level.total_quantity)
                                .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                        if available_quantity >= order.initial_quantity {
                            break;
                        }
                    }

                    if available_quantity < order.initial_quantity {
                        let elapsed_time = timer.elapsed().as_micros() as f64;
                        return Ok(MatchOutcome {
                            order_index: None,
                            levels_consumed: 0,
                            orders_touched: 0,
                            timer: elapsed_time,
                        });
                    }

                    let mut fill_quantity = order.initial_quantity;
                    let mut levels_consumed = 0;
                    let mut orders_touched = 0;

                    while fill_quantity > 0 {
                        let remove_node: bool;
                        {
                            let Some(mut price_node) = orderbook.ask.price_map.first_entry() else {
                                return Err(OrderBookError::PriceLevelEmpty);
                            };

                            if limit_price < *price_node.key() {
                                return Err(OrderBookError::UnexpectedReturn);
                            }

                            let price_level = price_node.get_mut();
                            while price_level.total_quantity > 0 && fill_quantity > 0 {
                                if let Some(head_idx) = price_level.head {
                                    match orderbook.ask.order_pool[head_idx].as_mut() {
                                        Some(first_order_node) => {
                                            if fill_quantity >= first_order_node.current_quantity {
                                                fill_quantity -= first_order_node.current_quantity;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(first_order_node.current_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                let next = first_order_node.next;
                                                orderbook.ask.order_pool[head_idx] = None;
                                                orderbook.ask.free_list.push(head_idx);
                                                orders_touched += 1;
                                                if let Some(next_order_idx) = next {
                                                    price_level.head = Some(next_order_idx);
                                                } else {
                                                    price_level.total_quantity = 0;
                                                    price_level.head = None;
                                                    price_level.tail = None;
                                                    price_level.order_count = 0;
                                                    break;
                                                }
                                            } else {
                                                first_order_node.current_quantity =
                                                    first_order_node
                                                        .current_quantity
                                                        .checked_sub(fill_quantity)
                                                        .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                price_level.total_quantity = price_level
                                                    .total_quantity
                                                    .checked_sub(fill_quantity)
                                                    .ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                                fill_quantity = 0;
                                                orders_touched += 1;
                                            }
                                        }
                                        None => {
                                            return Err(OrderBookError::HeadNotFound);
                                        }
                                    };
                                } else {
                                    break;
                                }
                            }
                            remove_node = price_level.total_quantity == 0;
                        }
                        if remove_node {
                            orderbook.ask.price_map.pop_first();
                            levels_consumed += 1;
                        }
                    }

                    let elapsed_time = timer.elapsed().as_micros() as f64;
                    Ok(MatchOutcome {
                        order_index: None,
                        levels_consumed,
                        orders_touched,
                        timer: elapsed_time,
                    })
                }
            }
        }
    }
}
