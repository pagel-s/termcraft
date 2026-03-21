use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug)]
pub enum BlockType {
    Air,
    Dirt,
    Grass,
    Stone,
    Wood,
    Leaves,
    IronOre,
    GoldOre,
    DiamondOre,
    CoalOre,
    Sand,
    Gravel,
    Bedrock,
    Planks,
    CraftingTable,
    Torch,
    Lever(bool),
    StoneButton(u8),
    RedstoneTorch(bool),
    Furnace,
    Snow,
    Ice,
    Cactus,
    DeadBush,
    BirchWood,
    BirchLeaves,
    RedFlower,
    YellowFlower,
    TallGrass,
    // Fluids and fluid-derived blocks
    Water(u8),
    Lava(u8),
    Cobblestone,
    Obsidian,
    Farmland(u8),
    Crops(u8),
    RedstoneOre,
    RedstoneDust(u8),
    Netherrack,
    SoulSand,
    Glowstone,
    NetherPortal,
    Tnt,
    PrimedTnt(u8),
    Piston {
        extended: bool,
        facing_right: bool,
    },
    StickyPiston {
        extended: bool,
        facing_right: bool,
    },
    StoneBricks,
    EndPortalFrame {
        filled: bool,
    },
    EndPortal,
    EndStone,
    // Appended variants to preserve bincode enum compatibility for existing saves.
    IronDoor(bool),
    SilverfishSpawner,
    Bed,
    Chest,
    Glass,
    WoodDoor(bool),
    Ladder,
    StoneSlab,
    StoneStairs,
    EnchantingTable,
    Anvil,
    BrewingStand,
    BlazeSpawner,
    RedstoneRepeater {
        powered: bool,
        delay: u8,
        facing_right: bool,
        timer: u8,
        target_powered: bool,
    },
    Wool,
    Sapling,
    SugarCane,
    Bookshelf,
    NetherWart(u8),
    BirchSapling,
    ZombieSpawner,
    SkeletonSpawner,
}

impl BlockType {
    pub fn is_solid(&self) -> bool {
        !matches!(
            self,
            BlockType::Air
                | BlockType::Leaves
                | BlockType::BirchLeaves
                | BlockType::Torch
                | BlockType::Lever(_)
                | BlockType::StoneButton(_)
                | BlockType::RedstoneTorch(_)
                | BlockType::RedstoneRepeater { .. }
                | BlockType::RedFlower
                | BlockType::YellowFlower
                | BlockType::TallGrass
                | BlockType::RedstoneDust(_)
                | BlockType::NetherPortal
                | BlockType::EndPortal
                | BlockType::PrimedTnt(_)
                | BlockType::IronDoor(true)
                | BlockType::WoodDoor(true)
                | BlockType::Crops(_)
                | BlockType::DeadBush
                | BlockType::Ladder
                | BlockType::Water(_)
                | BlockType::Lava(_)
                | BlockType::Sapling
                | BlockType::BirchSapling
                | BlockType::SugarCane
                | BlockType::NetherWart(_)
        )
    }

    pub fn is_replaceable(&self) -> bool {
        matches!(
            self,
            BlockType::Air
                | BlockType::Torch
                | BlockType::Lever(_)
                | BlockType::StoneButton(_)
                | BlockType::RedstoneTorch(_)
                | BlockType::RedstoneRepeater { .. }
                | BlockType::RedFlower
                | BlockType::YellowFlower
                | BlockType::TallGrass
                | BlockType::Crops(_)
                | BlockType::DeadBush
                | BlockType::Ladder
                | BlockType::RedstoneDust(_)
                | BlockType::Water(_)
                | BlockType::Lava(_)
                | BlockType::Sapling
                | BlockType::BirchSapling
                | BlockType::NetherWart(_)
        )
    }

    pub fn hardness(&self) -> f32 {
        match self {
            BlockType::Air
            | BlockType::Torch
            | BlockType::Lever(_)
            | BlockType::StoneButton(_)
            | BlockType::RedstoneTorch(_)
            | BlockType::RedstoneRepeater { .. }
            | BlockType::RedFlower
            | BlockType::YellowFlower
            | BlockType::TallGrass
            | BlockType::Crops(_)
            | BlockType::DeadBush
            | BlockType::RedstoneDust(_)
            | BlockType::NetherPortal
            | BlockType::EndPortal
            | BlockType::Tnt
            | BlockType::PrimedTnt(_)
            | BlockType::Water(_)
            | BlockType::Lava(_)
            | BlockType::Sapling
            | BlockType::BirchSapling
            | BlockType::SugarCane
            | BlockType::NetherWart(_) => 0.0,
            BlockType::Ladder => 0.4,
            BlockType::Glass => 0.3,
            BlockType::IronDoor(_) => 5.0,
            BlockType::WoodDoor(_) => 3.0,
            BlockType::SilverfishSpawner => 5.0,
            BlockType::BlazeSpawner => 5.0,
            BlockType::ZombieSpawner => 5.0,
            BlockType::SkeletonSpawner => 5.0,
            BlockType::Anvil => 5.0,
            BlockType::Glowstone => 0.3,
            BlockType::Bed => 0.2,
            BlockType::Chest => 2.5,
            BlockType::Wool => 0.8,
            BlockType::EnchantingTable => 5.0,
            BlockType::BrewingStand => 0.5,
            BlockType::Leaves | BlockType::BirchLeaves | BlockType::Snow => 0.2,
            BlockType::Dirt | BlockType::Grass | BlockType::Sand | BlockType::Farmland(_) => 0.5,
            BlockType::SoulSand => 0.6,
            BlockType::Gravel => 0.6,
            BlockType::Wood
            | BlockType::BirchWood
            | BlockType::Planks
            | BlockType::CraftingTable
            | BlockType::Furnace
            | BlockType::Bookshelf => 2.0,
            BlockType::Stone
            | BlockType::StoneBricks
            | BlockType::CoalOre
            | BlockType::Ice
            | BlockType::Cobblestone
            | BlockType::Netherrack
            | BlockType::EndStone
            | BlockType::StoneSlab
            | BlockType::StoneStairs
            | BlockType::Piston { .. }
            | BlockType::StickyPiston { .. } => 1.5,
            BlockType::EndPortalFrame { .. } => 8.0,
            BlockType::IronOre => 3.0,
            BlockType::RedstoneOre => 3.0,
            BlockType::GoldOre => 3.0,
            BlockType::DiamondOre => 4.0,
            BlockType::Obsidian => 10.0, // Very slow to mine
            BlockType::Cactus => 0.4,
            BlockType::Bedrock => -1.0,
        }
    }

    pub fn required_tool_level(&self) -> u8 {
        match self {
            BlockType::Stone
            | BlockType::StoneBricks
            | BlockType::CoalOre
            | BlockType::Furnace
            | BlockType::Ice
            | BlockType::Cobblestone
            | BlockType::Netherrack
            | BlockType::EndStone
            | BlockType::SilverfishSpawner
            | BlockType::BlazeSpawner
            | BlockType::ZombieSpawner
            | BlockType::SkeletonSpawner
            | BlockType::Piston { .. }
            | BlockType::StickyPiston { .. }
            | BlockType::StoneSlab
            | BlockType::StoneStairs
            | BlockType::EnchantingTable
            | BlockType::Anvil
            | BlockType::BrewingStand => 1,
            BlockType::IronDoor(_) => 2,
            BlockType::EndPortalFrame { .. } => 4,
            BlockType::IronOre => 2,
            BlockType::RedstoneOre => 3,
            BlockType::GoldOre | BlockType::DiamondOre => 3,
            BlockType::Obsidian => 4, // Needs Diamond Pickaxe
            BlockType::Bedrock => 255,
            _ => 0,
        }
    }

    pub fn obeys_gravity(&self) -> bool {
        matches!(self, BlockType::Sand | BlockType::Gravel)
    }

    pub fn is_fluid(&self) -> bool {
        matches!(self, BlockType::Water(_) | BlockType::Lava(_))
    }

    pub fn is_leaf_block(&self) -> bool {
        matches!(self, BlockType::Leaves | BlockType::BirchLeaves)
    }

    pub fn participates_in_farming_tick(&self) -> bool {
        matches!(
            self,
            BlockType::Farmland(_)
                | BlockType::Crops(_)
                | BlockType::Sapling
                | BlockType::BirchSapling
                | BlockType::SugarCane
                | BlockType::NetherWart(_)
        )
    }

    pub fn needs_bottom_support(&self) -> bool {
        matches!(
            self,
            BlockType::RedFlower
                | BlockType::YellowFlower
                | BlockType::TallGrass
                | BlockType::DeadBush
                | BlockType::Crops(_)
                | BlockType::Sapling
                | BlockType::BirchSapling
                | BlockType::SugarCane
                | BlockType::NetherWart(_)
                | BlockType::Cactus
        )
    }

    pub fn can_stay_on(&self, ground: BlockType) -> bool {
        match self {
            BlockType::RedFlower | BlockType::YellowFlower | BlockType::TallGrass => {
                matches!(
                    ground,
                    BlockType::Grass | BlockType::Dirt | BlockType::Farmland(_)
                )
            }
            BlockType::DeadBush => {
                matches!(ground, BlockType::Sand | BlockType::Dirt | BlockType::Grass)
            }
            BlockType::Crops(_) => matches!(ground, BlockType::Farmland(_)),
            BlockType::Sapling | BlockType::BirchSapling => {
                matches!(ground, BlockType::Grass | BlockType::Dirt)
            }
            BlockType::NetherWart(_) => matches!(ground, BlockType::SoulSand),
            BlockType::SugarCane => matches!(
                ground,
                BlockType::Grass | BlockType::Dirt | BlockType::Sand | BlockType::SugarCane
            ),
            BlockType::Cactus => matches!(ground, BlockType::Sand | BlockType::Cactus),
            _ => true,
        }
    }

    pub fn participates_in_redstone_tick(&self) -> bool {
        matches!(
            self,
            BlockType::StoneButton(_)
                | BlockType::RedstoneTorch(_)
                | BlockType::RedstoneRepeater { .. }
                | BlockType::RedstoneDust(_)
                | BlockType::Tnt
                | BlockType::PrimedTnt(_)
                | BlockType::Piston { .. }
                | BlockType::StickyPiston { .. }
        )
    }

    pub fn is_pickaxe_effective(&self) -> bool {
        matches!(
            self,
            BlockType::Stone
                | BlockType::StoneBricks
                | BlockType::CoalOre
                | BlockType::IronOre
                | BlockType::RedstoneOre
                | BlockType::GoldOre
                | BlockType::DiamondOre
                | BlockType::Ice
                | BlockType::Cobblestone
                | BlockType::Obsidian
                | BlockType::Furnace
                | BlockType::Netherrack
                | BlockType::EndStone
                | BlockType::Glass
                | BlockType::StoneSlab
                | BlockType::StoneStairs
                | BlockType::SilverfishSpawner
                | BlockType::BlazeSpawner
                | BlockType::ZombieSpawner
                | BlockType::SkeletonSpawner
                | BlockType::Piston { .. }
                | BlockType::StickyPiston { .. }
                | BlockType::EnchantingTable
                | BlockType::Anvil
                | BlockType::BrewingStand
        )
    }

    pub fn is_axe_effective(&self) -> bool {
        matches!(
            self,
            BlockType::Wood
                | BlockType::BirchWood
                | BlockType::Planks
                | BlockType::CraftingTable
                | BlockType::Bed
                | BlockType::Chest
                | BlockType::Wool
                | BlockType::WoodDoor(_)
                | BlockType::Ladder
                | BlockType::Bookshelf
        )
    }

    pub fn is_shovel_effective(&self) -> bool {
        matches!(
            self,
            BlockType::Dirt
                | BlockType::Grass
                | BlockType::Sand
                | BlockType::Gravel
                | BlockType::Farmland(_)
                | BlockType::SoulSand
                | BlockType::Snow
        )
    }
}
