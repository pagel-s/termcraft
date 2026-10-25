//! Local survival steering for land animals. No saved AI state or chunk loads.
use super::*;

impl GameState {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn calculate_animal_movement(
        &self,
        x: f64,
        y: f64,
        mut vx: f64,
        mut vy: f64,
        grounded: bool,
        half: f64,
        height: f64,
        recovering: bool,
    ) -> (f64, f64, f64, f64, bool, bool) {
        let (water, _) = self.entity_fluid_submersion(x, y, half, height);
        let swimming = water >= 0.2;
        let above_water =
            !grounded && matches!(self.world.fluid_at_point(x, y + 0.8), BlockType::Water(_));
        // Keep real knockback and collision response. Swimming resumes when
        // the brief hurt reaction ends, rather than making animals invulnerable.
        if !recovering {
            if swimming || above_water {
                if water > 0.45 {
                    vy = (vy - 0.05).clamp(-0.16, -0.045);
                }
                if let Some((bank_x, bank_y)) =
                    self.animal_water_bank(x, y, half, height, vx >= 0.0)
                {
                    vx = (bank_x - x).signum() * 0.11;
                    // A bank one block above the surface needs a short exit
                    // stroke, not endless paddling into its side. The ordinary
                    // swept collision/ledge checks still govern the ascent.
                    if (0.2..0.85).contains(&water)
                        && (bank_x - x).abs() < half + 1.1
                        && bank_y < y - 0.9
                    {
                        vy = vy.min(-0.36);
                    }
                } else {
                    vx = if vx < 0.0 { -0.075 } else { 0.075 };
                }
            } else if (grounded || self.has_stable_ground_support(x, y, half, vx))
                && vx.abs() > 0.01
            {
                // Look beyond the leading foot before walking or hopping into
                // a pond, waterfall or lava. Falling/knocked animals still fall.
                let probe_x = x + vx.signum() * (half + 0.45);
                let mut danger = false;
                for dy in 0..=2 {
                    let block = self
                        .world
                        .get_block(probe_x.floor() as i32, (y - 0.05).floor() as i32 + dy);
                    if block.is_solid() {
                        break;
                    }
                    if block.is_fluid() {
                        danger = true;
                        break;
                    }
                }
                if danger {
                    vx = -vx.signum() * 0.07;
                    if vy < 0.0 {
                        vy = 0.0;
                    }
                }
            }
        }
        self.calculate_movement_with_jump_held(
            x,
            y,
            vx,
            vy,
            grounded,
            half,
            height,
            // Airborne step assist can add another block to a normal animal
            // jump and let it climb out over a two-block wall or closed door.
            grounded || water > 0.0,
            swimming && !recovering,
        )
    }

    fn animal_water_bank(
        &self,
        x: f64,
        y: f64,
        half: f64,
        height: f64,
        right: bool,
    ) -> Option<(f64, f64)> {
        let column = x.floor() as i32;
        let mut surface = (y - height * 0.5).floor() as i32;
        // Find the open surface of this water column. A roof is not a bank.
        for _ in 0..16 {
            match self.world.get_block(column, surface) {
                BlockType::Water(_) => surface -= 1,
                BlockType::Air => break,
                _ => return None,
            }
        }
        if self.world.get_block(column, surface) != BlockType::Air {
            return None;
        }
        let surface = surface + 1;
        let preferred = if right { 1 } else { -1 };
        for distance in 1..=12 {
            for direction in [preferred, -preferred] {
                let bx = column + direction * distance;
                if !self.world.is_chunk_loaded_at(bx) {
                    continue;
                }
                for floor in surface - 1..=surface + 1 {
                    let bank_x = bx as f64 + 0.5;
                    if self.world.get_block(bx, floor).is_solid()
                        && !self.entity_collides_with_world(bank_x, floor as f64, half, height)
                        && self.entity_fluid_submersion(bank_x, floor as f64, half, height)
                            == (0.0, 0.0)
                        && self.has_line_of_sight(
                            x,
                            surface as f64 - height * 0.6,
                            bank_x,
                            floor as f64 - height * 0.6,
                        )
                    {
                        return Some((bank_x, floor as f64));
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pond() -> GameState {
        let mut s = GameState::network_view();
        s.dismiss_startup_splash();
        for cx in -2..=2 {
            s.world
                .apply_chunk_column_snapshot(cx, &vec![BlockType::Air; CHUNK_WIDTH * CHUNK_HEIGHT]);
        }
        for x in -24..24 {
            s.world.set_block(x, 20, BlockType::Stone);
            for y in 15..20 {
                s.world.set_block(
                    x,
                    y,
                    if (-4..=4).contains(&x) {
                        BlockType::Water(8)
                    } else {
                        BlockType::Stone
                    },
                );
            }
        }
        s.game_rules.do_mob_spawning = false;
        s.game_rules.do_weather_cycle = false;
        s.game_rules.do_daylight_cycle = false;
        s.player = Player::new(18.5, 15.0);
        s
    }

    #[test]
    fn every_land_animal_reaches_air_and_leaves_a_deep_pond() {
        macro_rules! escapes {
            ($field:ident, $kind:ident, $half:expr, $height:expr) => {{
                let mut s = pond();
                s.$field.push($kind::new(0.5, 19.9));
                let initial_health = s.$field[0].health;
                let mut breathed = false;
                let mut escaped = false;
                for _ in 0..260 {
                    s.update(0, 0);
                    assert_eq!(
                        s.$field.len(),
                        1,
                        "{} died in open water",
                        stringify!($field)
                    );
                    let e = &s.$field[0];
                    breathed |= !matches!(
                        s.world.fluid_at_point(e.x, e.y - $height + 0.25),
                        BlockType::Water(_)
                    );
                    escaped |= e.x.abs() > 5.6 && e.y <= 15.01;
                    assert!(
                        !s.entity_collides_with_world(e.x, e.y, $half, $height),
                        "{} clipped into bank",
                        stringify!($field)
                    );
                }
                let e = &s.$field[0];
                assert!(
                    breathed && escaped,
                    "{} failed to leave pond: {}, {}",
                    stringify!($field),
                    e.x,
                    e.y
                );
                assert_eq!(e.health, initial_health);
            }};
        }
        escapes!(cows, Cow, 0.6, 1.375);
        escapes!(sheep, Sheep, 0.55, 1.125);
        escapes!(pigs, Pig, 0.5, 1.0);
        escapes!(chickens, Chicken, 0.3, 1.0);
        escapes!(wolves, Wolf, 0.6, 0.875);
        escapes!(ocelots, Ocelot, 0.6, 0.75);
    }

    #[test]
    fn animals_turn_before_water_and_lava_but_keep_knockback() {
        for fluid in [BlockType::Water(8), BlockType::Lava(8)] {
            let mut s = pond();
            for y in 15..20 {
                s.world.set_block(4, y, fluid);
            }
            let movement =
                s.calculate_animal_movement(5.8, 15.0, -0.1, -0.4, false, 0.6, 1.375, false);
            assert!(
                movement.2 > 0.0 && movement.3 >= 0.0,
                "must turn and cancel a hop at the shore"
            );
            let hurt = s.calculate_animal_movement(5.8, 15.0, -0.3, -0.2, true, 0.6, 1.375, true);
            let expected = s.calculate_movement_with_jump_held(
                5.8, 15.0, -0.3, -0.2, true, 0.6, 1.375, true, false,
            );
            assert_eq!(hurt, expected);
        }
    }

    #[test]
    fn water_under_a_solid_path_does_not_scare_animals() {
        let mut s = pond();
        s.world.set_block(4, 15, BlockType::Stone);
        let movement = s.calculate_animal_movement(5.8, 15.0, -0.1, 0.0, true, 0.6, 1.375, false);
        assert!(movement.2 < 0.0);
    }

    #[test]
    fn swimming_is_independent_of_player_keys_and_preserves_hurt_impulses() {
        let mut s = pond();
        let a = s.calculate_animal_movement(0.5, 19.0, -0.1, 0.04, false, 0.6, 1.0, false);
        s.jump_held = true;
        s.player.sneaking = true;
        assert_eq!(
            a,
            s.calculate_animal_movement(0.5, 19.0, -0.1, 0.04, false, 0.6, 1.0, false)
        );
        assert_eq!(
            s.calculate_animal_movement(0.5, 19.0, -0.3, -0.2, false, 0.6, 1.0, true),
            s.calculate_movement_with_jump_held(
                0.5, 19.0, -0.3, -0.2, false, 0.6, 1.0, true, false
            )
        );
    }

    #[test]
    fn animals_climb_banks_one_block_above_the_water_surface_in_both_directions() {
        for (half, height) in [
            (0.6, 1.375),
            (0.55, 1.125),
            (0.5, 1.0),
            (0.3, 1.0),
            (0.6, 0.875),
            (0.6, 0.75),
        ] {
            for direction in [-1.0, 1.0] {
                let mut s = pond();
                for x in -24..24 {
                    if !(-4..=4).contains(&x) {
                        s.world.set_block(x, 14, BlockType::Stone);
                    }
                }
                let (mut x, mut y, mut vx, mut vy, mut grounded) =
                    (0.5, 19.9, direction * 0.1, 0.0, false);
                let mut escaped = false;
                for _ in 0..240 {
                    let next =
                        s.calculate_animal_movement(x, y, vx, vy, grounded, half, height, false);
                    (x, y, vx, vy, grounded) = (next.0, next.1, next.2, next.3, next.4);
                    escaped |= y <= 14.01 && !(-4.6..=5.6).contains(&x);
                }
                assert!(
                    escaped,
                    "failed to climb raised bank: direction {direction}, x={x}, y={y}"
                );
            }
        }
    }

    #[test]
    fn submerged_placement_protects_other_players_and_consumes_only_the_builders_stack() {
        let mut s = pond();
        s.active_peer = Some(1);
        s.player = Player::new(0.5, 16.0);
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Stone,
            count: 3,
            durability: None,
        });
        s.hotbar_index = 0;
        let original = s.player.clone();
        s.player = Player::new(2.5, 16.0);
        s.cooperative_players.insert(2, PlayerSession::capture(&s));
        s.player = original;
        s.world.set_block(2, 16, BlockType::Stone);
        s.apply_client_command(ClientCommand::UseAt(2, 15));
        assert_eq!(s.world.get_block(2, 15), BlockType::Water(8));
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 3);
        s.cooperative_players.get_mut(&2).unwrap().player.x = 8.5;
        s.apply_client_command(ClientCommand::UseAt(2, 15));
        assert_eq!(s.world.get_block(2, 15), BlockType::Stone);
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 2);
        assert_eq!(
            s.cooperative_players[&2].inventory.slots[0]
                .as_ref()
                .unwrap()
                .count,
            3
        );
    }

    #[test]
    fn roofed_water_still_drowns_animals_and_never_lifts_them_through_stone() {
        let mut s = pond();
        for x in -4..=4 {
            s.world.set_block(x, 14, BlockType::Stone);
        }
        s.cows.push(Cow::new(0.5, 19.9));
        let mut damaged = false;
        for _ in 0..350 {
            s.update(0, 0);
            if s.cows.is_empty() {
                damaged = true;
                break;
            }
            assert!(s.cows[0].y >= 15.0 + 1.375);
            damaged |= s.cows[0].health < 8.0;
        }
        assert!(
            damaged,
            "swimming must not disable drowning in a sealed tank"
        );
    }

    #[test]
    fn solid_blocks_replace_source_flow_and_falling_water_once() {
        for level in [1, 7, 8, 9] {
            let mut s = pond();
            s.player = Player::new(0.5, 16.0);
            s.inventory.slots[0] = Some(ItemStack {
                item_type: ItemType::Cobblestone,
                count: 2,
                durability: None,
            });
            s.hotbar_index = 0;
            s.world.set_block(2, 16, BlockType::Stone);
            s.world.set_block(2, 15, BlockType::Water(level));
            s.interact_block(2, 15, false);
            assert_eq!(s.world.get_block(2, 15), BlockType::Cobblestone);
            assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 1);
            for _ in 0..5 {
                s.world.update(0);
            }
            assert_eq!(s.world.get_block(2, 15), BlockType::Cobblestone);
        }
    }

    #[test]
    fn aiming_into_water_builds_against_a_reachable_bank_not_floating_in_midwater() {
        let mut s = pond();
        s.player = Player::new(1.5, 17.0);
        s.inventory.slots[0] = Some(ItemStack {
            item_type: ItemType::Stone,
            count: 3,
            durability: None,
        });
        s.hotbar_index = 0;
        s.interact_block(3, 16, false);
        assert_eq!(s.world.get_block(4, 16), BlockType::Stone);
        assert_eq!(s.world.get_block(3, 16), BlockType::Water(8));
        assert_eq!(s.inventory.slots[0].as_ref().unwrap().count, 2);
        s.player = Player::new(0.5, 15.0);
        s.interact_block(0, 15, false);
        assert_eq!(
            s.inventory.slots[0].as_ref().unwrap().count,
            2,
            "out of reach backing must not consume a block"
        );
        s.player = Player::new(0.5, 17.0);
        s.interact_block(0, 19, false);
        assert_eq!(
            s.inventory.slots[0].as_ref().unwrap().count,
            1,
            "reachable pool floor should support a block"
        );
        s.world.set_block(1, 16, BlockType::Stone);
        s.interact_block(0, 16, false);
        assert_eq!(
            s.inventory.slots[0].as_ref().unwrap().count,
            1,
            "no floating blocks or placement inside the player"
        );
    }
}
