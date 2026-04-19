use std::env;
use std::path::{Path, PathBuf};
use anyhow::Context;

pub struct WindowsPlatform;

impl super::Platform for WindowsPlatform {
    fn shellexpand_path(raw: &str) -> anyhow::Result<PathBuf> {
        // On Windows, ~ is less common, but some tools use it.
        // We'll map it to USERPROFILE.
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
        // Executable bits don't exist on Windows in the same way, 
        // usually existence and extension are enough.
        Ok(())
    }

    fn get_default_kernel_names() -> &'static [&'static str] {
        &["WolframKernel.exe", "MathKernel.exe"]
    }

    fn get_extra_lookup_paths() -> Vec<PathBuf> {
        let mut paths = vec![];
        
        // Common installation paths for Wolfram Engine and Mathematica on Windows
        let program_files = env::var("ProgramFiles").unwrap_or_else(|_| r"C:\Program Files".to_string());
        
        // Search in Wolfram Research directory
        let base_dir = PathBuf::from(&program_files).join("Wolfram Research");
        if base_dir.exists() {
            // Wolfram Engine
            let engine_dir = base_dir.join("Wolfram Engine");
            if engine_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&engine_dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            paths.push(entry.path().join("WolframKernel.exe"));
                        }
                    }
                }
            }
            
            // Mathematica
            let mathematica_dir = base_dir.join("Mathematica");
            if mathematica_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&mathematica_dir) {
                    for entry in entries.flatten() {
                        if entry.path().is_dir() {
                            paths.push(entry.path().join("WolframKernel.exe"));
                        }
                    }
                }
            }
        }
        
        paths
    }
}

pub fn shellexpand_path(raw: &str) -> anyhow::Result<PathBuf> {
    <WindowsPlatform as super::Platform>::shellexpand_path(raw)
}

pub fn validate_executable(path: &Path) -> anyhow::Result<()> {
    <WindowsPlatform as super::Platform>::validate_executable(path)
}

pub fn get_default_kernel_names() -> &'static [&'static str] {
    <WindowsPlatform as super::Platform>::get_default_kernel_names()
}

pub fn get_extra_lookup_paths() -> Vec<PathBuf> {
    <WindowsPlatform as super::Platform>::get_extra_lookup_paths()
}
