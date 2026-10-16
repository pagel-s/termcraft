//! Inventory geometry is shared by painting and picking, including the recipe book.
use super::{ARMOR_UI_OFFSET, CRAFT_GRID_UI_OFFSET, CRAFT_OUTPUT_UI_SLOT};
use super::{Color, GameState, Renderer};
use crate::world::item::Recipe;

impl Renderer {
    fn has_recipe_book(state: &GameState) -> bool {
        !state.at_chest && !state.at_enchanting_table && !state.at_anvil && !state.at_brewing_stand
    }
    pub fn toggle_recipe_book(&mut self) {
        self.recipe_book_open = !self.recipe_book_open;
    }
    pub fn scroll_recipe_book(&mut self, delta: i32) {
        self.recipe_scroll = self.recipe_scroll.saturating_add_signed(delta as isize);
    }
    pub(super) fn inventory_origin(&self, state: &GameState) -> (i32, i32) {
        let side = self.recipe_book_open && Self::has_recipe_book(state) && self.width >= 100;
        (
            (self.width as i32 - if side { 96 } else { 60 }) / 2 + if side { 36 } else { 0 },
            (self.height as i32 - 24) / 2,
        )
    }
    fn recipe_origin(&self, state: &GameState) -> (i32, i32, i32) {
        let (x, y) = self.inventory_origin(state);
        if self.width >= 100 {
            (x - 36, y, 34)
        } else {
            (x, y, 60)
        }
    }
    fn recipe_ready(state: &GameState, index: usize, recipe: &Recipe) -> bool {
        if state.at_furnace {
            state.can_start_furnace_recipe(index)
        } else {
            recipe.can_craft(&state.inventory, state.at_crafting_table, false)
        }
    }
    fn recipe_ids(state: &GameState) -> Vec<usize> {
        Recipe::all()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.needs_furnace == state.at_furnace)
            .map(|(i, _)| i)
            .collect()
    }
    pub(super) fn inventory_slot_position(
        &self,
        state: &GameState,
        slot: usize,
    ) -> Option<(i32, i32)> {
        let (x, y) = self.inventory_origin(state);
        if state.at_chest {
            if slot < 27 {
                return Some((x + 2 + (slot % 9) as i32 * 6, y + 4 + (slot / 9) as i32 * 2));
            }
            if slot < 54 {
                let i = slot - 27;
                return Some((
                    x + 2 + (i % 9) as i32 * 6,
                    y + if i < 9 {
                        17
                    } else {
                        12 + ((i - 9) / 9) as i32 * 2
                    },
                ));
            }
            return None;
        }
        if slot < 27 {
            return Some((
                x + 2 + (slot % 9) as i32 * 6,
                y + if slot < 9 {
                    8
                } else {
                    3 + ((slot - 9) / 9) as i32 * 2
                },
            ));
        }
        if state.has_personal_armor_ui() && (ARMOR_UI_OFFSET..ARMOR_UI_OFFSET + 4).contains(&slot) {
            return Some((x + 4, y + 11 + (slot - ARMOR_UI_OFFSET) as i32 * 2));
        }
        if !Self::has_recipe_book(state) || state.at_furnace {
            return None;
        }
        let size = if state.at_crafting_table { 3 } else { 2 };
        if (CRAFT_GRID_UI_OFFSET..CRAFT_GRID_UI_OFFSET + 9).contains(&slot) {
            let i = slot - CRAFT_GRID_UI_OFFSET;
            if i % 3 < size && i / 3 < size {
                return Some((x + 30 + (i % 3) as i32 * 6, y + 11 + (i / 3) as i32 * 2));
            }
        }
        (slot == CRAFT_OUTPUT_UI_SLOT).then_some((x + 30 + size as i32 * 6 + 6, y + 13))
    }
    pub(super) fn recipe_book_click(&self, state: &GameState, sx: u16, sy: u16) -> Option<usize> {
        if !self.recipe_book_open || !Self::has_recipe_book(state) {
            return None;
        }
        let (x, y, w) = self.recipe_origin(state);
        let (dx, dy) = (sx as i32 - x, sy as i32 - y);
        if !(2..w - 2).contains(&dx) || !(4..20).contains(&dy) {
            return None;
        }
        let ids = Self::recipe_ids(state);
        ids.get(self.recipe_scroll.min(ids.len().saturating_sub(8)) + ((dy - 4) / 2) as usize)
            .copied()
    }
    pub(super) fn compact_recipe_book(&self, state: &GameState) -> bool {
        self.recipe_book_open && Self::has_recipe_book(state) && self.width < 100
    }
    pub(super) fn draw_recipe_book(&mut self, state: &GameState) {
        if !self.recipe_book_open || !Self::has_recipe_book(state) {
            return;
        }
        let (x, y, w) = self.recipe_origin(state);
        let bg = Self::rgb((24, 31, 39));
        for row in 0..24 {
            self.put_str(x, y + row, &" ".repeat(w as usize), Color::Reset, bg);
        }
        let recipes = Recipe::all();
        let ids = Self::recipe_ids(state);
        self.recipe_scroll = self.recipe_scroll.min(ids.len().saturating_sub(8));
        self.put_str(x + 2, y + 1, "RECIPE BOOK", Self::rgb((237, 201, 121)), bg);
        self.put_str(
            x + 2,
            y + 2,
            &format!(
                "{}-{} / {}",
                self.recipe_scroll + 1,
                (self.recipe_scroll + 8).min(ids.len()),
                ids.len()
            ),
            Self::rgb((150, 166, 178)),
            bg,
        );
        for (row, &index) in ids.iter().skip(self.recipe_scroll).take(8).enumerate() {
            let r = &recipes[index];
            let ready = Self::recipe_ready(state, index, r);
            let ry = y + 4 + row as i32 * 2;
            let title = format!(
                "{} {} x{}",
                if ready { ">" } else { " " },
                r.result.name(),
                r.result_count
            );
            self.put_str(
                x + 2,
                ry,
                &title.chars().take((w - 4) as usize).collect::<String>(),
                if ready {
                    Self::rgb((157, 218, 145))
                } else {
                    Self::rgb((219, 230, 236))
                },
                bg,
            );
            let ingredients = r
                .ingredient_requirements()
                .iter()
                .map(|(item, n)| {
                    format!(
                        "{} {}/{}",
                        item.name(),
                        state
                            .inventory
                            .slots
                            .iter()
                            .flatten()
                            .filter(|s| s.item_type == *item)
                            .map(|s| s.count)
                            .sum::<u32>(),
                        n
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let text = if r.needs_crafting_table && !state.at_crafting_table {
                "Needs crafting table".to_owned()
            } else {
                ingredients
            };
            self.put_str(
                x + 2,
                ry + 1,
                &text.chars().take((w - 4) as usize).collect::<String>(),
                Self::rgb((150, 166, 178)),
                bg,
            );
        }
        self.put_str(
            x + 2,
            y + 21,
            if state.at_furnace {
                "Click: start smelting"
            } else {
                "Click: craft · Shift: max"
            },
            Self::rgb((139, 211, 221)),
            bg,
        );
        self.put_str(
            x + 2,
            y + 22,
            "Wheel/PgUp/PgDn · Tab: bag",
            Self::rgb((150, 166, 178)),
            bg,
        );
        if let Some(index) = self.recipe_book_click(state, state.mouse_x, state.mouse_y) {
            let r = &recipes[index];
            let text = if Self::recipe_ready(state, index, r) {
                format!("Craft {} x{}", r.result.name(), r.result_count)
            } else if state.at_furnace {
                "Check furnace input/fuel/job".into()
            } else if r.needs_crafting_table && !state.at_crafting_table {
                "Use a crafting table".into()
            } else if r
                .ingredient_requirements()
                .iter()
                .any(|(item, n)| !state.inventory.has_item(*item, *n))
            {
                "Missing ingredients".into()
            } else {
                "No room for the full output".into()
            };
            self.put_str(
                x + 2,
                y + 20,
                &text.chars().take((w - 4) as usize).collect::<String>(),
                Self::rgb((237, 201, 121)),
                bg,
            );
        }
    }
}
