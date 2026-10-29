#[cfg(target_arch = "wasm32")]
pub mod browser;
mod checkpoint;
pub mod engine;
pub mod entities;
pub mod generator;
mod platform;
pub mod renderer;
pub(crate) mod save_path;
pub use save_path::save_dir as save_directory;
pub mod world;
pub mod worlds;
