//! Block-owned furnace storage, persisted with its terrain chunk.
use super::item::{ItemStack, ItemType, Recipe};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FurnaceJob {
    pub input: ItemType,
    pub input_count: u32,
    pub output: ItemType,
    pub output_count: u32,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct FurnaceStorage {
    pub slots: [Option<ItemStack>; 3],
    pub job: Option<FurnaceJob>,
    pub progress: u16,
    pub burn: u16,
}

impl FurnaceStorage {
    pub fn valid(&self) -> bool {
        let recipes = Recipe::all();
        let input_valid = |item| {
            recipes.iter().any(|r| {
                r.needs_furnace
                    && r.ingredients
                        .iter()
                        .any(|&(i, _)| i == item && i != ItemType::Coal)
            })
        };
        self.slots.iter().flatten().all(|s| {
            s.count > 0 && s.count <= s.item_type.max_stack_size() && s.durability.is_none()
        }) && self.slots[0]
            .as_ref()
            .is_none_or(|s| input_valid(s.item_type))
            && self.slots[1].as_ref().is_none_or(|s| {
                matches!(
                    s.item_type,
                    ItemType::Coal
                        | ItemType::Wood
                        | ItemType::BirchWood
                        | ItemType::Planks
                        | ItemType::Stick
                )
            })
            && self.slots[2].as_ref().is_none_or(|s| {
                recipes
                    .iter()
                    .any(|r| r.needs_furnace && r.result == s.item_type)
            })
            && self.progress <= 80
            && self.burn <= 640
            && self.job.is_none_or(|j| {
                recipes.iter().any(|r| {
                    r.needs_furnace
                        && r.result == j.output
                        && r.result_count == j.output_count
                        && r.ingredients
                            .iter()
                            .any(|&(i, n)| i == j.input && n == j.input_count)
                })
            })
    }
}
