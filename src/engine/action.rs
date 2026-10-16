//! Transient presentation state. It is deliberately outside persisted player sessions.
use super::state::GameState;
use crate::world::item::ItemType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ActionKind {
    Swing,
    Mine,
    BowDraw,
    BowRelease,
    Eat,
    Drink,
    Use,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
pub struct ActionVisual {
    pub kind: ActionKind,
    pub item: Option<ItemType>,
    pub progress: f64,
    pub aim: [f64; 2],
    pub ticks: u8,
}
#[derive(Clone, Copy, Debug)]
pub(super) struct ActionRecord {
    visual: ActionVisual,
    age: u64,
}
impl GameState {
    pub(super) fn show_action(
        &mut self,
        kind: ActionKind,
        item: Option<ItemType>,
        x: i32,
        y: i32,
        ticks: u8,
        progress: f64,
    ) {
        let dx = x as f64 + 0.5 - self.player.x;
        let dy = y as f64 + 0.5 - (self.player.y - 1.0);
        let length = dx.hypot(dy).max(0.001);
        if dx.abs() > 0.1 {
            self.player.facing_right = dx > 0.0;
        }
        let visual = ActionVisual {
            kind,
            item,
            progress,
            aim: [dx / length, dy / length],
            ticks,
        };
        self.action_visuals.insert(
            self.selected_peer_id().unwrap_or(0),
            ActionRecord {
                visual,
                age: self.player.age,
            },
        );
    }
    pub fn action_visual(&self, alpha: f64) -> Option<ActionVisual> {
        if self.player.mining_timer > 0.0 && !self.bow_is_drawing() {
            return Some(ActionVisual {
                kind: ActionKind::Mine,
                item: self.inventory.slots[self.hotbar_index as usize]
                    .as_ref()
                    .map(|s| s.item_type),
                progress: ((self.player.age as f64 + alpha) / 8.0).fract(),
                aim: self
                    .action_visuals
                    .get(&self.selected_peer_id().unwrap_or(0))
                    .map_or(
                        [if self.player.facing_right { 1.0 } else { -1.0 }, 0.0],
                        |r| r.visual.aim,
                    ),
                ticks: 8,
            });
        }
        self.peer_action_visual(self.selected_peer_id().unwrap_or(0), self.player.age, alpha)
    }
    pub(crate) fn peer_action_visual(&self, id: u16, age: u64, alpha: f64) -> Option<ActionVisual> {
        let record = self.action_visuals.get(&id)?;
        let elapsed = age.saturating_sub(record.age) as f64 + alpha;
        let mut visual = record.visual;
        if elapsed >= f64::from(visual.ticks) {
            return None;
        }
        if visual.kind != ActionKind::BowDraw {
            visual.progress = (visual.progress + elapsed / f64::from(visual.ticks)).min(1.0);
        }
        Some(visual)
    }
    pub(crate) fn install_action_visual(&mut self, visual: Option<ActionVisual>) {
        let id = self.selected_peer_id().unwrap_or(0);
        if let Some(visual) = visual {
            self.action_visuals.insert(
                id,
                ActionRecord {
                    visual,
                    age: self.player.age,
                },
            );
        } else {
            self.action_visuals.remove(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        engine::command::ClientCommand,
        world::{
            block::BlockType,
            chunk::{CHUNK_HEIGHT, CHUNK_WIDTH},
            item::ItemStack,
        },
    };
    fn room() -> GameState {
        let mut s = GameState::network_view();
        s.dismiss_startup_splash();
        for cx in -2..=2 {
            s.world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in -20..=20 {
            s.world.set_block(x, 40, BlockType::Stone);
        }
        s.player.x = 0.5;
        s.player.y = 40.0;
        s.player.grounded = true;
        s.inventory.slots.fill(None);
        s.hotbar_index = 0;
        s
    }
    fn hold(s: &mut GameState, item: ItemType) {
        s.inventory.slots[0] = Some(ItemStack {
            item_type: item,
            count: 1,
            durability: item.max_durability(),
        });
    }
    #[test]
    fn every_food_fills_the_final_hunger_point_and_stops_at_full() {
        for food in [
            ItemType::Bread,
            ItemType::RawBeef,
            ItemType::RawMutton,
            ItemType::CookedMutton,
            ItemType::CookedBeef,
            ItemType::RawPorkchop,
            ItemType::CookedPorkchop,
            ItemType::RawChicken,
            ItemType::CookedChicken,
            ItemType::RawFish,
            ItemType::CookedFish,
        ] {
            let mut s = room();
            hold(&mut s, food);
            s.inventory.slots[0].as_mut().unwrap().count = 2;
            s.player.hunger = 19.0;
            s.apply_client_command(ClientCommand::UseAt(-20, 0));
            assert_eq!(s.player.hunger, 20.0, "{food:?}");
            assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 1);
            s.apply_client_command(ClientCommand::UseAt(-20, 0));
            assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 1);
            s.update(0, 0);
            s.apply_client_command(ClientCommand::UseAt(-20, 0));
            assert_eq!(
                s.inventory.slots[0].as_ref().unwrap().count,
                1,
                "fractional drain wasted another meal: {food:?}"
            );
        }
    }
    #[test]
    fn a_full_bag_leaves_drops_on_the_ground_then_collects_when_space_is_freed() {
        let mut s = room();
        s.inventory.slots.fill(Some(ItemStack {
            item_type: ItemType::Stone,
            count: 64,
            durability: None,
        }));
        s.item_entities
            .push(crate::entities::item_entity::ItemEntity::new(
                2.5,
                38.0,
                ItemType::Diamond,
            ));
        for _ in 0..80 {
            s.update(0, 0);
        }
        assert_eq!(s.item_entities.len(), 1);
        let drop = &s.item_entities[0];
        assert!(
            (drop.x - 2.5).abs() < 0.01,
            "a full bag must not attract items"
        );
        assert!(drop.grounded && (drop.y - 40.0).abs() < 0.1, "{drop:?}");
        s.inventory.slots[26] = None;
        for _ in 0..40 {
            s.update(0, 0);
        }
        assert!(s.item_entities.is_empty());
        assert!(s.inventory.has_item(ItemType::Diamond, 1));
    }
    #[test]
    fn dropped_items_are_destroyed_by_lava_and_fire_without_being_picked_up() {
        for hazard in [BlockType::Lava(8), BlockType::Fire(8)] {
            let mut s = room();
            s.world.set_block(3, 39, hazard);
            s.item_entities
                .push(crate::entities::item_entity::ItemEntity::new(
                    3.5,
                    39.5,
                    ItemType::Planks,
                ));
            s.update(0, 0);
            assert!(s.item_entities.is_empty(), "{hazard:?}");
            assert!(!s.inventory.has_item(ItemType::Planks, 1));
        }
    }
    #[test]
    fn crafting_is_atomic_with_a_full_bag_and_can_reuse_a_consumed_slot() {
        let mut s = room();
        s.inventory.slots.fill(Some(ItemStack {
            item_type: ItemType::Stone,
            count: 64,
            durability: None,
        }));
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Wood,
            count: 2,
            durability: None,
        });
        let before = s.inventory.slots.clone();
        s.attempt_craft(0);
        assert_eq!(s.inventory.slots, before);
        s.inventory.slots[0].as_mut().unwrap().count = 1;
        s.attempt_craft(0);
        assert_eq!(
            s.inventory.slots[0].as_ref().unwrap().item_type,
            ItemType::Planks
        );
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 4);
        s.inventory_open = true;
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Wood,
            count: 2,
            durability: None,
        });
        s.handle_inventory_click(0);
        s.handle_inventory_right_click(crate::engine::state::CRAFT_GRID_UI_OFFSET);
        s.handle_inventory_right_click(crate::engine::state::CRAFT_GRID_UI_OFFSET);
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Planks,
            count: 64,
            durability: None,
        });
        // A partial result stack still has room, so fill it before testing refusal.
        s.inventory.slots[0].as_mut().unwrap().count = 64;
        s.attempt_craft_from_grid_max();
        s.clear_active_crafting_grid();
        assert_eq!(s.crafting_grid_slot_stack(0).unwrap().count, 2);
        assert!(s.item_entities.is_empty());
        s.inventory.slots[26] = None;
        s.attempt_craft_from_grid_max();
        assert!(s.crafting_grid_slot_stack(0).is_none());
        assert_eq!(s.inventory.slots[26].as_ref().unwrap().count, 8);
    }
    #[test]
    fn clicking_ore_lights_it_only_within_reach_and_line_of_sight() {
        let mut s = room();
        s.world.set_block(3, 39, BlockType::RedstoneOre);
        s.apply_client_command(ClientCommand::UseAt(3, 39));
        assert!(matches!(
            s.world.get_block(3, 39),
            BlockType::LitRedstoneOre(_)
        ));
        s.world.set_block(3, 39, BlockType::RedstoneOre);
        s.world.set_block(1, 39, BlockType::Stone);
        s.apply_client_command(ClientCommand::UseAt(3, 39));
        assert_eq!(s.world.get_block(3, 39), BlockType::RedstoneOre);
        s.world.set_block(12, 39, BlockType::RedstoneOre);
        s.apply_client_command(ClientCommand::UseAt(12, 39));
        assert_eq!(s.world.get_block(12, 39), BlockType::RedstoneOre);
    }
    #[test]
    fn fire_burns_the_player_but_respects_a_fire_resistance_potion() {
        let mut s = room();
        s.world.set_block(0, 39, BlockType::Fire(8));
        s.update(0, 0);
        assert!(s.player.burning_timer > 0);
        let mut protected = room();
        hold(&mut protected, ItemType::PotionFireResistance);
        protected.apply_client_command(ClientCommand::UseAt(-20, 0));
        protected.world.set_block(0, 39, BlockType::Fire(8));
        protected.update(0, 0);
        assert_eq!(protected.player.burning_timer, 0);
        assert_eq!(protected.player.health, 20.0);
    }
    #[test]
    fn ordinary_clicks_merge_stacks_and_keep_the_remainder_selected() {
        let mut s = room();
        s.inventory_open = true;
        for (i, count) in [(0, 20), (1, 60)] {
            s.inventory.slots[i] = Some(ItemStack {
                item_type: ItemType::Cobblestone,
                count,
                durability: None,
            });
        }
        s.handle_inventory_click(0);
        s.handle_inventory_click(1);
        assert_eq!(s.inventory.slots[1].as_ref().unwrap().count, 64);
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 16);
        assert_eq!(s.selected_inventory_slot, Some(0));
        s.handle_inventory_click(2);
        assert!(s.inventory.slots[0].is_none());
        assert_eq!(s.inventory.slots[2].as_ref().unwrap().count, 16);
        assert!(s.selected_inventory_slot.is_none());
    }
    #[test]
    fn missing_swing_is_visible_but_does_not_spend_tool_durability() {
        let mut s = room();
        hold(&mut s, ItemType::IronSword);
        let before = s.inventory.slots[0].clone();
        s.apply_client_command(ClientCommand::SetPrimaryAction(true));
        s.update(3, 38);
        assert_eq!(s.action_visual(0.0).unwrap().kind, ActionKind::Swing);
        assert_eq!(s.inventory.slots[0], before);
        s.apply_client_command(ClientCommand::SetPrimaryAction(false));
        for _ in 0..12 {
            s.update(3, 38);
        }
        assert!(s.action_visual(0.0).is_none());
    }
    #[test]
    fn charged_bow_has_aim_and_release_and_consumes_one_arrow() {
        let mut s = room();
        hold(&mut s, ItemType::Bow);
        s.inventory.add_item(ItemType::Arrow, 3);
        s.apply_client_command(ClientCommand::SetPrimaryAction(true));
        for _ in 0..20 {
            s.update(6, 36);
        }
        let pose = s.action_visual(0.0).unwrap();
        assert_eq!(pose.kind, ActionKind::BowDraw);
        assert!(pose.progress > 0.8);
        assert!(pose.aim[1] < 0.0);
        s.apply_client_command(ClientCommand::SetPrimaryAction(false));
        s.update(6, 36);
        assert_eq!(s.action_visual(0.0).unwrap().kind, ActionKind::BowRelease);
        assert_eq!(s.arrows.len(), 1);
        assert!(s.inventory.has_item(ItemType::Arrow, 2));
        assert!(!s.inventory.has_item(ItemType::Arrow, 3));
    }
    #[test]
    fn last_food_remains_in_eating_pose_and_self_use_ignores_block_reach() {
        let mut s = room();
        hold(&mut s, ItemType::Bread);
        s.player.hunger = 10.0;
        s.interact_block(1000, 0, false);
        assert_eq!(s.player.hunger, 15.0);
        assert!(s.inventory.slots[0].is_none());
        let visual = s.action_visual(0.0).unwrap();
        assert_eq!(visual.kind, ActionKind::Eat);
        assert_eq!(visual.item, Some(ItemType::Bread));
        hold(&mut s, ItemType::Bread);
        s.player.hunger = 20.0;
        s.interact_block(1000, 0, false);
        assert!(s.inventory.slots[0].is_some());
    }
    #[test]
    fn buckets_do_not_fall_back_to_hidden_space_under_clicked_soil() {
        for bucket in [ItemType::WaterBucket, ItemType::LavaBucket] {
            let mut s = room();
            hold(&mut s, bucket);
            s.world.set_block(3, 40, BlockType::Farmland(7));
            s.world.set_block(2, 40, BlockType::WoodDoor(true));
            s.world.set_block(3, 39, BlockType::Stone);
            s.world.set_block(3, 41, BlockType::Air);
            let before = s.inventory.slots[0].clone();
            // An open gate is transparent to aiming, but cannot be replaced by a bucket.
            // The old face fallback selected the hidden space below the soil.
            s.interact_block(3, 40, false);
            assert_eq!(s.world.get_block(3, 41), BlockType::Air);
            assert_eq!(s.inventory.slots[0], before);
            // Dig a visible trench beside the soil, then pour into that exact cell.
            s.world.set_block(2, 40, BlockType::Air);
            s.player.x = 1.5;
            s.interact_block(2, 40, false);
            assert_eq!(
                s.world.get_block(2, 40),
                if bucket == ItemType::WaterBucket {
                    BlockType::Water(8)
                } else {
                    BlockType::Lava(8)
                }
            );
            assert_eq!(
                s.inventory.slots[0].as_ref().unwrap().item_type,
                ItemType::Bucket
            );
        }
    }

    #[test]
    fn planting_clicked_farmland_from_side_and_pouring_water_at_feet_work() {
        let mut s = room();
        s.world.set_block(2, 40, BlockType::Farmland(7));
        hold(&mut s, ItemType::WheatSeeds);
        s.interact_block(2, 40, false);
        assert_eq!(s.world.get_block(2, 39), BlockType::Crops(0));
        assert_eq!(s.action_visual(0.0).unwrap().kind, ActionKind::Use);
        hold(&mut s, ItemType::WaterBucket);
        s.interact_block(0, 39, false);
        assert_eq!(s.world.get_block(0, 39), BlockType::Water(8));
        assert_eq!(
            s.inventory.slots[0].as_ref().unwrap().item_type,
            ItemType::Bucket
        );
        s.interact_block(0, 39, false);
        assert_eq!(s.world.get_block(0, 39), BlockType::Air);
        assert_eq!(
            s.inventory.slots[0].as_ref().unwrap().item_type,
            ItemType::WaterBucket
        );
    }
}
