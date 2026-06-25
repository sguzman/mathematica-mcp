use std::env;
use std::path::PathBuf;

use anyhow::Context;
use serde::Deserialize;

pub const DEFAULT_CONFIG_FILE_NAME: &str = "mathematica-mcp.toml";
const CONFIG_PATH_ENV_VAR: &str = "MATHEMATICA_MCP_CONFIG";

#[derive(Debug, Default, Deserialize, Clone)]
pub struct RuntimeConfig {
  #[serde(default)]
  pub kernel: KernelConfig
}

#[derive(Debug, Default, Deserialize, Clone)]
pub struct KernelConfig {
  pub linux_path:   Option<String>,
  pub windows_path: Option<String>
}

#[derive(Debug, Clone)]
pub struct LoadedConfig {
  pub path:   PathBuf,
  pub config: RuntimeConfig
}

impl RuntimeConfig {
  #[cfg(target_os = "linux")]
  pub fn platform_kernel_path(&self) -> Option<&str> {
    self.kernel.linux_path.as_deref().map(str::trim).filter(|path| !path.is_empty())
  }

  #[cfg(target_os = "windows")]
  pub fn platform_kernel_path(&self) -> Option<&str> {
    self.kernel.windows_path.as_deref().map(str::trim).filter(|path| !path.is_empty())
  }
}

pub fn load_config() -> anyhow::Result<Option<LoadedConfig>> {
  let path = config_path()?;
  if !path.is_file() {
    return Ok(None);
  }

  let raw = std::fs::read_to_string(&path)
    .with_context(|| format!("failed to read config file at {}", path.display()))?;
  let config: RuntimeConfig = toml::from_str(&raw)
    .with_context(|| format!("failed to parse config file at {}", path.display()))?;

  Ok(Some(LoadedConfig {
    path,
    config
  }))
}

fn config_path() -> anyhow::Result<PathBuf> {
  if let Ok(raw) = env::var(CONFIG_PATH_ENV_VAR) {
    let trimmed = raw.trim();
    if !trimmed.is_empty() {
      return Ok(PathBuf::from(trimmed));
    }
  }

  Ok(env::current_dir()?.join(DEFAULT_CONFIG_FILE_NAME))
}

#[cfg(test)]
mod tests {
  use super::{
    KernelConfig,
    RuntimeConfig
  };

  #[test]
  #[cfg(target_os = "windows")]
  fn selects_windows_path() {
    let config = RuntimeConfig {
      kernel: KernelConfig {
        linux_path:   Some(
          "/usr/local/Wolfram/WolframEngine/15.0/Executables/WolframKernel".into()
        ),
        windows_path: Some(
          "C:\\Program Files\\Wolfram Research\\Wolfram\\14.3\\WolframKernel.exe".into()
        )
      }
    };

    assert_eq!(
      config.platform_kernel_path(),
      Some("C:\\Program Files\\Wolfram Research\\Wolfram\\14.3\\WolframKernel.exe")
    );
  }

  #[test]
  #[cfg(target_os = "linux")]
  fn selects_linux_path() {
    let config = RuntimeConfig {
      kernel: KernelConfig {
        linux_path:   Some(
          "/usr/local/Wolfram/WolframEngine/15.0/Executables/WolframKernel".into()
        ),
        windows_path: Some(
          "C:\\Program Files\\Wolfram Research\\Wolfram\\14.3\\WolframKernel.exe".into()
        )
      }
    };

    assert_eq!(
      config.platform_kernel_path(),
      Some("/usr/local/Wolfram/WolframEngine/15.0/Executables/WolframKernel")
    );
  }
}
