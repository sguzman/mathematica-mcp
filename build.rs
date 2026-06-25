#[cfg(target_os = "windows")]
fn main() {
  use std::path::PathBuf;

  let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("missing CARGO_MANIFEST_DIR"));
  let dll_path = manifest_dir.join("wstp-sdk-windows").join("wstp64i4.dll");

  println!("cargo:rerun-if-changed={}", dll_path.display());

  if !dll_path.is_file() {
    panic!("missing WSTP runtime DLL at {}", dll_path.display());
  }

  let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("missing OUT_DIR"));
  let profile_dir = out_dir
    .ancestors()
    .nth(3)
    .expect("unable to resolve Cargo profile directory");

  copy_if_needed(&dll_path, &profile_dir.join("wstp64i4.dll"));
  copy_if_needed(&dll_path, &profile_dir.join("deps").join("wstp64i4.dll"));
}

#[cfg(not(target_os = "windows"))]
fn main() {}

#[cfg(target_os = "windows")]
fn copy_if_needed(source: &std::path::Path, destination: &std::path::Path) {
  if let Some(parent) = destination.parent() {
    std::fs::create_dir_all(parent).expect("failed to create runtime DLL directory");
  }

  let should_copy = match std::fs::metadata(destination) {
    Ok(existing) => {
      let source_len = std::fs::metadata(source).expect("failed to stat source DLL").len();
      existing.len() != source_len
    }
    Err(_) => true,
  };

  if should_copy {
    std::fs::copy(source, destination).expect("failed to copy WSTP runtime DLL");
  }
}
