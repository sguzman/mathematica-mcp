#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(target_os = "linux")]
pub use linux::*;
#[cfg(target_os = "windows")]
pub use windows::*;

use std::path::PathBuf;

pub trait Platform {
    fn shellexpand_path(raw: &str) -> anyhow::Result<PathBuf>;
    fn validate_executable(path: &std::path::Path) -> anyhow::Result<()>;
    fn discover_kernel_path() -> Option<PathBuf>;
    fn get_default_kernel_names() -> &'static [&'static str];
}
