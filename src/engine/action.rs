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
