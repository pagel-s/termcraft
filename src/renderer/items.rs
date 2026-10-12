//! Shared pixel icons for world drops and inventory. Placed materials keep their palette.
use super::{Color, ItemType, Renderer, materials::Tile};
use crate::world::{Dimension, block::BlockType};

fn material(item: ItemType) -> Option<Tile> {
    if matches!(
        item,
        ItemType::WaterBucket | ItemType::LavaBucket | ItemType::WheatSeeds | ItemType::NetherWart
    ) {
        return None;
    }
    Tile::for_block(item.to_block()?, Dimension::Overworld, BlockType::Air)
}

pub(super) fn token(item: ItemType, x: usize, y: usize) -> u8 {
    if let Some(tile) = material(item) {
        return if let Some(detail) = tile.detail {
            detail[y].as_bytes()[x]
        } else {
            tile.pixels[y / 2].as_bytes()[x / 2]
        };
    }
    pattern(item)[y].as_bytes()[x]
}

pub(super) fn color(item: ItemType, token: u8) -> Option<Color> {
    if token == b' ' {
        return None;
    }
    if let Some(tile) = material(item) {
        return Some(Renderer::rgb(
            tile.colors[match token {
                b'.' => 0,
                b'+' => 1,
                b'-' => 2,
                _ => 3,
            }],
        ));
    }
    use ItemType::*;
    let body = match item {
        Bucket => (46, 52, 62),
        Coal | Gunpowder | Flint => (68, 72, 83),
        CookedBeef | CookedPorkchop | CookedMutton => (153, 97, 63),
        CookedChicken | CookedFish => (195, 151, 93),
        RawFish => (99, 165, 169),
        RottenFlesh => (118, 117, 68),
        RawIron => (174, 133, 103),
        RawGold => (220, 177, 72),
        AwkwardPotion => (163, 126, 188),
        WaterBucket | WaterBottle | PotionRegeneration => (77, 145, 196),
        LavaBucket | PotionFireResistance => (237, 139, 56),
        PotionHealing => (206, 90, 117),
        _ => Renderer::color_rgb(Renderer::inventory_item_color(item)).unwrap_or((180, 190, 200)),
    };
    Some(Renderer::rgb(match token {
        b't' => (139, 99, 61),
        b'g' => (207, 216, 217),
        b'd' => (42, 45, 56),
        b'+' => Renderer::lerp_rgb(body, (246, 237, 215), 0.4),
        b'-' => Renderer::lerp_rgb(body, (22, 25, 36), 0.4),
        _ => body,
    }))
}

fn pattern(item: ItemType) -> [&'static str; 8] {
    use ItemType::*;
    match item {
        WoodSword | StoneSword | IronSword | DiamondSword => [
            "      + ", "     ++ ", "    aa  ", "   aa   ", "  aa    ", " tta    ", "  tt    ",
            " t      ",
        ],
        WoodPickaxe | StonePickaxe | IronPickaxe | DiamondPickaxe => [
            "   +++  ", "  aa aa ", " aa t a ", "    t   ", "   t    ", "  t     ", " t      ",
            "        ",
        ],
        WoodAxe | StoneAxe | IronAxe | DiamondAxe => [
            "    ++  ", "   aaa  ", "   aaat ", "    at  ", "    t   ", "   t    ", "  t     ",
            "        ",
        ],
        WoodShovel | StoneShovel | IronShovel | DiamondShovel => [
            "     ++ ", "    aaa ", "    aa  ", "    t   ", "   t    ", "  t     ", " t      ",
            "        ",
        ],
        WoodHoe | StoneHoe | IronHoe | DiamondHoe => [
            "   +++  ", "   aaat ", "      t ", "     t  ", "    t   ", "   t    ", "  t     ",
            "        ",
        ],
        LeatherHelmet | IronHelmet | DiamondHelmet => [
            "        ", "  ++++  ", " aaaaaa ", " aa  aa ", " a    a ", " -    - ", "        ",
            "        ",
        ],
        LeatherChestplate | IronChestplate | DiamondChestplate => [
            " +    + ", " aa  aa ", "aaaaaaa-", " aaaaaa ", " aaaaaa ", " aaaaaa ", " ------ ",
            "        ",
        ],
        LeatherLeggings | IronLeggings | DiamondLeggings => [
            "        ", " ++++++ ", " aaaaaa ", " aa  aa ", " aa  aa ", " aa  aa ", " --  -- ",
            "        ",
        ],
        LeatherBoots | IronBoots | DiamondBoots => [
            "        ", "        ", "  +  +  ", " aa  aa ", " aa  aa ", "aaa  aaa", "---  ---",
            "        ",
        ],
        Bucket | WaterBucket | LavaBucket => [
            "  gggg  ", " g    g ", "g      g", "g aaaa g", " gaaaa- ", " gaaaa- ", "  gggg  ",
            "        ",
        ],
        GlassBottle | WaterBottle | AwkwardPotion | PotionHealing | PotionStrength
        | PotionRegeneration | PotionFireResistance => [
            "   tt   ", "   gg   ", "   gg   ", "  g++g  ", " gaaaag ", " gaaaag ", "  gggg  ",
            "        ",
        ],
        Bow => [
            "  t     ", "   tg   ", "    tg  ", "    t g ", "    t g ", "    tg  ", "   tg   ",
            "  t     ",
        ],
        FishingRod => [
            "     t  ", "    tgg ", "   t  g ", "  t   g ", " t    g ", "t     g ", "      a ",
            "        ",
        ],
        Arrow => [
            "      g ", "     ggg", "    t g ", "   t    ", "  t     ", " gt     ", "gg      ",
            "        ",
        ],
        Stick | BlazeRod | Bone => [
            "        ", "     +  ", "    a   ", "   a    ", "  a     ", " a      ", " -      ",
            "        ",
        ],
        Feather => [
            "     +  ", "    ++  ", "   +aa  ", "  +aa   ", " +aa    ", "  a     ", " t      ",
            "        ",
        ],
        Diamond | Flint | Coal | RawIron | RawGold => [
            "        ", "   ++   ", "  +aaa  ", " +aaaaa ", "  aaaa- ", "   aa-  ", "    -   ",
            "        ",
        ],
        IronIngot | GoldIngot => [
            "        ", "        ", "  ++++  ", " +++++a ", "aaaaaaa-", " ------ ", "        ",
            "        ",
        ],
        EnderPearl | Slimeball | MagmaCream => [
            "        ", "  ++++  ", " ++aaaa ", " +aaaaa ", " aaaaa- ", " aaaa-- ", "  ----  ",
            "        ",
        ],
        EyeOfEnder => [
            "        ", "  ++++  ", " ++ddaa ", " +addaa ", " aaddaa ", " aaaa-- ", "  ----  ",
            "        ",
        ],
        Egg | GhastTear => [
            "        ", "   ++   ", "  +aaa  ", "  +aaa  ", " +aaaa- ", " aaaaa- ", "  ----  ",
            "        ",
        ],
        Book => [
            "        ", " ttttt  ", " tggggt ", " tggggt ", " tggggt ", " tggggt ", " tttttt ",
            "        ",
        ],
        Paper => [
            "        ", " +++++  ", " gggg+g ", " gggggg ", " gggggg ", " gggggg ", " gggggg ",
            "        ",
        ],
        FlintAndSteel | Shears => [
            "  gg  a ", " g  gaa ", "  gg a  ", "   ga   ", "   ag   ", "  a gg  ", " aa  gg ",
            "        ",
        ],
        Wheat => [
            " +  + + ", " a+ a+a ", " +aaa+  ", "  a+aa  ", "   ta   ", "  t t   ", " t  t   ",
            "        ",
        ],
        WheatSeeds | NetherWart | BoneMeal | Gunpowder | BlazePowder => [
            "        ", "   a    ", " a  +   ", "  + a+  ", " aa a   ", " aaaaa  ", "  ---   ",
            "        ",
        ],
        Bread => [
            "        ", "        ", "  ++++  ", " +a+a+a ", "aaaaaaa ", " ------ ", "        ",
            "        ",
        ],
        RawBeef | CookedBeef | RawMutton | CookedMutton | RawPorkchop | CookedPorkchop
        | RottenFlesh => [
            "        ", "  ++aa  ", " +aaaaa ", " aaggga ", " aaaaga ", "  aaa-  ", "   --   ",
            "        ",
        ],
        RawChicken | CookedChicken => [
            "        ", "   ++   ", "  +aaa  ", "  aaaa  ", "   aa   ", "    gg  ", "     g  ",
            "        ",
        ],
        RawFish | CookedFish => [
            "        ", "    a   ", "  +aaa g", " +ad aag", "  aaaa g", "    a   ", "        ",
            "        ",
        ],
        String => [
            "        ", "  ggg   ", " g  gg  ", " g g gg ", "  gg gg ", "    gg  ", " gggg   ",
            "        ",
        ],
        Leather => [
            "        ", " aa  aa ", " aaaaaa ", "  aaaa  ", "  aaaa  ", " aaaaaa ", " --  -- ",
            "        ",
        ],
        Boat => [
            "        ", " t    t ", " tt  tt ", " tttttt ", "  aaaa  ", "   --   ", "        ",
            "        ",
        ],
        // These use the actual block material above, including transparent shapes.
        Dirt | Grass | Stone | Wood | Leaves | Sand | Gravel | Planks | CraftingTable | Torch
        | Lever | StoneButton | RedstoneTorch | Furnace | Wool | Snow | Ice | Cactus | DeadBush
        | BirchWood | BirchLeaves | RedFlower | YellowFlower | TallGrass | Cobblestone
        | Obsidian | RedstoneDust | Netherrack | SoulSand | Glowstone | Tnt | Piston | EndStone
        | Bed | Chest | Glass | WoodDoor | Ladder | StoneSlab | StoneStairs | EnchantingTable
        | Anvil | BrewingStand | RedstoneRepeater | Sapling | SugarCane | Bookshelf
        | BirchSapling => unreachable!("placed item must use material atlas"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_item_has_a_visible_valid_icon() {
        for item in ItemType::ALL {
            let mut occupied = 0;
            for y in 0..8 {
                for x in 0..8 {
                    if color(item, token(item, x, y)).is_some() {
                        occupied += 1;
                    }
                }
            }
            assert!(occupied >= 4, "invisible item: {item:?}");
        }
    }
    #[test]
    fn item_family_silhouettes_have_valid_rows_and_distinct_shapes() {
        let kinds = [
            ItemType::DiamondSword,
            ItemType::DiamondPickaxe,
            ItemType::DiamondHoe,
            ItemType::IronChestplate,
            ItemType::IronLeggings,
            ItemType::WaterBucket,
            ItemType::PotionHealing,
            ItemType::Bow,
            ItemType::Book,
            ItemType::RawFish,
        ];
        let mut silhouettes = std::collections::HashSet::new();
        for item in kinds {
            let rows = pattern(item);
            assert!(rows.iter().all(|r| r.len() == 8));
            silhouettes.insert(rows);
        }
        assert_eq!(silhouettes.len(), kinds.len());
        assert_ne!(
            color(ItemType::WaterBucket, b'a'),
            color(ItemType::LavaBucket, b'a')
        );
    }
}
