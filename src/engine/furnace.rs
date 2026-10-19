//! Explicit input/fuel/output storage for the player's existing furnace session.
use super::*;
pub const FURNACE_INPUT_UI_SLOT: usize = 60;
pub const FURNACE_FUEL_UI_SLOT: usize = 61;
pub const FURNACE_OUTPUT_UI_SLOT: usize = 62;

impl GameState {
    pub fn furnace_cook_fraction(&self) -> f32 {
        self.furnace_progress_ticks as f32 / FURNACE_COOK_TICKS as f32
    }
    pub fn furnace_has_heat(&self) -> bool {
        self.furnace_burn_ticks > 0
    }
    pub fn furnace_slot_stack(&self, index: usize) -> Option<&ItemStack> {
        self.furnace_slots.get(index)?.as_ref()
    }
    pub(super) fn furnace_ui_index(&self, slot: usize) -> Option<usize> {
        (self.at_furnace && self.furnace_explicit && (60..63).contains(&slot)).then(|| slot - 60)
    }
    pub(super) fn furnace_fuel_ticks(item: ItemType) -> u16 {
        match item {
            ItemType::Coal => FURNACE_COAL_BURN_TICKS,
            ItemType::Wood | ItemType::BirchWood | ItemType::Planks => 300,
            ItemType::Stick => 100,
            _ => 0,
        }
    }
    pub(super) fn explicit_furnace_can_load(&self, job: FurnaceJob) -> bool {
        self.inventory.has_item(job.input, job.input_count)
            && self.furnace_slots[0].as_ref().is_none_or(|s| {
                s.item_type == job.input
                    && s.count + job.input_count <= s.item_type.max_stack_size()
            })
    }
    pub(super) fn load_furnace_recipe(&mut self, job: FurnaceJob) -> bool {
        if !self.explicit_furnace_can_load(job) {
            return false;
        }
        self.inventory.remove_item(job.input, job.input_count);
        if let Some(stack) = &mut self.furnace_slots[0] {
            stack.count += job.input_count;
        } else {
            self.furnace_slots[0] = Some(ItemStack {
                item_type: job.input,
                count: job.input_count,
                durability: None,
            });
        }
        if self.furnace_slots[1].is_none()
            && self.furnace_burn_ticks == 0
            && self.inventory.remove_item(ItemType::Coal, 1)
        {
            self.furnace_slots[1] = Some(ItemStack {
                item_type: ItemType::Coal,
                count: 1,
                durability: None,
            });
        }
        true
    }
    fn explicit_furnace_has_room(&self, job: FurnaceJob) -> bool {
        self.furnace_slots[2].as_ref().is_none_or(|s| {
            s.item_type == job.output && s.count + job.output_count <= s.item_type.max_stack_size()
        })
    }
    fn light_explicit_furnace(&mut self) -> bool {
        if self.furnace_burn_ticks > 0 {
            return true;
        }
        let Some(stack) = &mut self.furnace_slots[1] else {
            return false;
        };
        let ticks = Self::furnace_fuel_ticks(stack.item_type);
        if ticks == 0 {
            return false;
        }
        self.furnace_burn_ticks = ticks;
        stack.count -= 1;
        if stack.count == 0 {
            self.furnace_slots[1] = None;
        }
        true
    }
    pub(super) fn update_explicit_furnace(&mut self) {
        if self.furnace_job.is_none() {
            let Some(input) = self.furnace_slots[0].as_ref() else {
                return;
            };
            let Some(job) = Self::furnace_job_for_input_item(input.item_type) else {
                return;
            };
            if input.count < job.input_count
                || !self.explicit_furnace_has_room(job)
                || !self.light_explicit_furnace()
            {
                return;
            }
            let input = self.furnace_slots[0].as_mut().unwrap();
            input.count -= job.input_count;
            if input.count == 0 {
                self.furnace_slots[0] = None;
            }
            self.furnace_job = Some(job);
            self.furnace_progress_ticks = 0;
        }
        let job = self.furnace_job.unwrap();
        if !self.explicit_furnace_has_room(job) {
            return;
        }
        if self.furnace_progress_ticks < FURNACE_COOK_TICKS {
            if !self.light_explicit_furnace() {
                return;
            }
            self.furnace_burn_ticks -= 1;
            self.furnace_progress_ticks += 1;
        }
        if self.furnace_progress_ticks >= FURNACE_COOK_TICKS {
            if let Some(out) = &mut self.furnace_slots[2] {
                out.count += job.output_count;
            } else {
                self.furnace_slots[2] = Some(ItemStack {
                    item_type: job.output,
                    count: job.output_count,
                    durability: None,
                });
            }
            self.furnace_job = None;
            self.furnace_progress_ticks = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stack(item_type: ItemType, count: u32) -> ItemStack {
        ItemStack {
            item_type,
            count,
            durability: None,
        }
    }
    #[test]
    fn explicit_slots_reject_wrong_items_pause_and_collect_without_bag_consumption() {
        let mut s = GameState::network_view();
        s.inventory.slots.fill(None);
        s.inventory.slots[0] = Some(stack(ItemType::RawIron, 3));
        s.inventory.slots[1] = Some(stack(ItemType::Coal, 2));
        s.inventory.slots[2] = Some(stack(ItemType::Diamond, 5));
        s.open_furnace_inventory_view();
        s.handle_inventory_click(2);
        s.handle_inventory_click(60);
        assert_eq!(s.inventory.slots[2].as_ref().unwrap().count, 5);
        assert!(s.furnace_slots[0].is_none());
        s.selected_inventory_slot = None;
        s.handle_inventory_click(0);
        s.handle_inventory_click(60);
        for _ in 0..250 {
            s.update_furnace();
        }
        assert!(s.furnace_job.is_none());
        assert_eq!(s.furnace_slots[0].as_ref().unwrap().count, 3);
        assert_eq!(s.inventory.slots[1].as_ref().unwrap().count, 2);
        assert!(s.quick_start_furnace_from_shift_click(1));
        s.clear_container_interaction_state();
        for _ in 0..FURNACE_COOK_TICKS {
            s.update_furnace();
        }
        assert_eq!(
            s.furnace_slots[2].as_ref().unwrap().item_type,
            ItemType::IronIngot
        );
        assert_eq!(s.furnace_slots[2].as_ref().unwrap().count, 1);
        assert_eq!(s.furnace_slots[0].as_ref().unwrap().count, 2);
        assert_eq!(s.furnace_slots[1].as_ref().unwrap().count, 1);
        s.open_furnace_inventory_view();
        assert!(s.quick_start_furnace_from_shift_click(62));
        assert_eq!(
            s.inventory.slots[0].as_ref().unwrap().item_type,
            ItemType::IronIngot
        );
        assert!(s.furnace_slots[2].is_none());
        s.selected_inventory_slot = None;
        s.handle_inventory_click(0);
        s.handle_inventory_click(62);
        assert!(
            s.furnace_slots[2].is_none(),
            "cannot insert items into output"
        );
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 1);
    }
    #[test]
    fn full_output_never_spends_input_or_fuel() {
        let mut s = GameState::network_view();
        s.open_furnace_inventory_view();
        s.furnace_slots = [
            Some(stack(ItemType::RawIron, 2)),
            Some(stack(ItemType::Coal, 3)),
            Some(stack(ItemType::IronIngot, 64)),
        ];
        let before = s.furnace_slots.clone();
        for _ in 0..300 {
            s.update_furnace();
        }
        assert_eq!(s.furnace_slots, before);
        assert_eq!(s.furnace_burn_ticks, 0);
        assert!(s.furnace_job.is_none());
    }
}
