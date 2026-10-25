//! Frozen native-v9 layouts. Serde defaults cannot extend bincode structs.
use super::*;

#[derive(Clone, Debug, Deserialize)]
pub(super) struct ItemEntityV9 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    item_type: ItemType,
    grounded: bool,
    age: u64,
}
impl From<ItemEntityV9> for ItemEntity {
    fn from(e: ItemEntityV9) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            item_type: e.item_type,
            grounded: e.grounded,
            age: e.age,
            payload: None,
            pickup_delay: 0,
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct FireballV9 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    age: u64,
    dead: bool,
}
impl From<FireballV9> for Fireball {
    fn from(e: FireballV9) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            age: e.age,
            dead: e.dead,
            reflected: false,
        }
    }
}

// Frozen creature layouts from native v10 (also used by v9).
#[derive(Clone, Debug, Deserialize)]
pub(super) struct ZombieV10 {
    director_event: u64,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    age: u64,
    health: f32,
    burning_timer: i32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    attack_cooldown: u8,
    attack_windup: u8,
    stuck_ticks: u8,
    reroute_ticks: u8,
    reroute_dir: i8,
}
impl From<ZombieV10> for Zombie {
    fn from(e: ZombieV10) -> Self {
        Self {
            director_event: e.director_event,
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            age: e.age,
            health: e.health,
            burning_timer: e.burning_timer,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            attack_cooldown: e.attack_cooldown,
            attack_windup: e.attack_windup,
            stuck_ticks: e.stuck_ticks,
            reroute_ticks: e.reroute_ticks,
            reroute_dir: e.reroute_dir,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct ZombiePigmanV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    aggressive_timer: u16,
    attack_cooldown: u8,
    attack_windup: u8,
    stuck_ticks: u8,
    reroute_ticks: u8,
    reroute_dir: i8,
}
impl From<ZombiePigmanV10> for ZombiePigman {
    fn from(e: ZombiePigmanV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            aggressive_timer: e.aggressive_timer,
            attack_cooldown: e.attack_cooldown,
            attack_windup: e.attack_windup,
            stuck_ticks: e.stuck_ticks,
            reroute_ticks: e.reroute_ticks,
            reroute_dir: e.reroute_dir,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct SkeletonV10 {
    director_event: u64,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    age: u64,
    health: f32,
    burning_timer: i32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    bow_cooldown: u8,
    bow_windup: u8,
    aim_x: f64,
    aim_y: f64,
    stuck_ticks: u8,
    reroute_ticks: u8,
    reroute_dir: i8,
}
impl From<SkeletonV10> for Skeleton {
    fn from(e: SkeletonV10) -> Self {
        Self {
            director_event: e.director_event,
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            age: e.age,
            health: e.health,
            burning_timer: e.burning_timer,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            bow_cooldown: e.bow_cooldown,
            bow_windup: e.bow_windup,
            aim_x: e.aim_x,
            aim_y: e.aim_y,
            stuck_ticks: e.stuck_ticks,
            reroute_ticks: e.reroute_ticks,
            reroute_dir: e.reroute_dir,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct CreeperV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    fuse_timer: u8,
    charged: bool,
    stuck_ticks: u8,
    reroute_ticks: u8,
    reroute_dir: i8,
}
impl From<CreeperV10> for Creeper {
    fn from(e: CreeperV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            fuse_timer: e.fuse_timer,
            charged: e.charged,
            stuck_ticks: e.stuck_ticks,
            reroute_ticks: e.reroute_ticks,
            reroute_dir: e.reroute_dir,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct SpiderV10 {
    director_event: u64,
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    attack_cooldown: u8,
    attack_windup: u8,
    stuck_ticks: u8,
    reroute_ticks: u8,
    reroute_dir: i8,
}
impl From<SpiderV10> for Spider {
    fn from(e: SpiderV10) -> Self {
        Self {
            director_event: e.director_event,
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            attack_cooldown: e.attack_cooldown,
            attack_windup: e.attack_windup,
            stuck_ticks: e.stuck_ticks,
            reroute_ticks: e.reroute_ticks,
            reroute_dir: e.reroute_dir,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct SilverfishV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    attack_cooldown: u8,
    attack_windup: u8,
    stuck_ticks: u8,
    reroute_ticks: u8,
    reroute_dir: i8,
}
impl From<SilverfishV10> for Silverfish {
    fn from(e: SilverfishV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            attack_cooldown: e.attack_cooldown,
            attack_windup: e.attack_windup,
            stuck_ticks: e.stuck_ticks,
            reroute_ticks: e.reroute_ticks,
            reroute_dir: e.reroute_dir,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct SlimeV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    jump_cooldown: u8,
    attack_cooldown: u8,
    attack_windup: u8,
    size: u8,
    stuck_ticks: u8,
    reroute_ticks: u8,
    reroute_dir: i8,
}
impl From<SlimeV10> for Slime {
    fn from(e: SlimeV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            jump_cooldown: e.jump_cooldown,
            attack_cooldown: e.attack_cooldown,
            attack_windup: e.attack_windup,
            size: e.size,
            stuck_ticks: e.stuck_ticks,
            reroute_ticks: e.reroute_ticks,
            reroute_dir: e.reroute_dir,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct EndermanV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    aggressive_timer: u16,
    teleport_cooldown: u8,
    attack_cooldown: u8,
    attack_windup: u8,
    stuck_ticks: u8,
    reroute_ticks: u8,
    reroute_dir: i8,
}
impl From<EndermanV10> for Enderman {
    fn from(e: EndermanV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            aggressive_timer: e.aggressive_timer,
            teleport_cooldown: e.teleport_cooldown,
            attack_cooldown: e.attack_cooldown,
            attack_windup: e.attack_windup,
            stuck_ticks: e.stuck_ticks,
            reroute_ticks: e.reroute_ticks,
            reroute_dir: e.reroute_dir,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct GhastV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    facing_right: bool,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    shoot_cooldown: u8,
}
impl From<GhastV10> for Ghast {
    fn from(e: GhastV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            facing_right: e.facing_right,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            shoot_cooldown: e.shoot_cooldown,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct BlazeV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    facing_right: bool,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    shoot_cooldown: u8,
    attack_cooldown: u8,
    attack_windup: u8,
}
impl From<BlazeV10> for Blaze {
    fn from(e: BlazeV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            facing_right: e.facing_right,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            shoot_cooldown: e.shoot_cooldown,
            attack_cooldown: e.attack_cooldown,
            attack_windup: e.attack_windup,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct CowV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    wander_timer: u16,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
}
impl From<CowV10> for Cow {
    fn from(e: CowV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            hazards: Default::default(),
            husbandry: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct SheepV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    wander_timer: u16,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    sheared: bool,
}
impl From<SheepV10> for Sheep {
    fn from(e: SheepV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            sheared: e.sheared,
            hazards: Default::default(),
            husbandry: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct PigV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    wander_timer: u16,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
}
impl From<PigV10> for Pig {
    fn from(e: PigV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            hazards: Default::default(),
            husbandry: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct ChickenV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    wander_timer: u16,
    egg_lay_timer: u16,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
}
impl From<ChickenV10> for Chicken {
    fn from(e: ChickenV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            egg_lay_timer: e.egg_lay_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            hazards: Default::default(),
            husbandry: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct SquidV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    wander_timer: u16,
    flop_cooldown: u8,
    swim_dir_x: f64,
    swim_dir_y: f64,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
}
impl From<SquidV10> for Squid {
    fn from(e: SquidV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            wander_timer: e.wander_timer,
            flop_cooldown: e.flop_cooldown,
            swim_dir_x: e.swim_dir_x,
            swim_dir_y: e.swim_dir_y,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct WolfV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    wander_timer: u16,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    aggressive_timer: u16,
    attack_cooldown: u8,
    attack_windup: u8,
}
impl From<WolfV10> for Wolf {
    fn from(e: WolfV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            aggressive_timer: e.aggressive_timer,
            attack_cooldown: e.attack_cooldown,
            attack_windup: e.attack_windup,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct OcelotV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    wander_timer: u16,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    panic_timer: u16,
    attack_cooldown: u8,
}
impl From<OcelotV10> for Ocelot {
    fn from(e: OcelotV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            panic_timer: e.panic_timer,
            attack_cooldown: e.attack_cooldown,
            hazards: Default::default(),
        }
    }
}
#[derive(Clone, Debug, Deserialize)]
pub(super) struct VillagerV10 {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    grounded: bool,
    facing_right: bool,
    jump_cooldown: u8,
    wander_timer: u16,
    age: u64,
    health: f32,
    hit_timer: u8,
    last_player_damage_tick: u64,
    home_x: i32,
    home_y: i32,
}
impl From<VillagerV10> for Villager {
    fn from(e: VillagerV10) -> Self {
        Self {
            x: e.x,
            y: e.y,
            vx: e.vx,
            vy: e.vy,
            grounded: e.grounded,
            facing_right: e.facing_right,
            jump_cooldown: e.jump_cooldown,
            wander_timer: e.wander_timer,
            age: e.age,
            health: e.health,
            hit_timer: e.hit_timer,
            last_player_damage_tick: e.last_player_damage_tick,
            home_x: e.home_x,
            home_y: e.home_y,
            hazards: Default::default(),
        }
    }
}
