use std::env;
use std::path::{Path, PathBuf};
use anyhow::Context;
use wolfram_app_discovery::WolframApp;

pub struct WindowsPlatform;

impl super::Platform for WindowsPlatform {
    fn shellexpand_path(raw: &str) -> anyhow::Result<PathBuf> {
        let expanded = if raw.contains('~') {
            let home = env::var("USERPROFILE")
                .or_else(|_| env::var("HOME"))
                .unwrap_or_else(|_| "C:".to_string());
            raw.replace('~', &home)
        } else {
            raw.to_string()
        };
        Ok(PathBuf::from(expanded))
    }

    fn validate_executable(path: &Path) -> anyhow::Result<()> {
        let md = std::fs::metadata(path)
            .with_context(|| format!("kernel path does not exist: {path:?}"))?;
        if !md.is_file() {
            return Err(anyhow::anyhow!("WOLFRAM_KERNEL_PATH is not a file: {}", path.display()));
        }
        Ok(())
    }

    fn discover_kernel_path() -> Option<PathBuf> {
        WolframApp::try_default().ok()?.kernel_executable_path().ok()
    }

    fn get_default_kernel_names() -> &'static [&'static str] {
        &["WolframKernel.exe", "MathKernel.exe"]
    }
}

pub fn shellexpand_path(raw: &str) -> anyhow::Result<PathBuf> {
    <WindowsPlatform as super::Platform>::shellexpand_path(raw)
}

pub fn validate_executable(path: &Path) -> anyhow::Result<()> {
    <WindowsPlatform as super::Platform>::validate_executable(path)
}

pub fn discover_kernel_path() -> Option<PathBuf> {
    <WindowsPlatform as super::Platform>::discover_kernel_path()
}

pub fn get_default_kernel_names() -> &'static [&'static str] {
    <WindowsPlatform as super::Platform>::get_default_kernel_names()
}
