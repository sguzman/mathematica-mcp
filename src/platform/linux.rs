use std::env;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Context};

pub struct LinuxPlatform;

impl super::Platform for LinuxPlatform {
    fn shellexpand_path(raw: &str) -> anyhow::Result<PathBuf> {
        let expanded = raw.replace('~', &env::var("HOME").unwrap_or_else(|_| "~".to_string()));
        Ok(PathBuf::from(expanded))
    }

    fn validate_executable(path: &Path) -> anyhow::Result<()> {
        use std::os::unix::fs::PermissionsExt;
        let md = std::fs::metadata(path)
            .with_context(|| format!("kernel path does not exist: {path:?}"))?;
        
        let mode = md.permissions().mode();
        if mode & 0o111 == 0 {
            return Err(anyhow!("WOLFRAM_KERNEL_PATH is not executable: {}", path.display()));
        }
        Ok(())
    }

    fn get_default_kernel_names() -> &'static [&'static str] {
        &["WolframKernel", "MathKernel"]
    }

    fn get_extra_lookup_paths() -> Vec<PathBuf> {
        vec![]
    }
}

pub fn shellexpand_path(raw: &str) -> anyhow::Result<PathBuf> {
    <LinuxPlatform as super::Platform>::shellexpand_path(raw)
}

pub fn validate_executable(path: &Path) -> anyhow::Result<()> {
    <LinuxPlatform as super::Platform>::validate_executable(path)
}

pub fn get_default_kernel_names() -> &'static [&'static str] {
    <LinuxPlatform as super::Platform>::get_default_kernel_names()
}

pub fn get_extra_lookup_paths() -> Vec<PathBuf> {
    <LinuxPlatform as super::Platform>::get_extra_lookup_paths()
}
