//! Persistent ambience regions. They change biome-dependent weather, color and
//! spawning without replacing terrain or anything the player has built.
use super::recipe::{Region, Terrain};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;

const FILE: &str = "environment.json";

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedEnvironment {
    version: u32,
    regions: Vec<Region>,
}

pub fn read(root: &Path) -> Result<Vec<Region>, String> {
    let file = match crate::platform::fs::File::open(root.join(FILE)) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read environment: {e}")),
    };
    let mut bytes = Vec::new();
    file.take(32769)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 32768 {
        return Err("environment save is too large".into());
    }
    let saved: SavedEnvironment =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid environment save: {e}"))?;
    if saved.version != 1 {
        return Err("unsupported environment save version".into());
    }
    validate(&saved.regions)?;
    Ok(saved.regions)
}

pub fn validate(regions: &[Region]) -> Result<(), String> {
    if regions.len() > 16 {
        return Err("at most 16 ambience regions per world".into());
    }
    for (i, region) in regions.iter().enumerate() {
        if region.start_x < -100_000
            || region.end_x > 100_000
            || region.start_x > region.end_x
            || region.end_x - region.start_x > 127
        {
            return Err("ambience regions must cover 1–128 columns within x=±100000".into());
        }
        if regions[..i]
            .iter()
            .any(|other| region.start_x <= other.end_x && other.start_x <= region.end_x)
        {
            return Err("ambience regions overlap".into());
        }
    }
    Ok(())
}

pub fn replace(regions: &[Region], replacement: Region) -> Result<Vec<Region>, String> {
    validate(std::slice::from_ref(&replacement))?;
    let mut next = Vec::new();
    for region in regions {
        if region.end_x < replacement.start_x || region.start_x > replacement.end_x {
            next.push(region.clone());
        } else {
            if region.start_x < replacement.start_x {
                next.push(Region {
                    start_x: region.start_x,
                    end_x: replacement.start_x - 1,
                    terrain: region.terrain,
                });
            }
            if region.end_x > replacement.end_x {
                next.push(Region {
                    start_x: replacement.end_x + 1,
                    end_x: region.end_x,
                    terrain: region.terrain,
                });
            }
        }
    }
    next.push(replacement);
    next.sort_by_key(|r| r.start_x);
    validate(&next)?;
    Ok(next)
}

pub fn write(root: &Path, regions: &[Region]) -> Result<(), String> {
    validate(regions)?;
    let data = serde_json::to_vec(&SavedEnvironment {
        version: 1,
        regions: regions.to_vec(),
    })
    .map_err(|e| e.to_string())?;
    let path = root.join(FILE);
    let temp = root.join(format!(
        ".environment-{}-{}.tmp",
        crate::platform::process_id(),
        rand::random::<u64>()
    ));
    let result = (|| -> std::io::Result<()> {
        crate::platform::fs::create_dir_all(root)?;
        crate::checkpoint::protect(&path)?;
        let mut file = crate::platform::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        file.write_all(&data)?;
        file.sync_all()?;
        drop(file);
        crate::platform::fs::rename(&temp, path)?;
        crate::checkpoint::sync_directory(root)
    })();
    if result.is_err() {
        let _ = crate::platform::fs::remove_file(temp);
    }
    result.map_err(|e| format!("cannot save environment: {e}"))
}

pub fn biome(terrain: Terrain) -> super::BiomeType {
    match terrain {
        Terrain::Plains => super::BiomeType::Plains,
        Terrain::Forest => super::BiomeType::Forest,
        Terrain::Desert => super::BiomeType::Desert,
        Terrain::Tundra => super::BiomeType::Tundra,
        Terrain::Mountains => super::BiomeType::ExtremeHills,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replacement_preserves_unaffected_area_and_rejects_over_budget() {
        let old = vec![Region {
            start_x: -40,
            end_x: 40,
            terrain: Terrain::Forest,
        }];
        let next = replace(
            &old,
            Region {
                start_x: -10,
                end_x: 10,
                terrain: Terrain::Tundra,
            },
        )
        .unwrap();
        assert_eq!(next.len(), 3);
        assert_eq!((next[0].start_x, next[0].end_x), (-40, -11));
        assert_eq!((next[2].start_x, next[2].end_x), (11, 40));
        assert!(
            replace(
                &next,
                Region {
                    start_x: 0,
                    end_x: 200,
                    terrain: Terrain::Desert
                }
            )
            .is_err()
        );
    }
}
