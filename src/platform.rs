//! Host services used by the shared game. Native builds retain std behaviour.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::fs;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) use std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub(crate) use web_time::Instant;
#[cfg(target_arch = "wasm32")]
#[path = "browser/storage.rs"]
pub(crate) mod fs;

#[cfg(all(test, not(target_arch = "wasm32")))]
#[allow(dead_code)]
#[path = "browser/storage.rs"]
mod browser_storage_tests;

pub(crate) fn process_id() -> u32 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::process::id()
    }
    #[cfg(target_arch = "wasm32")]
    {
        1
    }
}

pub(crate) fn temp_dir() -> std::path::PathBuf {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::temp_dir()
    }
    #[cfg(target_arch = "wasm32")]
    {
        std::path::PathBuf::from("tmp")
    }
}
