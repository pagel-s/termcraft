use super::*;
use crate::entities::husbandry::{COURTSHIP_TICKS, Husbandry};

const FARM_ANIMAL_CAP: usize = 128;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum AnimalKind {
    Cow,
    Sheep,
    Pig,
    Chicken,
}
impl AnimalKind {
    fn food(self) -> ItemType {
        match self {
            Self::Chicken => ItemType::WheatSeeds,
            _ => ItemType::Wheat,
        }
    }
    fn size(self) -> (f64, f64) {
        match self {
            Self::Cow => (0.6, 1.375),
            Self::Sheep => (0.55, 1.125),
            Self::Pig => (0.5, 1.0),
            Self::Chicken => (0.3, 1.0),
        }
    }
}
#[derive(Clone, Copy)]
struct Animal {
    kind: AnimalKind,
    index: usize,
    x: f64,
    y: f64,
    healthy: bool,
    hurt: bool,
    state: Husbandry,
}
impl GameState {
    fn farm_animals(&self) -> Vec<Animal> {
        let mut animals = Vec::new();
        macro_rules! collect {
            ($field:ident,$kind:ident) => {
                animals.extend(self.$field.iter().enumerate().map(|(index, m)| Animal {
                    kind: AnimalKind::$kind,
                    index,
                    x: m.x,
                    y: m.y,
                    healthy: m.health > 0.0,
                    hurt: m.hit_timer > 0,
                    state: m.husbandry,
                }));
            };
        }
        collect!(cows, Cow);
        collect!(sheep, Sheep);
        collect!(pigs, Pig);
        collect!(chickens, Chicken);
        animals
    }
    fn husbandry_mut(&mut self, kind: AnimalKind, i: usize) -> &mut Husbandry {
        match kind {
            AnimalKind::Cow => &mut self.cows[i].husbandry,
            AnimalKind::Sheep => &mut self.sheep[i].husbandry,
            AnimalKind::Pig => &mut self.pigs[i].husbandry,
            AnimalKind::Chicken => &mut self.chickens[i].husbandry,
        }
    }
    pub(super) fn feed_farm_animal(&mut self, kind: AnimalKind, i: usize) -> bool {
        let food = kind.food();
        if self.current_hotbar_item_type() != Some(food) {
            return false;
        }
        if self.farm_animals().len() >= FARM_ANIMAL_CAP {
            return true;
        }
        if self.husbandry_mut(kind, i).feed() {
            self.consume_held_item(food);
        }
        // A cooling-down animal is still the interaction target; never plant
        // a seed or place something behind it as a side effect of feeding.
        true
    }
    pub(super) fn tick_animal_husbandry(&mut self) {
        for a in self.farm_animals() {
            if a.healthy && self.body_terrain_loaded(a.x, a.y, 0.0, a.kind.size().0) {
                self.husbandry_mut(a.kind, a.index).tick();
            }
        }
        let animals = self.farm_animals();
        let mut paired = vec![false; animals.len()];
        let mut population = animals.len();
        for (i, a) in animals.iter().enumerate() {
            if paired[i]
                || !a.healthy
                || a.hurt
                || !a.state.ready()
                || population >= FARM_ANIMAL_CAP
            {
                continue;
            }
            let mate = animals.iter().enumerate().skip(i + 1).find(|(j, b)| {
                !paired[*j]
                    && b.kind == a.kind
                    && b.healthy
                    && !b.hurt
                    && b.state.ready()
                    && (a.x - b.x).abs() <= 1.5
                    && (a.y - b.y).abs() <= 0.5
                    && self.has_line_of_sight(a.x, a.y - 0.5, b.x, b.y - 0.5)
            });
            let Some((j, b)) = mate else {
                continue;
            };
            let (half, height) = a.kind.size();
            let site = [((a.x + b.x) * 0.5, a.y), (a.x, a.y), (b.x, b.y)]
                .into_iter()
                .find(|&(x, y)| {
                    self.body_terrain_loaded(x, y, 0.0, half)
                        && !self.entity_collides_with_world(x, y, half * 0.6, height * 0.6)
                        && !self.entity_touches_fluid(x, y, half, height)
                });
            let Some((x, y)) = site else {
                continue;
            };
            paired[i] = true;
            paired[j] = true;
            let ticks = a
                .state
                .courtship_ticks
                .min(b.state.courtship_ticks)
                .saturating_add(1);
            self.husbandry_mut(a.kind, a.index).courtship_ticks = ticks;
            self.husbandry_mut(b.kind, b.index).courtship_ticks = ticks;
            if ticks < COURTSHIP_TICKS {
                continue;
            }
            self.husbandry_mut(a.kind, a.index).after_birth();
            self.husbandry_mut(b.kind, b.index).after_birth();
            macro_rules! birth {
                ($field:ident,$ty:ident) => {{
                    let mut child = $ty::new(x, y);
                    child.husbandry = Husbandry::baby();
                    self.$field.push(child);
                }};
            }
            match a.kind {
                AnimalKind::Cow => birth!(cows, Cow),
                AnimalKind::Sheep => birth!(sheep, Sheep),
                AnimalKind::Pig => birth!(pigs, Pig),
                AnimalKind::Chicken => birth!(chickens, Chicken),
            }
            population += 1;
        }
        for (i, a) in animals.iter().enumerate() {
            if !paired[i] {
                self.husbandry_mut(a.kind, a.index).courtship_ticks = 0;
            }
        }
    }
    pub(super) fn farm_animal_goals(&self) -> HashMap<(AnimalKind, usize), f64> {
        let animals = self.farm_animals();
        let mut players = vec![(
            self.player.x,
            self.player.y,
            self.player.health,
            self.current_hotbar_item_type(),
        )];
        players.extend(self.other_players_here().map(|p| {
            (
                p.player.x,
                p.player.y,
                p.player.health,
                p.inventory.slots[p.hotbar_index as usize]
                    .as_ref()
                    .map(|s| s.item_type),
            )
        }));
        let mut goals = HashMap::new();
        for a in &animals {
            if !a.healthy || a.hurt {
                continue;
            }
            let mate = if a.state.ready() {
                animals
                    .iter()
                    .filter(|b| {
                        b.kind == a.kind && b.index != a.index && b.healthy && b.state.ready()
                    })
                    .filter(|b| {
                        (a.x - b.x).hypot(a.y - b.y) < 8.0
                            && self.has_line_of_sight(a.x, a.y - 0.5, b.x, b.y - 0.5)
                    })
                    .min_by(|b, c| (a.x - b.x).abs().total_cmp(&(a.x - c.x).abs()))
                    .map(|b| (b.x, 0.9))
            } else {
                None
            };
            let food = players
                .iter()
                .filter(|p| {
                    p.2 > 0.0
                        && p.3 == Some(a.kind.food())
                        && (p.0 - a.x).hypot(p.1 - a.y) < 10.0
                        && self.has_line_of_sight(a.x, a.y - 0.5, p.0, p.1 - 0.9)
                })
                .min_by(|p, q| (p.0 - a.x).abs().total_cmp(&(q.0 - a.x).abs()))
                .map(|p| (p.0, 1.8));
            let parent = if a.state.child_ticks > 0 {
                animals
                    .iter()
                    .filter(|b| b.kind == a.kind && b.healthy && b.state.child_ticks == 0)
                    .filter(|b| {
                        (a.x - b.x).hypot(a.y - b.y) < 10.0
                            && self.has_line_of_sight(a.x, a.y - 0.3, b.x, b.y - 0.5)
                    })
                    .min_by(|b, c| (a.x - b.x).abs().total_cmp(&(a.x - c.x).abs()))
                    .map(|b| (b.x, 1.6))
            } else {
                None
            };
            if let Some((target, gap)) = mate.or(food).or(parent) {
                let dx = target - a.x;
                goals.insert(
                    (a.kind, a.index),
                    if dx.abs() > gap {
                        dx.signum() * 0.1
                    } else {
                        0.0
                    },
                );
            }
        }
        goals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn farm(kind: AnimalKind) -> GameState {
        let mut s = GameState::network_view();
        s.dismiss_startup_splash();
        s.game_rules.do_mob_spawning = false;
        s.game_rules.do_daylight_cycle = false;
        s.game_rules.do_weather_cycle = false;
        for cx in -2..=2 {
            s.world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in -64..96 {
            s.world.set_block(x, 20, BlockType::Grass);
        }
        s.player = Player::new(0.5, 20.0);
        s.time_of_day = 8000.0;
        for x in [1.5, 2.5] {
            match kind {
                AnimalKind::Cow => s.cows.push(Cow::new(x, 20.0)),
                AnimalKind::Sheep => s.sheep.push(Sheep::new(x, 20.0)),
                AnimalKind::Pig => s.pigs.push(Pig::new(x, 20.0)),
                AnimalKind::Chicken => s.chickens.push(Chicken::new(x, 20.0)),
            }
        }
        s.inventory.slots[0] = Some(ItemStack {
            item_type: kind.food(),
            count: 8,
            durability: None,
        });
        s.hotbar_index = 0;
        s
    }
    #[test]
    fn feeding_each_species_produces_one_child_and_obeys_cooldowns() {
        for kind in [
            AnimalKind::Cow,
            AnimalKind::Sheep,
            AnimalKind::Pig,
            AnimalKind::Chicken,
        ] {
            let mut s = farm(kind);
            s.interact_block(1, 19, false);
            s.interact_block(2, 19, false);
            assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 6, "{kind:?}");
            for _ in 0..100 {
                s.update(0, 0);
            }
            let animals = s.farm_animals();
            assert_eq!(animals.len(), 3, "{kind:?}");
            assert_eq!(
                animals.iter().filter(|a| a.state.child_ticks > 0).count(),
                1
            );
            assert_eq!(
                animals
                    .iter()
                    .filter(|a| a.state.cooldown_ticks > 0)
                    .count(),
                2
            );
            for i in 0..3 {
                assert!(s.feed_farm_animal(kind, i));
            }
            assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 6);
            for _ in 0..100 {
                s.update(0, 0);
            }
            assert_eq!(s.farm_animals().len(), 3);
        }
    }
    #[test]
    fn food_attraction_uses_the_nearby_players_held_food_and_visibility() {
        let mut s = farm(AnimalKind::Cow);
        s.cows[0].x = 6.5;
        assert!(s.farm_animal_goals()[&(AnimalKind::Cow, 0)] < 0.0);
        s.inventory.slots[0] = None;
        assert!(!s.farm_animal_goals().contains_key(&(AnimalKind::Cow, 0)));
        s.join_peer(1);
        s.join_peer(2);
        s.activate_peer(2);
        s.player.x = 14.5;
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Wheat,
            count: 8,
            durability: None,
        });
        s.activate_peer(1);
        assert!(s.farm_animal_goals()[&(AnimalKind::Cow, 0)] > 0.0);
        for y in 14..20 {
            s.world.set_block(10, y, BlockType::Stone);
        }
        assert!(!s.farm_animal_goals().contains_key(&(AnimalKind::Cow, 0)));
    }
    #[test]
    fn feeding_and_breeding_do_not_pass_through_a_wall() {
        let mut s = farm(AnimalKind::Cow);
        s.cows[0].x = 1.8;
        s.cows[1].x = 3.2;
        for y in 14..20 {
            s.world.set_block(2, y, BlockType::Stone);
        }
        s.interact_block(3, 19, false);
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 8);
        for m in &mut s.cows {
            m.husbandry.feed();
        }
        for _ in 0..120 {
            s.tick_animal_husbandry();
        }
        assert_eq!(s.cows.len(), 2);
        assert!(s.cows.iter().all(|m| m.husbandry.courtship_ticks == 0));
    }
    #[test]
    fn a_farm_persists_away_from_players_and_young_animals_mature() {
        let mut s = farm(AnimalKind::Cow);
        s.cows[0].husbandry.feed();
        s.cows[0].x = 500.0;
        s.cows[1].x = 510.0;
        s.trim_far_overworld_mobs();
        assert_eq!(s.cows.len(), 1);
        let mut h = Husbandry::baby();
        for _ in 0..crate::entities::husbandry::CHILD_TICKS {
            assert!(!h.feed());
            h.tick();
        }
        assert_eq!(h.scale(), 1.0);
        assert!(h.feed());
        assert!(h.ready());
    }
}
