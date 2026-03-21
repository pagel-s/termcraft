use super::block::BlockType;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Debug)]
pub enum ItemType {
    Dirt,
    Grass,
    Stone,
    Wood,
    Leaves,
    Stick,
    WoodPickaxe,
    StonePickaxe,
    IronPickaxe,
    DiamondPickaxe,
    Coal,
    Sand,
    Gravel,
    Planks,
    CraftingTable,
    IronIngot,
    Diamond,
    Torch,
    Lever,
    StoneButton,
    RedstoneTorch,
    Furnace,
    RawIron,
    RawGold,
    GoldIngot,
    WoodSword,
    StoneSword,
    IronSword,
    DiamondSword,
    WoodHoe,
    StoneHoe,
    IronHoe,
    DiamondHoe,
    WheatSeeds,
    Wheat,
    Bread,
    RawBeef,
    CookedBeef,
    RottenFlesh,
    Bone,
    Arrow,
    Bow,
    Gunpowder,
    String,
    Leather,
    Wool,
    RawMutton,
    CookedMutton,
    // Biome Items
    Snow,
    Ice,
    Cactus,
    DeadBush,
    BirchWood,
    BirchLeaves,
    RedFlower,
    YellowFlower,
    TallGrass,
    // New Items
    WaterBucket,
    LavaBucket,
    Cobblestone,
    Obsidian,
    RedstoneDust,
    Netherrack,
    SoulSand,
    Glowstone,
    Tnt,
    Piston,
    EndStone,
    EyeOfEnder,
    EnderPearl,
    BlazeRod,
    BlazePowder,
    Bed,
    Chest,
    // Appended variants to preserve bincode enum compatibility for existing saves.
    WoodAxe,
    StoneAxe,
    IronAxe,
    DiamondAxe,
    WoodShovel,
    StoneShovel,
    IronShovel,
    DiamondShovel,
    Glass,
    WoodDoor,
    Ladder,
    StoneSlab,
    StoneStairs,
    LeatherHelmet,
    LeatherChestplate,
    LeatherLeggings,
    LeatherBoots,
    IronHelmet,
    IronChestplate,
    IronLeggings,
    IronBoots,
    DiamondHelmet,
    DiamondChestplate,
    DiamondLeggings,
    DiamondBoots,
    Slimeball,
    RawPorkchop,
    CookedPorkchop,
    RawChicken,
    CookedChicken,
    Feather,
    Egg,
    EnchantingTable,
    Anvil,
    BrewingStand,
    GlassBottle,
    WaterBottle,
    NetherWart,
    AwkwardPotion,
    PotionHealing,
    PotionStrength,
    PotionRegeneration,
    PotionFireResistance,
    GhastTear,
    MagmaCream,
    FishingRod,
    RawFish,
    CookedFish,
    RedstoneRepeater,
    // Appended to preserve bincode enum compatibility for existing saves.
    Bucket,
    Sapling,
    BoneMeal,
    Flint,
    FlintAndSteel,
    Shears,
    SugarCane,
    Paper,
    Book,
    Bookshelf,
    Boat,
    BirchSapling,
}

impl ItemType {
    pub fn from_block(block: BlockType) -> Option<Self> {
        match block {
            BlockType::Air => None,
            BlockType::Dirt | BlockType::Grass => Some(ItemType::Dirt),
            BlockType::Stone => Some(ItemType::Cobblestone),
            BlockType::StoneBricks => Some(ItemType::Cobblestone),
            BlockType::Wood => Some(ItemType::Wood),
            BlockType::Leaves => Some(ItemType::Leaves),
            BlockType::IronOre => Some(ItemType::RawIron),
            BlockType::GoldOre => Some(ItemType::RawGold),
            BlockType::DiamondOre => Some(ItemType::Diamond),
            BlockType::CoalOre => Some(ItemType::Coal),
            BlockType::RedstoneOre => Some(ItemType::RedstoneDust),
            BlockType::Sand => Some(ItemType::Sand),
            BlockType::Gravel => Some(ItemType::Gravel),
            BlockType::Planks => Some(ItemType::Planks),
            BlockType::CraftingTable => Some(ItemType::CraftingTable),
            BlockType::Torch => Some(ItemType::Torch),
            BlockType::Lever(_) => Some(ItemType::Lever),
            BlockType::StoneButton(_) => Some(ItemType::StoneButton),
            BlockType::RedstoneTorch(_) => Some(ItemType::RedstoneTorch),
            BlockType::Furnace => Some(ItemType::Furnace),
            BlockType::Snow => Some(ItemType::Snow),
            BlockType::Ice => Some(ItemType::Ice),
            BlockType::Cactus => Some(ItemType::Cactus),
            BlockType::DeadBush => Some(ItemType::Stick),
            BlockType::BirchWood => Some(ItemType::BirchWood),
            BlockType::BirchLeaves => Some(ItemType::BirchLeaves),
            BlockType::RedFlower => Some(ItemType::RedFlower),
            BlockType::YellowFlower => Some(ItemType::YellowFlower),
            // Fluids are not mined into buckets; bucket fill/empty is handled as interaction.
            BlockType::Water(_) | BlockType::Lava(_) => None,
            BlockType::Cobblestone => Some(ItemType::Cobblestone),
            BlockType::Obsidian => Some(ItemType::Obsidian),
            BlockType::RedstoneDust(_) => Some(ItemType::RedstoneDust),
            BlockType::Netherrack => Some(ItemType::Netherrack),
            BlockType::SoulSand => Some(ItemType::SoulSand),
            BlockType::Glowstone => Some(ItemType::Glowstone),
            BlockType::EndStone => Some(ItemType::EndStone),
            BlockType::Tnt => Some(ItemType::Tnt),
            BlockType::Piston { .. } | BlockType::StickyPiston { .. } => Some(ItemType::Piston),
            BlockType::Bed => Some(ItemType::Bed),
            BlockType::Chest => Some(ItemType::Chest),
            BlockType::Glass => Some(ItemType::Glass),
            BlockType::WoodDoor(_) => Some(ItemType::WoodDoor),
            BlockType::Ladder => Some(ItemType::Ladder),
            BlockType::StoneSlab => Some(ItemType::StoneSlab),
            BlockType::StoneStairs => Some(ItemType::StoneStairs),
            BlockType::EnchantingTable => Some(ItemType::EnchantingTable),
            BlockType::Anvil => Some(ItemType::Anvil),
            BlockType::BrewingStand => Some(ItemType::BrewingStand),
            BlockType::RedstoneRepeater { .. } => Some(ItemType::RedstoneRepeater),
            BlockType::Wool => Some(ItemType::Wool),
            BlockType::Sapling => Some(ItemType::Sapling),
            BlockType::BirchSapling => Some(ItemType::BirchSapling),
            BlockType::SugarCane => Some(ItemType::SugarCane),
            BlockType::Bookshelf => Some(ItemType::Bookshelf),
            BlockType::NetherWart(_) => Some(ItemType::NetherWart),
            BlockType::Farmland(_) => Some(ItemType::Dirt),
            BlockType::Crops(7) => Some(ItemType::Wheat),
            BlockType::Crops(_) => Some(ItemType::WheatSeeds),
            BlockType::TallGrass
            | BlockType::Bedrock
            | BlockType::PrimedTnt(_)
            | BlockType::NetherPortal
            | BlockType::EndPortalFrame { .. }
            | BlockType::EndPortal
            | BlockType::IronDoor(_)
            | BlockType::SilverfishSpawner
            | BlockType::BlazeSpawner
            | BlockType::ZombieSpawner
            | BlockType::SkeletonSpawner => None,
        }
    }

    pub fn to_block(&self) -> Option<BlockType> {
        match self {
            ItemType::Dirt => Some(BlockType::Dirt),
            ItemType::Grass => Some(BlockType::Grass),
            ItemType::Stone => Some(BlockType::Stone),
            ItemType::Wood => Some(BlockType::Wood),
            ItemType::Leaves => Some(BlockType::Leaves),
            ItemType::Sand => Some(BlockType::Sand),
            ItemType::Gravel => Some(BlockType::Gravel),
            ItemType::Planks => Some(BlockType::Planks),
            ItemType::CraftingTable => Some(BlockType::CraftingTable),
            ItemType::Torch => Some(BlockType::Torch),
            ItemType::Lever => Some(BlockType::Lever(false)),
            ItemType::StoneButton => Some(BlockType::StoneButton(0)),
            ItemType::RedstoneTorch => Some(BlockType::RedstoneTorch(true)),
            ItemType::Furnace => Some(BlockType::Furnace),
            ItemType::RedstoneDust => Some(BlockType::RedstoneDust(0)),
            ItemType::Snow => Some(BlockType::Snow),
            ItemType::Ice => Some(BlockType::Ice),
            ItemType::Cactus => Some(BlockType::Cactus),
            ItemType::DeadBush => Some(BlockType::DeadBush),
            ItemType::BirchWood => Some(BlockType::BirchWood),
            ItemType::BirchLeaves => Some(BlockType::BirchLeaves),
            ItemType::RedFlower => Some(BlockType::RedFlower),
            ItemType::YellowFlower => Some(BlockType::YellowFlower),
            ItemType::TallGrass => Some(BlockType::TallGrass),
            ItemType::WaterBucket => Some(BlockType::Water(8)),
            ItemType::LavaBucket => Some(BlockType::Lava(8)),
            ItemType::Cobblestone => Some(BlockType::Cobblestone),
            ItemType::Obsidian => Some(BlockType::Obsidian),
            ItemType::Netherrack => Some(BlockType::Netherrack),
            ItemType::SoulSand => Some(BlockType::SoulSand),
            ItemType::Glowstone => Some(BlockType::Glowstone),
            ItemType::EndStone => Some(BlockType::EndStone),
            ItemType::WheatSeeds => Some(BlockType::Crops(0)),
            ItemType::Tnt => Some(BlockType::Tnt),
            ItemType::Piston => Some(BlockType::Piston {
                extended: false,
                facing_right: true,
            }),
            ItemType::Bed => Some(BlockType::Bed),
            ItemType::Chest => Some(BlockType::Chest),
            ItemType::Glass => Some(BlockType::Glass),
            ItemType::Wool => Some(BlockType::Wool),
            ItemType::Sapling => Some(BlockType::Sapling),
            ItemType::BirchSapling => Some(BlockType::BirchSapling),
            ItemType::SugarCane => Some(BlockType::SugarCane),
            ItemType::Bookshelf => Some(BlockType::Bookshelf),
            ItemType::NetherWart => Some(BlockType::NetherWart(0)),
            ItemType::WoodDoor => Some(BlockType::WoodDoor(false)),
            ItemType::Ladder => Some(BlockType::Ladder),
            ItemType::StoneSlab => Some(BlockType::StoneSlab),
            ItemType::StoneStairs => Some(BlockType::StoneStairs),
            ItemType::EnchantingTable => Some(BlockType::EnchantingTable),
            ItemType::Anvil => Some(BlockType::Anvil),
            ItemType::BrewingStand => Some(BlockType::BrewingStand),
            ItemType::RedstoneRepeater => Some(BlockType::RedstoneRepeater {
                powered: false,
                delay: 1,
                facing_right: true,
                timer: 0,
                target_powered: false,
            }),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ItemType::Dirt => "Dirt",
            ItemType::Grass => "Grass",
            ItemType::Stone => "Stone",
            ItemType::Wood => "Wood",
            ItemType::Leaves => "Leaves",
            ItemType::Stick => "Stick",
            ItemType::WoodPickaxe => "Wood Pickaxe",
            ItemType::StonePickaxe => "Stone Pickaxe",
            ItemType::IronPickaxe => "Iron Pickaxe",
            ItemType::DiamondPickaxe => "Diamond Pickaxe",
            ItemType::Coal => "Coal",
            ItemType::Sand => "Sand",
            ItemType::Gravel => "Gravel",
            ItemType::Planks => "Planks",
            ItemType::CraftingTable => "Crafting Table",
            ItemType::IronIngot => "Iron Ingot",
            ItemType::Diamond => "Diamond",
            ItemType::RedstoneDust => "Redstone Dust",
            ItemType::Torch => "Torch",
            ItemType::Lever => "Lever",
            ItemType::StoneButton => "Stone Button",
            ItemType::RedstoneTorch => "Redstone Torch",
            ItemType::Furnace => "Furnace",
            ItemType::RawIron => "Raw Iron",
            ItemType::RawGold => "Raw Gold",
            ItemType::GoldIngot => "Gold Ingot",
            ItemType::Snow => "Snow",
            ItemType::Ice => "Ice",
            ItemType::Cactus => "Cactus",
            ItemType::DeadBush => "Dead Bush",
            ItemType::BirchWood => "Birch Wood",
            ItemType::BirchLeaves => "Birch Leaves",
            ItemType::RedFlower => "Red Flower",
            ItemType::YellowFlower => "Yellow Flower",
            ItemType::TallGrass => "Tall Grass",
            ItemType::Sapling => "Sapling",
            ItemType::BoneMeal => "Bone Meal",
            ItemType::Flint => "Flint",
            ItemType::FlintAndSteel => "Flint and Steel",
            ItemType::Shears => "Shears",
            ItemType::SugarCane => "Sugar Cane",
            ItemType::Paper => "Paper",
            ItemType::Book => "Book",
            ItemType::Bookshelf => "Bookshelf",
            ItemType::Boat => "Boat",
            ItemType::BirchSapling => "Birch Sapling",
            ItemType::Bucket => "Bucket",
            ItemType::WaterBucket => "Water Bucket",
            ItemType::LavaBucket => "Lava Bucket",
            ItemType::Cobblestone => "Cobblestone",
            ItemType::Obsidian => "Obsidian",
            ItemType::Netherrack => "Netherrack",
            ItemType::SoulSand => "Soul Sand",
            ItemType::Glowstone => "Glowstone",
            ItemType::EndStone => "End Stone",
            ItemType::Tnt => "TNT",
            ItemType::Piston => "Piston",
            ItemType::WoodSword => "Wood Sword",
            ItemType::StoneSword => "Stone Sword",
            ItemType::IronSword => "Iron Sword",
            ItemType::DiamondSword => "Diamond Sword",
            ItemType::WoodHoe => "Wood Hoe",
            ItemType::StoneHoe => "Stone Hoe",
            ItemType::IronHoe => "Iron Hoe",
            ItemType::DiamondHoe => "Diamond Hoe",
            ItemType::WheatSeeds => "Seeds",
            ItemType::Wheat => "Wheat",
            ItemType::Bread => "Bread",
            ItemType::RawBeef => "Raw Beef",
            ItemType::CookedBeef => "Cooked Beef",
            ItemType::RottenFlesh => "Rotten Flesh",
            ItemType::Bone => "Bone",
            ItemType::Arrow => "Arrow",
            ItemType::Bow => "Bow",
            ItemType::EyeOfEnder => "Eye of Ender",
            ItemType::EnderPearl => "Ender Pearl",
            ItemType::BlazeRod => "Blaze Rod",
            ItemType::BlazePowder => "Blaze Powder",
            ItemType::Bed => "Bed",
            ItemType::Chest => "Chest",
            ItemType::WoodAxe => "Wood Axe",
            ItemType::StoneAxe => "Stone Axe",
            ItemType::IronAxe => "Iron Axe",
            ItemType::DiamondAxe => "Diamond Axe",
            ItemType::WoodShovel => "Wood Shovel",
            ItemType::StoneShovel => "Stone Shovel",
            ItemType::IronShovel => "Iron Shovel",
            ItemType::DiamondShovel => "Diamond Shovel",
            ItemType::Glass => "Glass",
            ItemType::WoodDoor => "Wood Door",
            ItemType::Ladder => "Ladder",
            ItemType::StoneSlab => "Stone Slab",
            ItemType::StoneStairs => "Stone Stairs",
            ItemType::LeatherHelmet => "Leather Helmet",
            ItemType::LeatherChestplate => "Leather Chestplate",
            ItemType::LeatherLeggings => "Leather Leggings",
            ItemType::LeatherBoots => "Leather Boots",
            ItemType::IronHelmet => "Iron Helmet",
            ItemType::IronChestplate => "Iron Chestplate",
            ItemType::IronLeggings => "Iron Leggings",
            ItemType::IronBoots => "Iron Boots",
            ItemType::DiamondHelmet => "Diamond Helmet",
            ItemType::DiamondChestplate => "Diamond Chestplate",
            ItemType::DiamondLeggings => "Diamond Leggings",
            ItemType::DiamondBoots => "Diamond Boots",
            ItemType::Slimeball => "Slimeball",
            ItemType::Gunpowder => "Gunpowder",
            ItemType::String => "String",
            ItemType::Leather => "Leather",
            ItemType::Wool => "Wool",
            ItemType::RawMutton => "Raw Mutton",
            ItemType::CookedMutton => "Cooked Mutton",
            ItemType::RawPorkchop => "Raw Porkchop",
            ItemType::CookedPorkchop => "Cooked Porkchop",
            ItemType::RawChicken => "Raw Chicken",
            ItemType::CookedChicken => "Cooked Chicken",
            ItemType::Feather => "Feather",
            ItemType::Egg => "Egg",
            ItemType::EnchantingTable => "Enchanting Table",
            ItemType::Anvil => "Anvil",
            ItemType::BrewingStand => "Brewing Stand",
            ItemType::GlassBottle => "Glass Bottle",
            ItemType::WaterBottle => "Water Bottle",
            ItemType::NetherWart => "Nether Wart",
            ItemType::AwkwardPotion => "Awkward Potion",
            ItemType::PotionHealing => "Potion of Healing",
            ItemType::PotionStrength => "Potion of Strength",
            ItemType::PotionRegeneration => "Potion of Regeneration",
            ItemType::PotionFireResistance => "Potion of Fire Resistance",
            ItemType::GhastTear => "Ghast Tear",
            ItemType::MagmaCream => "Magma Cream",
            ItemType::FishingRod => "Fishing Rod",
            ItemType::RawFish => "Raw Fish",
            ItemType::CookedFish => "Cooked Fish",
            ItemType::RedstoneRepeater => "Redstone Repeater",
        }
    }

    fn material_tier(&self) -> u8 {
        match self {
            ItemType::WoodPickaxe
            | ItemType::WoodAxe
            | ItemType::WoodShovel
            | ItemType::WoodHoe => 1,
            ItemType::StonePickaxe
            | ItemType::StoneAxe
            | ItemType::StoneShovel
            | ItemType::StoneHoe => 2,
            ItemType::IronPickaxe
            | ItemType::IronAxe
            | ItemType::IronShovel
            | ItemType::IronHoe => 3,
            ItemType::DiamondPickaxe
            | ItemType::DiamondAxe
            | ItemType::DiamondShovel
            | ItemType::DiamondHoe => 4,
            _ => 0,
        }
    }

    fn is_pickaxe(&self) -> bool {
        matches!(
            self,
            ItemType::WoodPickaxe
                | ItemType::StonePickaxe
                | ItemType::IronPickaxe
                | ItemType::DiamondPickaxe
        )
    }

    fn is_axe(&self) -> bool {
        matches!(
            self,
            ItemType::WoodAxe | ItemType::StoneAxe | ItemType::IronAxe | ItemType::DiamondAxe
        )
    }

    fn is_shovel(&self) -> bool {
        matches!(
            self,
            ItemType::WoodShovel
                | ItemType::StoneShovel
                | ItemType::IronShovel
                | ItemType::DiamondShovel
        )
    }

    pub fn tool_level(&self) -> u8 {
        if self.is_pickaxe() {
            self.material_tier()
        } else {
            0
        }
    }

    pub fn efficiency(&self, block: BlockType) -> f32 {
        let tier = self.material_tier();
        if tier == 0 {
            return 1.0;
        }
        let effective = (self.is_pickaxe() && block.is_pickaxe_effective())
            || (self.is_axe() && block.is_axe_effective())
            || (self.is_shovel() && block.is_shovel_effective());
        if effective {
            2.0 + (tier as f32 * 2.0)
        } else {
            1.0
        }
    }

    pub fn max_durability(&self) -> Option<u32> {
        match self {
            ItemType::WoodPickaxe
            | ItemType::WoodSword
            | ItemType::WoodHoe
            | ItemType::WoodAxe
            | ItemType::WoodShovel => Some(60),
            ItemType::StonePickaxe
            | ItemType::StoneSword
            | ItemType::StoneHoe
            | ItemType::StoneAxe
            | ItemType::StoneShovel => Some(132),
            ItemType::IronPickaxe
            | ItemType::IronSword
            | ItemType::IronHoe
            | ItemType::IronAxe
            | ItemType::IronShovel => Some(251),
            ItemType::DiamondPickaxe
            | ItemType::DiamondSword
            | ItemType::DiamondHoe
            | ItemType::DiamondAxe
            | ItemType::DiamondShovel => Some(1562),
            ItemType::LeatherHelmet => Some(56),
            ItemType::LeatherChestplate => Some(81),
            ItemType::LeatherLeggings => Some(76),
            ItemType::LeatherBoots => Some(66),
            ItemType::IronHelmet => Some(166),
            ItemType::IronChestplate => Some(241),
            ItemType::IronLeggings => Some(226),
            ItemType::IronBoots => Some(196),
            ItemType::DiamondHelmet => Some(364),
            ItemType::DiamondChestplate => Some(529),
            ItemType::DiamondLeggings => Some(496),
            ItemType::DiamondBoots => Some(430),
            ItemType::Bow => Some(384),
            ItemType::FishingRod => Some(65),
            ItemType::FlintAndSteel => Some(65),
            ItemType::Shears => Some(239),
            _ => None,
        }
    }

    pub fn damage(&self) -> f32 {
        match self {
            ItemType::WoodSword => 4.0,
            ItemType::StoneSword => 5.0,
            ItemType::IronSword => 6.0,
            ItemType::DiamondSword => 7.0,
            ItemType::WoodAxe => 3.0,
            ItemType::StoneAxe => 4.0,
            ItemType::IronAxe => 5.0,
            ItemType::DiamondAxe => 6.0,
            ItemType::WoodPickaxe
            | ItemType::StonePickaxe
            | ItemType::IronPickaxe
            | ItemType::DiamondPickaxe
            | ItemType::WoodShovel
            | ItemType::StoneShovel
            | ItemType::IronShovel
            | ItemType::DiamondShovel => 2.0,
            ItemType::Bow => 1.0, // Bow melee damage
            _ => 1.0,
        }
    }

    pub fn armor_slot_index(&self) -> Option<usize> {
        match self {
            ItemType::LeatherHelmet | ItemType::IronHelmet | ItemType::DiamondHelmet => Some(0),
            ItemType::LeatherChestplate
            | ItemType::IronChestplate
            | ItemType::DiamondChestplate => Some(1),
            ItemType::LeatherLeggings | ItemType::IronLeggings | ItemType::DiamondLeggings => {
                Some(2)
            }
            ItemType::LeatherBoots | ItemType::IronBoots | ItemType::DiamondBoots => Some(3),
            _ => None,
        }
    }

    pub fn armor_points(&self) -> u8 {
        match self {
            ItemType::LeatherHelmet => 1,
            ItemType::LeatherChestplate => 3,
            ItemType::LeatherLeggings => 2,
            ItemType::LeatherBoots => 1,
            ItemType::IronHelmet => 2,
            ItemType::IronChestplate => 6,
            ItemType::IronLeggings => 5,
            ItemType::IronBoots => 2,
            ItemType::DiamondHelmet => 3,
            ItemType::DiamondChestplate => 8,
            ItemType::DiamondLeggings => 6,
            ItemType::DiamondBoots => 3,
            _ => 0,
        }
    }

    pub fn max_stack_size(&self) -> u32 {
        match self {
            ItemType::Bucket
            | ItemType::WaterBucket
            | ItemType::LavaBucket
            | ItemType::FlintAndSteel
            | ItemType::Shears
            | ItemType::Boat => 1,
            ItemType::WaterBottle
            | ItemType::AwkwardPotion
            | ItemType::PotionHealing
            | ItemType::PotionStrength
            | ItemType::PotionRegeneration
            | ItemType::PotionFireResistance => 1,
            _ => 64,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ItemStack {
    pub item_type: ItemType,
    pub count: u32,
    pub durability: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Inventory {
    pub slots: Vec<Option<ItemStack>>,
    pub capacity: usize,
}

impl Inventory {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
            capacity,
        }
    }

    pub fn has_item(&self, item: ItemType, amount: u32) -> bool {
        let mut count = 0;
        for slot in self.slots.iter().flatten() {
            if slot.item_type == item {
                count += slot.count;
            }
        }
        count >= amount
    }

    pub fn remove_item(&mut self, item: ItemType, amount: u32) -> bool {
        if !self.has_item(item, amount) {
            return false;
        }
        let mut remaining = amount;
        for slot in self.slots.iter_mut() {
            if let Some(s) = slot
                && s.item_type == item
            {
                if s.count > remaining {
                    s.count -= remaining;
                    remaining = 0;
                    break;
                } else {
                    remaining -= s.count;
                    *slot = None;
                    if remaining == 0 {
                        break;
                    }
                }
            }
        }
        remaining == 0
    }

    pub fn add_item(&mut self, item: ItemType, amount: u32) -> u32 {
        let mut remaining = amount;
        let stack_limit = if item.max_durability().is_some() {
            1
        } else {
            item.max_stack_size()
        };
        if stack_limit > 1 {
            for slot in self.slots.iter_mut().flatten() {
                if slot.item_type == item {
                    let add = remaining.min(stack_limit.saturating_sub(slot.count));
                    slot.count += add;
                    remaining -= add;
                    if remaining == 0 {
                        return 0;
                    }
                }
            }
        }
        for i in 0..self.capacity {
            if self.slots[i].is_none() {
                let add = remaining.min(stack_limit);
                self.slots[i] = Some(ItemStack {
                    item_type: item,
                    count: add,
                    durability: item.max_durability(),
                });
                remaining -= add;
                if remaining == 0 {
                    break;
                }
            }
        }
        remaining
    }
}

#[derive(Clone, Debug)]
pub struct RecipeShape {
    pub width: usize,
    pub height: usize,
    pub slots: Vec<Option<ItemType>>,
}

impl RecipeShape {
    pub fn new(rows: &[&[Option<ItemType>]]) -> Self {
        let height = rows.len();
        assert!(height > 0, "recipe shape cannot be empty");
        let width = rows[0].len();
        assert!(width > 0, "recipe shape cannot have zero width");
        assert!(
            rows.iter().all(|row| row.len() == width),
            "recipe rows must have equal width"
        );
        let mut slots = Vec::with_capacity(width * height);
        for row in rows {
            slots.extend_from_slice(row);
        }
        Self {
            width,
            height,
            slots,
        }
    }

    pub fn ingredient_counts(&self) -> Vec<(ItemType, u32)> {
        let mut counts: BTreeMap<ItemType, u32> = BTreeMap::new();
        for item in self.slots.iter().flatten() {
            *counts.entry(*item).or_insert(0) += 1;
        }
        counts.into_iter().collect()
    }
}

pub struct Recipe {
    pub result: ItemType,
    pub result_count: u32,
    pub ingredients: Vec<(ItemType, u32)>,
    pub needs_crafting_table: bool,
    pub needs_furnace: bool,
    pub shape: Option<RecipeShape>,
}

impl Recipe {
    fn apply_default_shape(&mut self) {
        if self.needs_furnace || self.shape.is_some() {
            return;
        }
        let s = Some;
        self.shape = match self.result {
            ItemType::Stick if self.ingredients == vec![(ItemType::Planks, 2)] => {
                Some(RecipeShape::new(&[
                    &[s(ItemType::Planks)],
                    &[s(ItemType::Planks)],
                ]))
            }
            ItemType::CraftingTable => Some(RecipeShape::new(&[
                &[s(ItemType::Planks), s(ItemType::Planks)],
                &[s(ItemType::Planks), s(ItemType::Planks)],
            ])),
            ItemType::Furnace => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                ],
                &[s(ItemType::Cobblestone), None, s(ItemType::Cobblestone)],
                &[
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                ],
            ])),
            ItemType::Torch => Some(RecipeShape::new(&[
                &[s(ItemType::Coal)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::RedstoneTorch => Some(RecipeShape::new(&[
                &[s(ItemType::RedstoneDust)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::Lever => Some(RecipeShape::new(&[
                &[s(ItemType::Stick)],
                &[s(ItemType::Cobblestone)],
            ])),
            ItemType::StoneButton => Some(RecipeShape::new(&[&[s(ItemType::Stone)]])),
            ItemType::Bucket => Some(RecipeShape::new(&[
                &[s(ItemType::IronIngot), None, s(ItemType::IronIngot)],
                &[None, s(ItemType::IronIngot), None],
            ])),
            ItemType::Tnt => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Gunpowder),
                    s(ItemType::Sand),
                    s(ItemType::Gunpowder),
                ],
                &[s(ItemType::Sand), s(ItemType::Gunpowder), s(ItemType::Sand)],
                &[
                    s(ItemType::Gunpowder),
                    s(ItemType::Sand),
                    s(ItemType::Gunpowder),
                ],
            ])),
            ItemType::Piston => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                ],
                &[
                    s(ItemType::Cobblestone),
                    s(ItemType::IronIngot),
                    s(ItemType::Cobblestone),
                ],
                &[
                    s(ItemType::Cobblestone),
                    s(ItemType::RedstoneDust),
                    s(ItemType::Cobblestone),
                ],
            ])),
            ItemType::RedstoneRepeater => Some(RecipeShape::new(&[
                &[
                    s(ItemType::RedstoneTorch),
                    s(ItemType::RedstoneDust),
                    s(ItemType::RedstoneTorch),
                ],
                &[s(ItemType::Stone), s(ItemType::Stone), s(ItemType::Stone)],
            ])),
            ItemType::WoodPickaxe => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                ],
                &[None, s(ItemType::Stick), None],
                &[None, s(ItemType::Stick), None],
            ])),
            ItemType::StonePickaxe => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                ],
                &[None, s(ItemType::Stick), None],
                &[None, s(ItemType::Stick), None],
            ])),
            ItemType::IronPickaxe => Some(RecipeShape::new(&[
                &[
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                ],
                &[None, s(ItemType::Stick), None],
                &[None, s(ItemType::Stick), None],
            ])),
            ItemType::DiamondPickaxe => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                ],
                &[None, s(ItemType::Stick), None],
                &[None, s(ItemType::Stick), None],
            ])),
            ItemType::WoodAxe => Some(RecipeShape::new(&[
                &[s(ItemType::Planks), s(ItemType::Planks)],
                &[s(ItemType::Planks), s(ItemType::Stick)],
                &[None, s(ItemType::Stick)],
            ])),
            ItemType::StoneAxe => Some(RecipeShape::new(&[
                &[s(ItemType::Cobblestone), s(ItemType::Cobblestone)],
                &[s(ItemType::Cobblestone), s(ItemType::Stick)],
                &[None, s(ItemType::Stick)],
            ])),
            ItemType::IronAxe => Some(RecipeShape::new(&[
                &[s(ItemType::IronIngot), s(ItemType::IronIngot)],
                &[s(ItemType::IronIngot), s(ItemType::Stick)],
                &[None, s(ItemType::Stick)],
            ])),
            ItemType::DiamondAxe => Some(RecipeShape::new(&[
                &[s(ItemType::Diamond), s(ItemType::Diamond)],
                &[s(ItemType::Diamond), s(ItemType::Stick)],
                &[None, s(ItemType::Stick)],
            ])),
            ItemType::WoodShovel => Some(RecipeShape::new(&[
                &[s(ItemType::Planks)],
                &[s(ItemType::Stick)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::StoneShovel => Some(RecipeShape::new(&[
                &[s(ItemType::Cobblestone)],
                &[s(ItemType::Stick)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::IronShovel => Some(RecipeShape::new(&[
                &[s(ItemType::IronIngot)],
                &[s(ItemType::Stick)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::DiamondShovel => Some(RecipeShape::new(&[
                &[s(ItemType::Diamond)],
                &[s(ItemType::Stick)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::WoodSword => Some(RecipeShape::new(&[
                &[s(ItemType::Planks)],
                &[s(ItemType::Planks)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::StoneSword => Some(RecipeShape::new(&[
                &[s(ItemType::Cobblestone)],
                &[s(ItemType::Cobblestone)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::IronSword => Some(RecipeShape::new(&[
                &[s(ItemType::IronIngot)],
                &[s(ItemType::IronIngot)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::DiamondSword => Some(RecipeShape::new(&[
                &[s(ItemType::Diamond)],
                &[s(ItemType::Diamond)],
                &[s(ItemType::Stick)],
            ])),
            ItemType::WoodHoe => Some(RecipeShape::new(&[
                &[s(ItemType::Planks), s(ItemType::Planks)],
                &[None, s(ItemType::Stick)],
                &[None, s(ItemType::Stick)],
            ])),
            ItemType::StoneHoe => Some(RecipeShape::new(&[
                &[s(ItemType::Cobblestone), s(ItemType::Cobblestone)],
                &[None, s(ItemType::Stick)],
                &[None, s(ItemType::Stick)],
            ])),
            ItemType::IronHoe => Some(RecipeShape::new(&[
                &[s(ItemType::IronIngot), s(ItemType::IronIngot)],
                &[None, s(ItemType::Stick)],
                &[None, s(ItemType::Stick)],
            ])),
            ItemType::DiamondHoe => Some(RecipeShape::new(&[
                &[s(ItemType::Diamond), s(ItemType::Diamond)],
                &[None, s(ItemType::Stick)],
                &[None, s(ItemType::Stick)],
            ])),
            ItemType::Bread => Some(RecipeShape::new(&[&[
                s(ItemType::Wheat),
                s(ItemType::Wheat),
                s(ItemType::Wheat),
            ]])),
            ItemType::Bed => Some(RecipeShape::new(&[
                &[s(ItemType::Wool), s(ItemType::Wool), s(ItemType::Wool)],
                &[
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                ],
            ])),
            ItemType::Chest => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                ],
                &[s(ItemType::Planks), None, s(ItemType::Planks)],
                &[
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                    s(ItemType::Planks),
                ],
            ])),
            ItemType::EnchantingTable => Some(RecipeShape::new(&[
                &[None, s(ItemType::BlazePowder), None],
                &[
                    s(ItemType::Diamond),
                    s(ItemType::Obsidian),
                    s(ItemType::Diamond),
                ],
                &[
                    s(ItemType::Obsidian),
                    s(ItemType::Obsidian),
                    s(ItemType::Obsidian),
                ],
            ])),
            ItemType::Anvil => Some(RecipeShape::new(&[
                &[
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                ],
                &[None, s(ItemType::IronIngot), None],
                &[
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                ],
            ])),
            ItemType::BrewingStand => Some(RecipeShape::new(&[
                &[None, s(ItemType::BlazeRod), None],
                &[
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                ],
            ])),
            ItemType::GlassBottle => Some(RecipeShape::new(&[
                &[s(ItemType::Glass), None, s(ItemType::Glass)],
                &[None, s(ItemType::Glass), None],
            ])),
            ItemType::WoodDoor => Some(RecipeShape::new(&[
                &[s(ItemType::Planks), s(ItemType::Planks)],
                &[s(ItemType::Planks), s(ItemType::Planks)],
                &[s(ItemType::Planks), s(ItemType::Planks)],
            ])),
            ItemType::Ladder => Some(RecipeShape::new(&[
                &[s(ItemType::Stick), None, s(ItemType::Stick)],
                &[s(ItemType::Stick), s(ItemType::Stick), s(ItemType::Stick)],
                &[s(ItemType::Stick), None, s(ItemType::Stick)],
            ])),
            ItemType::StoneSlab => Some(RecipeShape::new(&[&[
                s(ItemType::Cobblestone),
                s(ItemType::Cobblestone),
                s(ItemType::Cobblestone),
            ]])),
            ItemType::StoneStairs => Some(RecipeShape::new(&[
                &[s(ItemType::Cobblestone), None, None],
                &[s(ItemType::Cobblestone), s(ItemType::Cobblestone), None],
                &[
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                    s(ItemType::Cobblestone),
                ],
            ])),
            ItemType::LeatherHelmet => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Leather),
                    s(ItemType::Leather),
                    s(ItemType::Leather),
                ],
                &[s(ItemType::Leather), None, s(ItemType::Leather)],
            ])),
            ItemType::LeatherChestplate => Some(RecipeShape::new(&[
                &[s(ItemType::Leather), None, s(ItemType::Leather)],
                &[
                    s(ItemType::Leather),
                    s(ItemType::Leather),
                    s(ItemType::Leather),
                ],
                &[
                    s(ItemType::Leather),
                    s(ItemType::Leather),
                    s(ItemType::Leather),
                ],
            ])),
            ItemType::LeatherLeggings => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Leather),
                    s(ItemType::Leather),
                    s(ItemType::Leather),
                ],
                &[s(ItemType::Leather), None, s(ItemType::Leather)],
                &[s(ItemType::Leather), None, s(ItemType::Leather)],
            ])),
            ItemType::LeatherBoots => Some(RecipeShape::new(&[
                &[s(ItemType::Leather), None, s(ItemType::Leather)],
                &[s(ItemType::Leather), None, s(ItemType::Leather)],
            ])),
            ItemType::IronHelmet => Some(RecipeShape::new(&[
                &[
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                ],
                &[s(ItemType::IronIngot), None, s(ItemType::IronIngot)],
            ])),
            ItemType::IronChestplate => Some(RecipeShape::new(&[
                &[s(ItemType::IronIngot), None, s(ItemType::IronIngot)],
                &[
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                ],
                &[
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                ],
            ])),
            ItemType::IronLeggings => Some(RecipeShape::new(&[
                &[
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                    s(ItemType::IronIngot),
                ],
                &[s(ItemType::IronIngot), None, s(ItemType::IronIngot)],
                &[s(ItemType::IronIngot), None, s(ItemType::IronIngot)],
            ])),
            ItemType::IronBoots => Some(RecipeShape::new(&[
                &[s(ItemType::IronIngot), None, s(ItemType::IronIngot)],
                &[s(ItemType::IronIngot), None, s(ItemType::IronIngot)],
            ])),
            ItemType::DiamondHelmet => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                ],
                &[s(ItemType::Diamond), None, s(ItemType::Diamond)],
            ])),
            ItemType::DiamondChestplate => Some(RecipeShape::new(&[
                &[s(ItemType::Diamond), None, s(ItemType::Diamond)],
                &[
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                ],
                &[
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                ],
            ])),
            ItemType::DiamondLeggings => Some(RecipeShape::new(&[
                &[
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                    s(ItemType::Diamond),
                ],
                &[s(ItemType::Diamond), None, s(ItemType::Diamond)],
                &[s(ItemType::Diamond), None, s(ItemType::Diamond)],
            ])),
            ItemType::DiamondBoots => Some(RecipeShape::new(&[
                &[s(ItemType::Diamond), None, s(ItemType::Diamond)],
                &[s(ItemType::Diamond), None, s(ItemType::Diamond)],
            ])),
            ItemType::Bow => Some(RecipeShape::new(&[
                &[None, s(ItemType::Stick), s(ItemType::String)],
                &[s(ItemType::Stick), None, s(ItemType::String)],
                &[None, s(ItemType::Stick), s(ItemType::String)],
            ])),
            ItemType::FishingRod => Some(RecipeShape::new(&[
                &[None, None, s(ItemType::Stick)],
                &[None, s(ItemType::Stick), s(ItemType::String)],
                &[s(ItemType::Stick), None, s(ItemType::String)],
            ])),
            _ => None,
        };
    }

    fn required_ingredients(&self) -> Vec<(ItemType, u32)> {
        if let Some(shape) = &self.shape {
            return shape.ingredient_counts();
        }
        self.ingredients.clone()
    }

    pub fn ingredient_requirements(&self) -> Vec<(ItemType, u32)> {
        self.required_ingredients()
    }

    pub fn ingredient_rows(&self) -> Option<Vec<Vec<Option<ItemType>>>> {
        let shape = self.shape.as_ref()?;
        let mut rows = Vec::with_capacity(shape.height);
        for y in 0..shape.height {
            let start = y * shape.width;
            let end = start + shape.width;
            rows.push(shape.slots[start..end].to_vec());
        }
        Some(rows)
    }

    pub fn all() -> Vec<Recipe> {
        let mut recipes = vec![
            Recipe {
                result: ItemType::Planks,
                result_count: 4,
                ingredients: vec![(ItemType::Wood, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Planks,
                result_count: 4,
                ingredients: vec![(ItemType::BirchWood, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Stick,
                result_count: 4,
                ingredients: vec![(ItemType::Planks, 2)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::CraftingTable,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 4)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Furnace,
                result_count: 1,
                ingredients: vec![(ItemType::Cobblestone, 8)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Torch,
                result_count: 4,
                ingredients: vec![(ItemType::Coal, 1), (ItemType::Stick, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::RedstoneTorch,
                result_count: 1,
                ingredients: vec![(ItemType::RedstoneDust, 1), (ItemType::Stick, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Lever,
                result_count: 1,
                ingredients: vec![(ItemType::Cobblestone, 1), (ItemType::Stick, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::StoneButton,
                result_count: 1,
                ingredients: vec![(ItemType::Stone, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Bucket,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 3)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Tnt,
                result_count: 1,
                ingredients: vec![(ItemType::Gunpowder, 5), (ItemType::Sand, 4)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Piston,
                result_count: 1,
                ingredients: vec![
                    (ItemType::Planks, 3),
                    (ItemType::Cobblestone, 4),
                    (ItemType::IronIngot, 1),
                    (ItemType::RedstoneDust, 1),
                ],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::RedstoneRepeater,
                result_count: 1,
                ingredients: vec![
                    (ItemType::Stone, 3),
                    (ItemType::RedstoneTorch, 2),
                    (ItemType::RedstoneDust, 1),
                ],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::WoodPickaxe,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 3), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::StonePickaxe,
                result_count: 1,
                ingredients: vec![(ItemType::Cobblestone, 3), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronPickaxe,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 3), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondPickaxe,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 3), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::WoodAxe,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 3), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::StoneAxe,
                result_count: 1,
                ingredients: vec![(ItemType::Cobblestone, 3), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronAxe,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 3), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondAxe,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 3), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::WoodShovel,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 1), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::StoneShovel,
                result_count: 1,
                ingredients: vec![(ItemType::Cobblestone, 1), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronShovel,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 1), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondShovel,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 1), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::WoodSword,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 2), (ItemType::Stick, 1)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::StoneSword,
                result_count: 1,
                ingredients: vec![(ItemType::Cobblestone, 2), (ItemType::Stick, 1)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronSword,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 2), (ItemType::Stick, 1)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondSword,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 2), (ItemType::Stick, 1)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronIngot,
                result_count: 1,
                ingredients: vec![(ItemType::RawIron, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::Stone,
                result_count: 1,
                ingredients: vec![(ItemType::Cobblestone, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::GoldIngot,
                result_count: 1,
                ingredients: vec![(ItemType::RawGold, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::Glass,
                result_count: 1,
                ingredients: vec![(ItemType::Sand, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::WoodHoe,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 2), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::StoneHoe,
                result_count: 1,
                ingredients: vec![(ItemType::Cobblestone, 2), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronHoe,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 2), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondHoe,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 2), (ItemType::Stick, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::LeatherHelmet,
                result_count: 1,
                ingredients: vec![(ItemType::Leather, 5)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::LeatherChestplate,
                result_count: 1,
                ingredients: vec![(ItemType::Leather, 8)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::LeatherLeggings,
                result_count: 1,
                ingredients: vec![(ItemType::Leather, 7)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::LeatherBoots,
                result_count: 1,
                ingredients: vec![(ItemType::Leather, 4)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronHelmet,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 5)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronChestplate,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 8)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronLeggings,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 7)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::IronBoots,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 4)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondHelmet,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 5)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondChestplate,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 8)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondLeggings,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 7)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::DiamondBoots,
                result_count: 1,
                ingredients: vec![(ItemType::Diamond, 4)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Bread,
                result_count: 1,
                ingredients: vec![(ItemType::Wheat, 3)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Bed,
                result_count: 1,
                ingredients: vec![(ItemType::Wool, 3), (ItemType::Planks, 3)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Chest,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 8)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::WoodDoor,
                result_count: 3,
                ingredients: vec![(ItemType::Planks, 6)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Ladder,
                result_count: 3,
                ingredients: vec![(ItemType::Stick, 7)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::StoneSlab,
                result_count: 6,
                ingredients: vec![(ItemType::Cobblestone, 3)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::StoneStairs,
                result_count: 4,
                ingredients: vec![(ItemType::Cobblestone, 6)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::EnchantingTable,
                result_count: 1,
                ingredients: vec![
                    (ItemType::BlazePowder, 1),
                    (ItemType::Diamond, 2),
                    (ItemType::Obsidian, 4),
                ],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Anvil,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 7)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::BrewingStand,
                result_count: 1,
                ingredients: vec![(ItemType::BlazeRod, 1), (ItemType::Cobblestone, 3)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::GlassBottle,
                result_count: 3,
                ingredients: vec![(ItemType::Glass, 3)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::MagmaCream,
                result_count: 1,
                ingredients: vec![(ItemType::Slimeball, 1), (ItemType::BlazePowder, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::CookedBeef,
                result_count: 1,
                ingredients: vec![(ItemType::RawBeef, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::CookedMutton,
                result_count: 1,
                ingredients: vec![(ItemType::RawMutton, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::CookedPorkchop,
                result_count: 1,
                ingredients: vec![(ItemType::RawPorkchop, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::CookedChicken,
                result_count: 1,
                ingredients: vec![(ItemType::RawChicken, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::CookedFish,
                result_count: 1,
                ingredients: vec![(ItemType::RawFish, 1)],
                needs_crafting_table: false,
                needs_furnace: true,
                shape: None,
            },
            Recipe {
                result: ItemType::Bow,
                result_count: 1,
                ingredients: vec![(ItemType::Stick, 3), (ItemType::String, 3)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::FishingRod,
                result_count: 1,
                ingredients: vec![(ItemType::Stick, 3), (ItemType::String, 2)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Arrow,
                result_count: 4,
                ingredients: vec![
                    (ItemType::Flint, 1),
                    (ItemType::Stick, 1),
                    (ItemType::Feather, 1),
                ],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: Some(RecipeShape::new(&[
                    &[Some(ItemType::Flint)],
                    &[Some(ItemType::Stick)],
                    &[Some(ItemType::Feather)],
                ])),
            },
            Recipe {
                result: ItemType::FlintAndSteel,
                result_count: 1,
                ingredients: vec![(ItemType::Flint, 1), (ItemType::IronIngot, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: Some(RecipeShape::new(&[
                    &[Some(ItemType::IronIngot), None],
                    &[None, Some(ItemType::Flint)],
                ])),
            },
            Recipe {
                result: ItemType::Shears,
                result_count: 1,
                ingredients: vec![(ItemType::IronIngot, 2)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: Some(RecipeShape::new(&[
                    &[Some(ItemType::IronIngot), None],
                    &[None, Some(ItemType::IronIngot)],
                ])),
            },
            Recipe {
                result: ItemType::BlazePowder,
                result_count: 2,
                ingredients: vec![(ItemType::BlazeRod, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::BoneMeal,
                result_count: 3,
                ingredients: vec![(ItemType::Bone, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Paper,
                result_count: 3,
                ingredients: vec![(ItemType::SugarCane, 3)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: Some(RecipeShape::new(&[&[
                    Some(ItemType::SugarCane),
                    Some(ItemType::SugarCane),
                    Some(ItemType::SugarCane),
                ]])),
            },
            Recipe {
                result: ItemType::Book,
                result_count: 1,
                ingredients: vec![(ItemType::Paper, 3), (ItemType::Leather, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
            Recipe {
                result: ItemType::Bookshelf,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 6), (ItemType::Book, 3)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: Some(RecipeShape::new(&[
                    &[
                        Some(ItemType::Planks),
                        Some(ItemType::Planks),
                        Some(ItemType::Planks),
                    ],
                    &[
                        Some(ItemType::Book),
                        Some(ItemType::Book),
                        Some(ItemType::Book),
                    ],
                    &[
                        Some(ItemType::Planks),
                        Some(ItemType::Planks),
                        Some(ItemType::Planks),
                    ],
                ])),
            },
            Recipe {
                result: ItemType::Boat,
                result_count: 1,
                ingredients: vec![(ItemType::Planks, 5)],
                needs_crafting_table: true,
                needs_furnace: false,
                shape: Some(RecipeShape::new(&[
                    &[Some(ItemType::Planks), None, Some(ItemType::Planks)],
                    &[
                        Some(ItemType::Planks),
                        Some(ItemType::Planks),
                        Some(ItemType::Planks),
                    ],
                ])),
            },
            Recipe {
                result: ItemType::EyeOfEnder,
                result_count: 1,
                ingredients: vec![(ItemType::EnderPearl, 1), (ItemType::BlazePowder, 1)],
                needs_crafting_table: false,
                needs_furnace: false,
                shape: None,
            },
        ];
        for recipe in &mut recipes {
            recipe.apply_default_shape();
        }
        recipes
    }

    pub fn can_craft(
        &self,
        inventory: &Inventory,
        at_crafting_table: bool,
        at_furnace: bool,
    ) -> bool {
        if self.needs_crafting_table && !at_crafting_table {
            return false;
        }
        if self.needs_furnace && !at_furnace {
            return false;
        }
        if !at_furnace && let Some(shape) = &self.shape {
            let grid_size = if at_crafting_table { 3 } else { 2 };
            if shape.width > grid_size || shape.height > grid_size {
                return false;
            }
        }
        for (item, amount) in self.required_ingredients() {
            if !inventory.has_item(item, amount) {
                return false;
            }
        }
        true
    }

    pub fn craft(&self, inventory: &mut Inventory) {
        for (item, amount) in self.required_ingredients() {
            inventory.remove_item(item, amount);
        }
        inventory.add_item(self.result, self.result_count);
    }
}

#[cfg(test)]
mod tests {
    use crate::world::block::BlockType;

    use super::{Inventory, ItemType, Recipe};

    #[test]
    fn test_blaze_chain_recipes_exist() {
        let recipes = Recipe::all();
        assert!(recipes.iter().any(|r| {
            r.result == ItemType::BlazePowder && r.ingredients == vec![(ItemType::BlazeRod, 1)]
        }));
        assert!(recipes.iter().any(|r| {
            r.result == ItemType::EyeOfEnder
                && r.ingredients == vec![(ItemType::EnderPearl, 1), (ItemType::BlazePowder, 1)]
        }));
    }

    #[test]
    fn test_bone_meal_recipe_exists_and_yields_three() {
        let recipe = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::BoneMeal)
            .expect("bone meal recipe should exist");
        assert_eq!(recipe.ingredients, vec![(ItemType::Bone, 1)]);
        assert_eq!(recipe.result_count, 3);
        assert!(!recipe.needs_crafting_table);
    }

    #[test]
    fn test_crafting_eye_of_ender_consumes_pearl_and_powder() {
        let mut inv = Inventory::new(8);
        inv.add_item(ItemType::EnderPearl, 1);
        inv.add_item(ItemType::BlazePowder, 1);
        let recipe = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::EyeOfEnder)
            .expect("eye recipe should exist");

        assert!(recipe.can_craft(&inv, false, false));
        recipe.craft(&mut inv);
        assert!(inv.has_item(ItemType::EyeOfEnder, 1));
        assert!(!inv.has_item(ItemType::EnderPearl, 1));
        assert!(!inv.has_item(ItemType::BlazePowder, 1));
    }

    #[test]
    fn test_tool_family_recipes_exist() {
        let recipes = Recipe::all();
        assert!(recipes.iter().any(|r| r.result == ItemType::WoodAxe));
        assert!(recipes.iter().any(|r| r.result == ItemType::StoneAxe));
        assert!(recipes.iter().any(|r| r.result == ItemType::IronAxe));
        assert!(recipes.iter().any(|r| r.result == ItemType::DiamondAxe));
        assert!(recipes.iter().any(|r| r.result == ItemType::WoodShovel));
        assert!(recipes.iter().any(|r| r.result == ItemType::StoneShovel));
        assert!(recipes.iter().any(|r| r.result == ItemType::IronShovel));
        assert!(recipes.iter().any(|r| r.result == ItemType::DiamondShovel));
    }

    #[test]
    fn test_fishing_rod_recipe_is_shaped_and_requires_table() {
        let rod = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::FishingRod)
            .expect("fishing rod recipe should exist");
        assert!(rod.needs_crafting_table);
        let rows = rod.ingredient_rows().expect("fishing rod should be shaped");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0][2], Some(ItemType::Stick));
        assert_eq!(rows[1][2], Some(ItemType::String));
        assert_eq!(rows[2][0], Some(ItemType::Stick));
    }

    #[test]
    fn test_arrow_recipe_is_shaped_and_requires_table() {
        let arrow = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::Arrow)
            .expect("arrow recipe should exist");
        assert!(arrow.needs_crafting_table);
        assert_eq!(arrow.result_count, 4);
        let rows = arrow
            .ingredient_rows()
            .expect("arrow recipe should be shaped");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0][0], Some(ItemType::Flint));
        assert_eq!(rows[1][0], Some(ItemType::Stick));
        assert_eq!(rows[2][0], Some(ItemType::Feather));
    }

    #[test]
    fn test_flint_and_steel_recipe_is_shaped_and_non_stackable() {
        let flint_and_steel = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::FlintAndSteel)
            .expect("flint and steel recipe should exist");
        assert!(!flint_and_steel.needs_crafting_table);
        assert_eq!(ItemType::FlintAndSteel.max_stack_size(), 1);
        assert_eq!(ItemType::FlintAndSteel.max_durability(), Some(65));
        let rows = flint_and_steel
            .ingredient_rows()
            .expect("flint and steel recipe should be shaped");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Some(ItemType::IronIngot));
        assert_eq!(rows[1][1], Some(ItemType::Flint));
    }

    #[test]
    fn test_shears_recipe_is_shaped_and_non_stackable() {
        let shears = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::Shears)
            .expect("shears recipe should exist");
        assert!(!shears.needs_crafting_table);
        assert_eq!(ItemType::Shears.max_stack_size(), 1);
        assert_eq!(ItemType::Shears.max_durability(), Some(239));
        let rows = shears
            .ingredient_rows()
            .expect("shears recipe should be shaped");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Some(ItemType::IronIngot));
        assert_eq!(rows[1][1], Some(ItemType::IronIngot));
    }

    #[test]
    fn test_fishing_items_have_expected_properties() {
        assert_eq!(ItemType::FishingRod.max_durability(), Some(65));
        assert_eq!(ItemType::FishingRod.max_stack_size(), 64);
        assert_eq!(ItemType::RawFish.max_durability(), None);
        assert_eq!(ItemType::CookedFish.max_durability(), None);
    }

    #[test]
    fn test_bucket_items_have_expected_stack_and_recipe_behavior() {
        assert_eq!(ItemType::Bucket.max_stack_size(), 1);
        assert_eq!(ItemType::WaterBucket.max_stack_size(), 1);
        assert_eq!(ItemType::LavaBucket.max_stack_size(), 1);

        let bucket = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::Bucket)
            .expect("bucket recipe should exist");
        assert!(!bucket.needs_crafting_table);
        let rows = bucket
            .ingredient_rows()
            .expect("bucket recipe should be shaped");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Some(ItemType::IronIngot));
        assert_eq!(rows[0][1], None);
        assert_eq!(rows[0][2], Some(ItemType::IronIngot));
        assert_eq!(rows[1][1], Some(ItemType::IronIngot));
    }

    #[test]
    fn test_paper_book_and_bookshelf_chain_exists() {
        let recipes = Recipe::all();
        let paper = recipes
            .iter()
            .find(|recipe| recipe.result == ItemType::Paper)
            .expect("paper recipe should exist");
        assert!(paper.needs_crafting_table);
        assert_eq!(paper.result_count, 3);
        let paper_rows = paper
            .ingredient_rows()
            .expect("paper recipe should be shaped");
        assert_eq!(paper_rows.len(), 1);
        assert_eq!(paper_rows[0][0], Some(ItemType::SugarCane));
        assert_eq!(paper_rows[0][1], Some(ItemType::SugarCane));
        assert_eq!(paper_rows[0][2], Some(ItemType::SugarCane));

        let book = recipes
            .iter()
            .find(|recipe| recipe.result == ItemType::Book)
            .expect("book recipe should exist");
        assert!(!book.needs_crafting_table);
        assert_eq!(
            book.ingredients,
            vec![(ItemType::Paper, 3), (ItemType::Leather, 1)]
        );

        let bookshelf = recipes
            .iter()
            .find(|recipe| recipe.result == ItemType::Bookshelf)
            .expect("bookshelf recipe should exist");
        assert!(bookshelf.needs_crafting_table);
        let rows = bookshelf
            .ingredient_rows()
            .expect("bookshelf recipe should be shaped");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0][0], Some(ItemType::Planks));
        assert_eq!(rows[1][1], Some(ItemType::Book));
        assert_eq!(rows[2][2], Some(ItemType::Planks));
    }

    #[test]
    fn test_boat_recipe_is_shaped_requires_table_and_is_non_stackable() {
        let boat = Recipe::all()
            .into_iter()
            .find(|recipe| recipe.result == ItemType::Boat)
            .expect("boat recipe should exist");
        assert!(boat.needs_crafting_table);
        assert_eq!(boat.result_count, 1);
        assert_eq!(ItemType::Boat.max_stack_size(), 1);
        let rows = boat
            .ingredient_rows()
            .expect("boat recipe should be shaped");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0][0], Some(ItemType::Planks));
        assert_eq!(rows[0][1], None);
        assert_eq!(rows[0][2], Some(ItemType::Planks));
        assert_eq!(rows[1][0], Some(ItemType::Planks));
        assert_eq!(rows[1][1], Some(ItemType::Planks));
        assert_eq!(rows[1][2], Some(ItemType::Planks));
    }

    #[test]
    fn test_raw_fish_has_furnace_recipe() {
        let recipes = Recipe::all();
        assert!(recipes.iter().any(|r| {
            r.result == ItemType::CookedFish
                && r.ingredients == vec![(ItemType::RawFish, 1)]
                && r.needs_furnace
        }));
    }

    #[test]
    fn test_tool_efficiency_matches_block_family() {
        assert!(ItemType::WoodAxe.efficiency(BlockType::Wood) > 1.0);
        assert_eq!(ItemType::WoodAxe.efficiency(BlockType::Stone), 1.0);
        assert!(ItemType::WoodShovel.efficiency(BlockType::Dirt) > 1.0);
        assert_eq!(ItemType::WoodShovel.efficiency(BlockType::Wood), 1.0);
        assert!(ItemType::WoodPickaxe.efficiency(BlockType::Stone) > 1.0);
        assert_eq!(ItemType::WoodPickaxe.efficiency(BlockType::Dirt), 1.0);
    }

    #[test]
    fn test_only_pickaxes_grant_harvest_level() {
        assert_eq!(ItemType::WoodPickaxe.tool_level(), 1);
        assert_eq!(ItemType::StonePickaxe.tool_level(), 2);
        assert_eq!(ItemType::WoodHoe.tool_level(), 0);
        assert_eq!(ItemType::WoodAxe.tool_level(), 0);
        assert_eq!(ItemType::WoodShovel.tool_level(), 0);
    }

    #[test]
    fn test_building_family_recipes_exist() {
        let recipes = Recipe::all();
        assert!(recipes.iter().any(|r| r.result == ItemType::Glass));
        assert!(recipes.iter().any(|r| r.result == ItemType::WoodDoor));
        assert!(recipes.iter().any(|r| r.result == ItemType::Ladder));
        assert!(recipes.iter().any(|r| r.result == ItemType::StoneSlab));
        assert!(recipes.iter().any(|r| r.result == ItemType::StoneStairs));
        assert!(
            recipes
                .iter()
                .any(|r| r.result == ItemType::EnchantingTable)
        );
        assert!(recipes.iter().any(|r| r.result == ItemType::Anvil));
        assert!(recipes.iter().any(|r| r.result == ItemType::BrewingStand));
        assert!(recipes.iter().any(|r| r.result == ItemType::GlassBottle));
        assert!(recipes.iter().any(|r| r.result == ItemType::MagmaCream));
        assert!(
            recipes
                .iter()
                .any(|r| r.result == ItemType::RedstoneRepeater)
        );
    }

    #[test]
    fn test_building_items_map_to_blocks() {
        assert_eq!(ItemType::Glass.to_block(), Some(BlockType::Glass));
        assert_eq!(ItemType::Wool.to_block(), Some(BlockType::Wool));
        assert_eq!(
            ItemType::WoodDoor.to_block(),
            Some(BlockType::WoodDoor(false))
        );
        assert_eq!(ItemType::Ladder.to_block(), Some(BlockType::Ladder));
        assert_eq!(ItemType::StoneSlab.to_block(), Some(BlockType::StoneSlab));
        assert_eq!(
            ItemType::StoneStairs.to_block(),
            Some(BlockType::StoneStairs)
        );
        assert_eq!(
            ItemType::EnchantingTable.to_block(),
            Some(BlockType::EnchantingTable)
        );
        assert_eq!(ItemType::Anvil.to_block(), Some(BlockType::Anvil));
        assert_eq!(
            ItemType::BrewingStand.to_block(),
            Some(BlockType::BrewingStand)
        );
        assert_eq!(
            ItemType::RedstoneRepeater.to_block(),
            Some(BlockType::RedstoneRepeater {
                powered: false,
                delay: 1,
                facing_right: true,
                timer: 0,
                target_powered: false
            })
        );
    }

    #[test]
    fn test_repeater_recipe_is_shaped_and_requires_table() {
        let repeater = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::RedstoneRepeater)
            .expect("repeater recipe should exist");
        assert!(repeater.needs_crafting_table);
        let rows = repeater
            .ingredient_rows()
            .expect("repeater recipe should be shaped");
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].len(), 3);
        assert_eq!(rows[0][0], Some(ItemType::RedstoneTorch));
        assert_eq!(rows[0][1], Some(ItemType::RedstoneDust));
        assert_eq!(rows[0][2], Some(ItemType::RedstoneTorch));
        assert_eq!(rows[1][0], Some(ItemType::Stone));
        assert_eq!(rows[1][1], Some(ItemType::Stone));
        assert_eq!(rows[1][2], Some(ItemType::Stone));
    }

    #[test]
    fn test_shaped_recipe_metadata_is_available() {
        let pickaxe = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::WoodPickaxe)
            .expect("wood pickaxe recipe should exist");
        let rows = pickaxe
            .ingredient_rows()
            .expect("pickaxe recipe should be shaped");
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].len(), 3);
        assert_eq!(rows[0][0], Some(ItemType::Planks));
        assert_eq!(rows[2][1], Some(ItemType::Stick));

        let blaze_powder = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::BlazePowder)
            .expect("blaze powder recipe should exist");
        assert!(blaze_powder.ingredient_rows().is_none());
    }

    #[test]
    fn test_shape_size_blocks_three_by_three_recipe_without_table() {
        let mut inv = Inventory::new(8);
        inv.add_item(ItemType::Planks, 3);
        inv.add_item(ItemType::Stick, 2);

        let mut pickaxe = Recipe::all()
            .into_iter()
            .find(|r| r.result == ItemType::WoodPickaxe)
            .expect("wood pickaxe recipe should exist");
        pickaxe.needs_crafting_table = false;
        assert!(!pickaxe.can_craft(&inv, false, false));
    }

    #[test]
    fn test_vanilla_output_counts_for_door_and_slab() {
        let recipes = Recipe::all();
        let door = recipes
            .iter()
            .find(|r| r.result == ItemType::WoodDoor)
            .expect("wood door recipe should exist");
        assert_eq!(door.result_count, 3);

        let slab = recipes
            .iter()
            .find(|r| r.result == ItemType::StoneSlab)
            .expect("stone slab recipe should exist");
        assert_eq!(slab.result_count, 6);
    }

    #[test]
    fn test_armor_recipes_exist() {
        let recipes = Recipe::all();
        assert!(recipes.iter().any(|r| r.result == ItemType::LeatherHelmet));
        assert!(
            recipes
                .iter()
                .any(|r| r.result == ItemType::LeatherChestplate)
        );
        assert!(
            recipes
                .iter()
                .any(|r| r.result == ItemType::LeatherLeggings)
        );
        assert!(recipes.iter().any(|r| r.result == ItemType::LeatherBoots));
        assert!(recipes.iter().any(|r| r.result == ItemType::IronHelmet));
        assert!(recipes.iter().any(|r| r.result == ItemType::IronChestplate));
        assert!(recipes.iter().any(|r| r.result == ItemType::IronLeggings));
        assert!(recipes.iter().any(|r| r.result == ItemType::IronBoots));
        assert!(recipes.iter().any(|r| r.result == ItemType::DiamondHelmet));
        assert!(
            recipes
                .iter()
                .any(|r| r.result == ItemType::DiamondChestplate)
        );
        assert!(
            recipes
                .iter()
                .any(|r| r.result == ItemType::DiamondLeggings)
        );
        assert!(recipes.iter().any(|r| r.result == ItemType::DiamondBoots));
    }

    #[test]
    fn test_armor_points_and_slot_index_mapping() {
        assert_eq!(ItemType::LeatherHelmet.armor_slot_index(), Some(0));
        assert_eq!(ItemType::IronChestplate.armor_slot_index(), Some(1));
        assert_eq!(ItemType::DiamondLeggings.armor_slot_index(), Some(2));
        assert_eq!(ItemType::LeatherBoots.armor_slot_index(), Some(3));
        assert_eq!(ItemType::Stick.armor_slot_index(), None);

        assert_eq!(ItemType::LeatherHelmet.armor_points(), 1);
        assert_eq!(ItemType::IronChestplate.armor_points(), 6);
        assert_eq!(ItemType::DiamondLeggings.armor_points(), 6);
        assert_eq!(ItemType::DiamondBoots.armor_points(), 3);
        assert_eq!(ItemType::Stick.armor_points(), 0);
    }
}
