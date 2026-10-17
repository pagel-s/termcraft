//! Explicit throwing preserves stacks, tool wear and enchantments.
use super::*;
use crate::entities::item_entity::DroppedStack;

impl GameState {
    pub fn drop_selected_item(&mut self, whole_stack: bool) -> bool {
        if self.death_screen_active
            || self.credits_active
            || self.settings_menu_open
            || self.player.health <= 0.0
            || self.item_entities.len() >= ITEM_ENTITY_HARD_CAP
        {
            return false;
        }
        let slot = if self.inventory_open {
            let Some(slot) = self.selected_inventory_slot else {
                return false;
            };
            slot
        } else {
            self.hotbar_index as usize
        };
        let Some(Some(mut stack)) = self.ui_slot_item_clone(slot) else {
            return false;
        };
        let enchant_level = self.ui_slot_enchant_level(slot).unwrap_or(0);
        let count = if whole_stack { stack.count } else { 1 };
        let mut thrown = stack.clone();
        thrown.count = count;
        stack.count -= count;
        if !self.set_ui_slot_item(slot, (stack.count > 0).then_some(stack)) {
            return false;
        }
        if self.ui_slot_item_clone(slot) == Some(None) {
            self.selected_inventory_slot = None;
        }
        let direction = if self.player.facing_right { 1.0 } else { -1.0 };
        let mut item = ItemEntity::new(self.player.x, self.player.y - 1.1, thrown.item_type);
        item.vx = direction * 0.22;
        item.vy = -0.12;
        item.pickup_delay = 40;
        item.payload = Some(DroppedStack {
            stack: thrown,
            enchant_level,
        });
        self.item_entities.push(item);
        true
    }

    pub(super) fn collect_loose_item(&mut self, item: &mut ItemEntity) -> bool {
        let Some(payload) = item.payload.as_mut() else {
            return self.inventory.add_item(item.item_type, 1) == 0;
        };
        let limit = if payload.stack.item_type.max_durability().is_some() {
            1
        } else {
            payload.stack.item_type.max_stack_size()
        };
        for (index, slot) in self.inventory.slots.iter_mut().enumerate() {
            if let Some(stack) = slot
                && stack.item_type == payload.stack.item_type
                && stack.durability == payload.stack.durability
                && self.inventory_enchant_levels[index] == payload.enchant_level
            {
                let count = payload.stack.count.min(limit.saturating_sub(stack.count));
                stack.count += count;
                payload.stack.count -= count;
            }
            if payload.stack.count == 0 {
                return true;
            }
        }
        for (index, slot) in self.inventory.slots.iter_mut().enumerate() {
            if slot.is_none() {
                let count = payload.stack.count.min(limit);
                let mut stack = payload.stack.clone();
                stack.count = count;
                *slot = Some(stack);
                self.inventory_enchant_levels[index] = payload.enchant_level;
                payload.stack.count -= count;
                if payload.stack.count == 0 {
                    return true;
                }
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn throwing_and_recollecting_preserves_counts_wear_and_enchantment() {
        let mut s = GameState::new();
        s.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        s.inventory.add_item(ItemType::DiamondSword, 1);
        s.inventory.slots[0].as_mut().unwrap().durability = Some(37);
        s.inventory_enchant_levels[0] = 3;
        s.hotbar_index = 0;
        assert!(s.drop_selected_item(false));
        assert!(s.inventory.slots[0].is_none());
        let mut item = s.item_entities.pop().unwrap();
        assert_eq!(item.pickup_delay, 40);
        assert!(s.collect_loose_item(&mut item));
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().durability, Some(37));
        assert_eq!(s.inventory_enchant_levels[0], 3);
        s.inventory = Inventory::new(PLAYER_INVENTORY_CAPACITY);
        s.inventory_enchant_levels.fill(0);
        s.inventory.add_item(ItemType::Stone, 64);
        assert!(s.drop_selected_item(false));
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 63);
        assert!(s.drop_selected_item(true));
        assert!(s.inventory.slots[0].is_none());
        let mut item = s.item_entities.pop().unwrap();
        assert_eq!(item.payload.as_ref().unwrap().stack.count, 63);
        s.inventory
            .add_item(ItemType::Dirt, PLAYER_INVENTORY_CAPACITY as u32 * 64);
        assert!(!s.collect_loose_item(&mut item));
        assert_eq!(item.payload.as_ref().unwrap().stack.count, 63);
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Stone,
            count: 63,
            durability: None,
        });
        s.inventory_enchant_levels[0] = 0;
        assert!(!s.collect_loose_item(&mut item));
        assert_eq!(item.payload.as_ref().unwrap().stack.count, 62);
    }
}
