use std::{collections::{BTreeMap, HashMap, btree_map::Entry}, time::Instant};
use crate::order_book::types::{BookDepth, EngineCancelOrder, EngineModifyOrder, ModifyOutcome, OrderBookError, OrderNode, PriceLevel, PriceLevelDepth};

#[derive(Debug)]
pub struct OrderBook{
    pub ask : HalfBook,
    pub bid : HalfBook
}
impl OrderBook {
    pub fn new () -> Self{
        Self { ask : HalfBook::new(), bid : HalfBook::new() }
    }

    pub fn create_buy_order(&mut self, resting_order : OrderNode) -> Result<usize, OrderBookError>{
        
        let mut order = resting_order;
        let order_quantity = order.current_quantity;
        let price = order.market_limit;
        let order_id = resting_order.order_id;

        match self.bid.price_map.entry(price){ // here price is not moved, bcoz u32 implements Copy
            Entry::Occupied(mut entry) => {
                let price_level = entry.get_mut();
                if price_level.total_quantity == 0 || price_level.head == None || price_level.tail == None{
                    entry.remove();
                } else {
                     order.prev = price_level.tail;
                if let Some(free_index) = self.bid.free_list.pop(){
                    self.bid.order_registry.insert(order_id, free_index);
                    self.bid.order_pool[free_index] = Some(order);
                    let prev_tail_idx = price_level.tail.unwrap();
                    price_level.tail = Some(free_index);
                    price_level.total_quantity += order_quantity;
                    price_level.order_count += 1;
                        if let Some(prev_order) = self.bid.order_pool.get_mut(prev_tail_idx).unwrap(){
                            prev_order.next = Some(free_index);
                        };
                    return Ok(free_index);
                }
                else {
                self.bid.order_pool.push(Some(order));
                let new_tail = self.bid.order_pool.len() - 1;
                self.bid.order_registry.insert(order_id, new_tail);
                let pre_tail_idx = price_level.tail.unwrap();
                price_level.tail = Some(new_tail);
                price_level.total_quantity += order_quantity;
                price_level.order_count += 1;
                if let Some(prev_order) = self.bid.order_pool.get_mut(pre_tail_idx).unwrap(){
                    prev_order.next = Some(new_tail);
                };
                return Ok(new_tail);
                }
                }
            }
            Entry::Vacant(_) => {
                // it means price_level doesn't exist. we create below
            }
        }

        let mut new_price_level = PriceLevel{
            head : None,
            tail : None,
            order_count : 0,
            total_quantity : 0
        };
        if let Some(free_index) = self.bid.free_list.pop(){
            self.bid.order_registry.insert(order_id, free_index);
            self.bid.order_pool[free_index] = Some(order);
            new_price_level.head = Some(free_index);
            new_price_level.tail = Some(free_index);
            new_price_level.order_count += 1;
            new_price_level.total_quantity += order_quantity;
            self.bid.price_map.entry(price).or_insert(new_price_level);
            return Ok(free_index)
        }
        self.bid.order_pool.push(Some(order));
        let new_index = self.bid.order_pool.len()-1;
        self.bid.order_registry.insert(order_id, new_index);
        new_price_level.head = Some(new_index);
        new_price_level.tail = Some(new_index);
        new_price_level.order_count += 1;
        new_price_level.total_quantity += order_quantity;
        self.bid.price_map.entry(price).or_insert(new_price_level);
        
        Ok(new_index)
    }

    pub fn create_sell_order(&mut self, resting_order : OrderNode) -> Result<usize, OrderBookError>{
        let mut order = resting_order;
        let order_quantity = order.current_quantity;
        let price = order.market_limit;
        let order_id = resting_order.order_id;

        match self.ask.price_map.entry(price){
            Entry::Occupied(mut entry) => {
                let price_level = entry.get_mut();
                if price_level.total_quantity == 0 || price_level.head == None || price_level.tail == None{
                    entry.remove();
                }else {
                    order.prev = price_level.tail;
                if let Some(free_index) = self.ask.free_list.pop(){
                    self.ask.order_registry.insert(order_id, free_index);
                    self.ask.order_pool[free_index] = Some(order);
                    let prev_tail_idx = price_level.tail.unwrap();
                    price_level.tail = Some(free_index);
                    price_level.total_quantity += order_quantity;
                    price_level.order_count += 1;
                    if let Some(prev_order) = self.ask.order_pool.get_mut(prev_tail_idx).unwrap(){
                        prev_order.next = Some(free_index);
                    };
                    return Ok(free_index);
                }
                else {
                self.ask.order_pool.push(Some(order));
                let new_tail = self.ask.order_pool.len() - 1;
                self.ask.order_registry.insert(order_id, new_tail);
                let prev_tail_idx = price_level.tail.unwrap();
                price_level.tail = Some(new_tail);
                price_level.total_quantity += order_quantity;
                price_level.order_count += 1;
                if let Some(prev_order) = self.ask.order_pool.get_mut(prev_tail_idx).unwrap(){
                    prev_order.next = Some(new_tail);
                };
                return Ok(new_tail);
                }
                }
            }
            Entry::Vacant(_) => {
                //do nothing
            }
        }
        
        let mut new_price_level = PriceLevel{
            head : None,
            tail : None,
            order_count : 0,
            total_quantity : 0
        };
        if let Some(free_index) = self.ask.free_list.pop(){
            self.ask.order_registry.insert(order_id, free_index);
            self.ask.order_pool[free_index] = Some(order);
            new_price_level.head = Some(free_index);
            new_price_level.tail = Some(free_index);
            new_price_level.order_count += 1;
            new_price_level.total_quantity += order_quantity;
            self.ask.price_map.entry(price).or_insert(new_price_level);
            return Ok(free_index)
        }
        self.ask.order_pool.push(Some(order));
        let new_index = self.ask.order_pool.len()-1;
        self.ask.order_registry.insert(order_id, new_index);
        new_price_level.head = Some(new_index);
        new_price_level.tail = Some(new_index);
        new_price_level.order_count += 1;
        new_price_level.total_quantity += order_quantity;
        self.ask.price_map.entry(price).or_insert(new_price_level);
        
        Ok(new_index)
    }

    pub fn cancel_order(&mut self, order_id : u64, order : EngineCancelOrder) -> Result<(), OrderBookError>{
        if order.is_buy_side {
            let existing_index = self.bid.order_registry.get(&order_id);
            if existing_index.is_none(){
                return Err(OrderBookError::OrderNotFound);
            }
                            let (prev, next, old_price, old_quantity) = {
                                match self.bid.order_pool[*existing_index.unwrap()].as_ref(){
                                    Some(node) => {
                                        (node.prev, node.next, node.market_limit, node.current_quantity)
                                    }
                                    None => { 
                                        return Err(OrderBookError::OrderNotFound);
                                    }
                                }
                            };
                    if let Some(price_level) = self.bid.price_map.get_mut(&old_price){

                        if price_level.head.is_some() && price_level.tail.is_some(){
                            if *existing_index.unwrap() == price_level.head.unwrap() && *existing_index.unwrap() == price_level.tail.unwrap(){
                                self.bid.order_pool[*existing_index.unwrap()] = None;
                                price_level.head = None;
                                price_level.tail = None;
                                price_level.total_quantity = price_level.total_quantity.checked_sub(old_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                price_level.order_count = price_level.order_count.checked_sub(1).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                self.bid.free_list.push(*existing_index.unwrap());
                                return Ok(());
                            }
                            else if *existing_index.unwrap() == price_level.tail.unwrap() {
                               if let Some(prev_index) = prev{
                                    if let Some(possible_prev_node) = self.bid.order_pool.get_mut(prev_index){
                                        if let Some(prev_node) = possible_prev_node{
                                            prev_node.next = None;
                                            price_level.tail = Some(prev_index);
                                        }           
                                    }
                                self.bid.order_pool[*existing_index.unwrap()] = None;
                                price_level.total_quantity = price_level.total_quantity.checked_sub(old_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                price_level.order_count = price_level.order_count.checked_sub(1).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                self.bid.free_list.push(*existing_index.unwrap());
                                return Ok(()); 
                                } else {
                                    return Err(OrderBookError::PrevNotFound);
                                }
                            }
                            else if *existing_index.unwrap() == price_level.head.unwrap() {
                                if let Some(next_index) = next{
                                    if let Some(possible_next_node) = self.bid.order_pool.get_mut(next_index){
                                        if let Some(next_node) = possible_next_node{
                                            next_node.prev = None;
                                            price_level.head = Some(next_index);
                                        }
                                    }
                                    self.bid.order_pool[*existing_index.unwrap()] = None;
                                    price_level.total_quantity = price_level.total_quantity.checked_sub(old_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                    price_level.order_count = price_level.order_count.checked_sub(1).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                    self.bid.free_list.push(*existing_index.unwrap());
                                    return Ok(());
                                } else {
                                    return Err(OrderBookError::NextNotFound);
                                }                    
                            }
                            else {
                                if let Some(prev_index) = prev{
                                    if let Some(possible_prev_node) = self.bid.order_pool.get_mut(prev_index){
                                        if let Some(prev_node) = possible_prev_node{
                                            prev_node.next = next
                                        }
                                    }
                                }
                                else {
                                    return Err(OrderBookError::PrevNotFound);
                                }
                                if let Some(next_index) = next{
                                    if let Some(possible_next_node) = self.bid.order_pool.get_mut(next_index){
                                        if let Some(next_node) = possible_next_node{
                                            next_node.prev = prev
                                        }
                                    }
                                }
                                else {
                                    return Err(OrderBookError::NextNotFound);
                                }
                                self.bid.order_pool[*existing_index.unwrap()] = None;
                                price_level.total_quantity = price_level.total_quantity.checked_sub(old_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                price_level.order_count = price_level.order_count.checked_sub(1).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                self.bid.free_list.push(*existing_index.unwrap());
                                return Ok(());
                            }
                        } else {
                            self.bid.price_map.remove(&old_price);
                            return Err(OrderBookError::HeadTailCorrupted);
                        }
                    } else {
                        return Err(OrderBookError::NodeNotFound);
                    }
        } else {
            let existing_index = self.ask.order_registry.get(&order_id);
            if existing_index.is_none(){
                return Err(OrderBookError::OrderNotFound);
            }
                    let (prev, next, old_price, old_quantity) = {
                                match self.ask.order_pool[*existing_index.unwrap()].as_ref(){
                                    Some(node) => {
                                        (node.prev, node.next, node.market_limit, node.current_quantity)
                                    }
                                    None => {
                                        return Err(OrderBookError::NodeNotFound);
                                    }
                                }
                            };
                    if let Some(price_level) = self.ask.price_map.get_mut(&old_price){

                        if price_level.head.is_some() && price_level.tail.is_some(){
                            if *existing_index.unwrap() == price_level.head.unwrap() && *existing_index.unwrap() == price_level.tail.unwrap(){
                                self.ask.order_pool[*existing_index.unwrap()] = None;
                                price_level.head = None;
                                price_level.tail = None;
                                price_level.total_quantity = price_level.total_quantity.checked_sub(old_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                price_level.order_count = price_level.order_count.checked_sub(1).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                self.ask.free_list.push(*existing_index.unwrap());
                                return Ok(());
                            }
                            else if *existing_index.unwrap() == price_level.tail.unwrap() {
                               if let Some(prev_index) = prev{
                                    if let Some(possible_prev_node) = self.ask.order_pool.get_mut(prev_index){
                                        if let Some(prev_node) = possible_prev_node{
                                            prev_node.next = None;
                                            price_level.tail = Some(prev_index);
                                        }           
                                    }
                                self.ask.order_pool[*existing_index.unwrap()] = None;
                                price_level.total_quantity = price_level.total_quantity.checked_sub(old_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                price_level.order_count = price_level.order_count.checked_sub(1).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                self.ask.free_list.push(*existing_index.unwrap());
                                return Ok(()); 
                                } else {
                                    return Err(OrderBookError::PrevNotFound);
                                }
                            }
                            else if *existing_index.unwrap() == price_level.head.unwrap() {
                                if let Some(next_index) = next{
                                    if let Some(possible_next_node) = self.ask.order_pool.get_mut(next_index){
                                        if let Some(next_node) = possible_next_node{
                                            next_node.prev = None;
                                            price_level.head = Some(next_index);
                                        }
                                    }
                                    self.ask.order_pool[*existing_index.unwrap()] = None;
                                    price_level.total_quantity = price_level.total_quantity.checked_sub(old_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                    price_level.order_count = price_level.order_count.checked_sub(1).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                    self.ask.free_list.push(*existing_index.unwrap());
                                    return Ok(());
                                } else {
                                    return Err(OrderBookError::NextNotFound);
                                }                    
                            }
                            else {
                                if let Some(prev_index) = prev{
                                    if let Some(possible_prev_node) = self.ask.order_pool.get_mut(prev_index){
                                        if let Some(prev_node) = possible_prev_node{
                                            prev_node.next = next
                                        }
                                    }
                                }
                                else {
                                    return Err(OrderBookError::PrevNotFound);
                                }
                                if let Some(next_index) = next{
                                    if let Some(possible_next_node) = self.ask.order_pool.get_mut(next_index){
                                        if let Some(next_node) = possible_next_node{
                                            next_node.prev = prev
                                        }
                                    }
                                }
                                else {
                                    return Err(OrderBookError::NextNotFound);
                                }
                                self.ask.order_pool[*existing_index.unwrap()] = None;
                                price_level.total_quantity = price_level.total_quantity.checked_sub(old_quantity).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                price_level.order_count = price_level.order_count.checked_sub(1).ok_or_else(|| OrderBookError::QuantityUnderflow)?;
                                self.ask.free_list.push(*existing_index.unwrap());
                                return Ok(());
                            }
                        } else {
                            self.ask.price_map.remove(&old_price);
                            return Err(OrderBookError::HeadTailCorrupted);
                        }
                    } else {
                        return Err(OrderBookError::NodeNotFound);
                    }
        }
    }

    pub fn modify_order(&mut self, order_id : u64, order : EngineModifyOrder) -> Result<Option<ModifyOutcome>, OrderBookError>{
        if order.is_buy_side{
            let existing_index = self.bid.order_registry.get(&order_id);
            if existing_index.is_none(){
                return Err(OrderBookError::OrderNotFound);
            }
                let (old_initial_qty, old_current_qty, old_price) = {
                    match self.bid.order_pool[*existing_index.unwrap()].as_ref(){
                        Some(node) => {
                            (node.initial_quantity, node.current_quantity, node.market_limit)
                        }
                        None => {
                            return Err(OrderBookError::NodeNotFound);
                        }
                    }
                };
                if let Some(new_price) = order.new_price && let Some(new_qty) = order.new_quantity{
                    if new_price != old_price{
                        if let Ok(_) = self.cancel_order(order_id ,EngineCancelOrder { order_id : order.order_id, security_id : order.security_id, is_buy_side: order.is_buy_side,}){
                            return Ok(Some(ModifyOutcome::Both {new_price, new_initial_qty: new_qty, old_current_qty }));
                            }
                        }
                    return Ok(None);
                } else if let Some(new_qty) = order.new_quantity  {
                    if new_qty > old_initial_qty{
                        if let Ok(_) = self.cancel_order(order_id ,EngineCancelOrder { order_id : order.order_id,security_id : order.security_id, is_buy_side: order.is_buy_side,}){
                            return Ok(Some(ModifyOutcome::Requantized {old_price, new_initial_qty: new_qty, old_current_qty }))
                        }
                        return Ok(None);
                    }
                    else {
                        match self.bid.order_pool[*existing_index.unwrap()].as_mut(){
                            Some(order_node) => {
                                order_node.initial_quantity = new_qty;
                                return Ok(Some(ModifyOutcome::Inplace));
                            }
                            None => {
                                return Err(OrderBookError::NodeNotFound);
                            }
                        }
                    }
                } else {
                    if let Ok(_) = self.cancel_order(order_id ,EngineCancelOrder { order_id : order.order_id,security_id : order.security_id, is_buy_side: order.is_buy_side,}){
                        return Ok(Some(ModifyOutcome::Repriced {new_price : order.new_price.unwrap(), old_initial_qty, old_current_qty }));
                    }
                    return Ok(None);
                }
        } else {
            let existing_index = self.ask.order_registry.get(&order_id);
            if existing_index.is_none(){
                return Err(OrderBookError::OrderNotFound);
            }
                let (old_initial_qty, old_current_qty, old_price) = {
                    match self.ask.order_pool[*existing_index.unwrap()].as_ref(){
                        Some(node) => {
                            (node.initial_quantity, node.current_quantity, node.market_limit)
                        }
                        None => {
                            return Err(OrderBookError::NodeNotFound);
                        }
                    }
                };

                if let Some(new_price) = order.new_price && let Some(new_qty) = order.new_quantity{
                    if new_price != old_price{
                        if let Ok(_) = self.cancel_order(order_id ,EngineCancelOrder { order_id : order.order_id,security_id : order.security_id, is_buy_side: order.is_buy_side,}){
                           return Ok(Some(ModifyOutcome::Requantized {old_price, new_initial_qty: new_qty, old_current_qty }))
                        }
                    }
                    return Ok(None);
                } else if let Some(new_qty) = order.new_quantity  {
                    if new_qty > old_initial_qty{
                        if let Ok(_) = self.cancel_order(order_id ,EngineCancelOrder { order_id : order.order_id, security_id : order.security_id, is_buy_side: order.is_buy_side,}){
                            return Ok(Some(ModifyOutcome::Requantized { old_price, new_initial_qty: new_qty, old_current_qty }))
                        }
                        return Ok(None);
                    }
                    else {
                        match self.ask.order_pool[*existing_index.unwrap()].as_mut(){
                            Some(order_node) => {
                                order_node.initial_quantity = new_qty;
                                return Ok(Some(ModifyOutcome::Inplace));
                            }
                            None => {
                                return Err(OrderBookError::NodeNotFound);
                            }
                        }  
                    }
                }else {
                    if let Ok(_) = self.cancel_order(order_id ,EngineCancelOrder { order_id : order.order_id, security_id : order.security_id, is_buy_side: order.is_buy_side,}){
                        return Ok(Some(ModifyOutcome::Repriced { new_price : order.new_price.unwrap(), old_initial_qty, old_current_qty }));
                    }
                    return Ok(None);
                }
        }
    }
    
    pub fn depth(&self, levels_count : Option<u32>) -> Result<BookDepth, anyhow::Error>{
        let timer = Instant::now();

        let ask_iter = self.ask.price_map.iter().rev();
        let bid_iter = self.bid.price_map.iter();

        let ask_depth : Vec<_> = match levels_count {
            Some(n) => ask_iter.take(n as usize)
            .map(|(price, price_level)| PriceLevelDepth {
                price_level : *price,
                quantity : price_level.total_quantity
            })
            .collect(),
            None => ask_iter.map(|(price, price_level)| PriceLevelDepth {
                price_level : *price,
                quantity : price_level.total_quantity
            }).collect()
        };
        let bid_depth = match levels_count {
            Some(n) => bid_iter.take(n as usize)
            .map(|(price, price_level)| PriceLevelDepth {
                price_level : *price,
                quantity : price_level.total_quantity
            })
            .collect(),
            None => bid_iter.map(|(price, price_level)| PriceLevelDepth {
                price_level : *price,
                quantity : price_level.total_quantity
            }).collect()
        };
        let elapsed_time = timer.elapsed().as_micros() as f64;
        Ok(BookDepth { bid_depth, ask_depth, timer : elapsed_time })
    }
}

#[derive(Debug)]
pub struct HalfBook{
    pub price_map : BTreeMap<u32, PriceLevel>,
    pub order_registry : HashMap<u64, usize>,
    pub order_pool : Vec<Option<OrderNode>>,
    pub free_list : Vec<usize>, // we're storing the free indices from the price level to keep the cache lines hot.
}

impl HalfBook {
    pub fn new() -> Self{
        Self { price_map: BTreeMap::new(), order_registry : HashMap::new(), order_pool: Vec::new(), free_list: Vec::new()}
    }
}