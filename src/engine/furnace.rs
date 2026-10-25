//! Explicit input/fuel/output storage shared by everyone using the same block.
use super::*;
pub const FURNACE_INPUT_UI_SLOT: usize = 60;
pub const FURNACE_FUEL_UI_SLOT: usize = 61;
pub const FURNACE_OUTPUT_UI_SLOT: usize = 62;

impl GameState {
    pub(super) fn furnace_storage(&self) -> FurnaceStorage {
        FurnaceStorage {
            slots: self.furnace_slots.clone(),
            job: self.furnace_job,
            progress: self.furnace_progress_ticks,
            burn: self.furnace_burn_ticks,
        }
    }
    pub(super) fn install_furnace_storage(&mut self, f: FurnaceStorage) {
        self.furnace_slots = f.slots;
        self.furnace_job = f.job;
        self.furnace_progress_ticks = f.progress;
        self.furnace_burn_ticks = f.burn;
    }
    pub(super) fn bound_furnace(&self) -> Option<&FurnaceStorage> {
        if !self.at_furnace {
            return None;
        }
        let (x, y) = self.open_chest_pos?;
        self.world.furnace(x, y)
    }
    pub(crate) fn refresh_furnace_view(&mut self) {
        if let Some(f) = self.bound_furnace().cloned() {
            self.install_furnace_storage(f);
        }
    }
    pub(super) fn commit_furnace_view(&mut self) {
        if self.at_furnace
            && let Some((x, y)) = self.open_chest_pos
        {
            self.world.set_furnace(x, y, self.furnace_storage());
        }
    }
    pub(super) fn open_placed_furnace(&mut self, x: i32, y: i32) {
        // Old saves kept a personal queue. Transfer it once to this furnace;
        // if occupied, preserve the old items in the bag or as ground drops.
        let legacy = self.furnace_storage();
        let prior_bound = self.at_furnace && self.open_chest_pos.is_some();
        self.open_furnace_inventory_view();
        let mut shared = self.world.furnace(x, y).cloned().unwrap_or_default();
        if !prior_bound && legacy != FurnaceStorage::default() {
            if shared == FurnaceStorage::default() {
                shared = legacy;
            } else {
                for stack in legacy.slots.into_iter().flatten() {
                    self.add_item_to_inventory_or_drop(stack.item_type, stack.count);
                }
                if let Some(job) = legacy.job {
                    self.add_item_to_inventory_or_drop(job.input, job.input_count);
                }
            }
        }
        self.world.set_furnace(x, y, shared.clone());
        self.open_chest_pos = Some((x, y));
        self.install_furnace_storage(shared);
    }
    pub(super) fn clear_bound_furnace_view(&mut self) {
        if self.at_furnace && self.open_chest_pos.is_some() {
            self.install_furnace_storage(FurnaceStorage::default());
        }
    }
    pub(super) fn tick_placed_furnaces(&mut self) {
        let personal = self.furnace_storage();
        for (x, y) in self.world.furnace_positions() {
            let Some(f) = self.world.furnace(x, y).cloned() else {
                continue;
            };
            self.install_furnace_storage(f);
            self.update_explicit_furnace();
            self.world.set_furnace(x, y, self.furnace_storage());
        }
        self.install_furnace_storage(personal);
        self.refresh_furnace_view();
    }
    pub fn furnace_cook_fraction(&self) -> f32 {
        self.bound_furnace()
            .map_or(self.furnace_progress_ticks, |f| f.progress) as f32
            / FURNACE_COOK_TICKS as f32
    }
    pub fn furnace_has_heat(&self) -> bool {
        self.bound_furnace()
            .map_or(self.furnace_burn_ticks, |f| f.burn)
            > 0
    }
    pub fn furnace_slot_stack(&self, index: usize) -> Option<&ItemStack> {
        self.bound_furnace()
            .map_or(&self.furnace_slots, |f| &f.slots)
            .get(index)?
            .as_ref()
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
            && self.furnace_slot_stack(0).is_none_or(|s| {
                s.item_type == job.input
                    && s.count + job.input_count <= s.item_type.max_stack_size()
            })
    }
    pub(super) fn load_furnace_recipe(&mut self, job: FurnaceJob) -> bool {
        self.refresh_furnace_view();
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
        self.commit_furnace_view();
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
