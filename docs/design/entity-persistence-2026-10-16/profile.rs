use std::{time::Instant,fs};
use termcraft::{engine::{state::GameState,net::ServerSimulation},entities::item_entity::ItemEntity,world::item::ItemType};
fn p95(samples:&mut [f64])->f64 {samples.sort_by(f64::total_cmp);samples[(samples.len()*95/100).min(samples.len()-1)]}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let base=std::env::temp_dir().join(format!("termcraft-persistence-profile-{}",std::process::id()));
    for count in [64,512,2048] {
        let root=base.join(count.to_string());
        let mut state=GameState::open_in_directory(&root)?;
        state.item_entities=(0..count).map(|i| {let mut e=ItemEntity::new(state.player.x+i as f64,state.player.y,ItemType::Coal);e.age=1234;e}).collect();
        state.world.try_save_all()?;
        let mut native=Vec::new();
        for _ in 0..20 {let t=Instant::now();state.try_save_progression()?;native.push(t.elapsed().as_secs_f64()*1000.0);}
        let native_bytes=fs::metadata(root.join("player_progression.bin"))?.len();
        let restored=GameState::open_in_directory(&root)?;
        assert_eq!(restored.item_entities,state.item_entities);
        drop(restored);
        let mut server=ServerSimulation::with_state(state);
        server.load_persistent()?;
        server.authenticate(1,None)?;
        let mut shared=Vec::new();
        for _ in 0..20 {let t=Instant::now();server.save()?;shared.push(t.elapsed().as_secs_f64()*1000.0);}
        let shared_bytes=fs::metadata(root.join("server-state.json"))?.len();
        drop(server);
        let start=Instant::now();
        let restored=GameState::open_in_directory(&root)?;
        let load_ms=start.elapsed().as_secs_f64()*1000.0;
        assert_eq!(restored.item_entities.len(),count);
        assert!(restored.item_entities.iter().all(|e|e.age==1234));
        println!("{}\t{:.4}\t{:.4}\t{}\t{}\t{:.4}",count,p95(&mut native),p95(&mut shared),native_bytes,shared_bytes,load_ms);
    }
    fs::remove_dir_all(base)?;
    Ok(())
}
