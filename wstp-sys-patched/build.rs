//! This script links the Mathematica WSTPi4 library.
//!
//! It does this by finding the local Mathematica installation by using the users
//! `wolframscript` to evaluate `$InstallationDirectory`. This script will fail if
//! `wolframscript` is not on `$PATH`.

use std::path::PathBuf;
use std::process;

use wolfram_app_discovery::{SystemID, WolframApp, WolframVersion};

/// Oldest Wolfram Version that wstp-rs aims to be compatible with.
const WOLFRAM_VERSION: WolframVersion = WolframVersion::new(13, 0, 1);

fn main() {
    env_logger::init();

    // Ensure that changes to environment variables checked by wolfram-app-discovery will
    // cause cargo to rebuild the current crate.
    wolfram_app_discovery::config::set_print_cargo_build_script_directives(true);

    // This crate is being built by docs.rs. Skip trying to locate a WolframApp.
    // See: https://docs.rs/about/builds#detecting-docsrs
    if std::env::var("DOCS_RS").is_ok() {
        // Force docs.rs to use the bindings generated for this version / system.
        let bindings_path = make_bindings_path(&WOLFRAM_VERSION, SystemID::MacOSX_x86_64);

        // This environment variable is included using `env!()`. wstp-sys will fail to
        // build if it is not set correctly.
        println!(
            "cargo:rustc-env=CRATE_WSTP_SYS_BINDINGS={}",
            bindings_path.display()
        );

        return;
    }

    //
    // Error if this is a cross compilation
    //

    let host = std::env::var("HOST").expect("expected 'HOST' env var to be set");
    let target = std::env::var("TARGET").expect("expected 'TARGET' env var to be set");

    // Note: `host == target` is required for the use of `cfg!(..)` in this
    //       script to be valid.
    if host != target {
        if !target.contains("windows") {
            panic!(
                "error: crate wstp-sys does not support cross compilation. (host: {}, target: {})",
                host,
                target
            );
        }
        println!("cargo:warning=cross-compiling to Windows; assuming WSTP SDK is provided manually");
    }

    let app: Option<WolframApp> = if host == target {
        WolframApp::try_default().ok()
    } else {
        None
    };

    let target_system_id: SystemID =
        SystemID::try_from_rust_target(&std::env::var("TARGET").unwrap())
            .expect("unable to get System ID for target system");

    //-------------
    // Link to WSTP
    //-------------

    link_to_wstp(app.as_ref(), &target);

    //----------------------------------------------------
    // Generate or use pre-generated Rust bindings to WSTP
    //----------------------------------------------------
    // See docs/Maintenance.md for instructions on how to pre-generate
    // bindings for new WL versions.

    // TODO: Update to a higher minimum WSTP version and remove this workaround.
    // NOTE: WSTP didn't support 64-bit ARM Linux in v13.0.1, so pre-generated
    //       bindings aren't available. If starting Linux-ARM64, use bindings
    //       from a newer version. (This mismatch is neglible since there were
    //       no significant API changes to WSTP between these two versions anyway.)
    let wolfram_version = match target_system_id {
        SystemID::Linux_ARM64 => WolframVersion::new(13, 2, 0),
        _ => WOLFRAM_VERSION,
    };

    let bindings_path = use_pregenerated_bindings(wolfram_version, target_system_id);

    println!(
        "cargo:rustc-env=CRATE_WSTP_SYS_BINDINGS={}",
        bindings_path.display()
    );
}

//========================================================================
// Tell `lib.rs` where to find the file containing the WSTP Rust bindings.
//========================================================================

//-----------------------
// Pre-generated bindings
//-----------------------

/// Use bindings that have been pre-generated.
#[allow(dead_code)]
fn use_pregenerated_bindings(
    wolfram_version: WolframVersion,
    target_system_id: SystemID,
) -> PathBuf {
    let bindings_path = make_bindings_path(&wolfram_version, target_system_id);

    println!("cargo:rerun-if-changed={}", bindings_path.display());

    if !bindings_path.is_file() {
        println!(
            "
    ==== ERROR: wstp-sys =====

    Rust bindings for Wolfram WSTP for target configuration:

        WolframVersion:    {}
        SystemID:          {}

    have not been pre-generated.

    See wstp-sys/generated/ for a listing of currently available targets.

    =========================================
            ",
            wolfram_version, target_system_id
        );
        panic!("<See printed error>");
    }

    println!(
        "cargo:warning=info: using pre-generated bindings for WSTP ({wolfram_version}, {target_system_id}): {}",
        bindings_path.display()
    );

    bindings_path
}

fn make_bindings_path(wolfram_version: &WolframVersion, system_id: SystemID) -> PathBuf {
    let bindings_path = PathBuf::from("generated")
        .join(&wolfram_version.to_string())
        .join(system_id.as_str())
        .join("WSTP_bindings.rs");

    let absolute_bindings_path =
        PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join(&bindings_path);

    absolute_bindings_path
}

//======================================
// Link to WSTP
//======================================

/// Emits the necessary `cargo` instructions to link to the WSTP static library,
/// and also links the WSTP interface libraries (the libraries that WSTP itself
/// depends on).
fn link_to_wstp(app: Option<&WolframApp>, target: &str) {
    // Path to the WSTP library file.
    let lib_path = if target.contains("windows") && app.is_none() {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let sdk_path = std::env::var("WSTP_WINDOWS_SDK_PATH").map(PathBuf::from).unwrap_or_else(|_| {
            manifest_dir.parent().unwrap().join("wstp-sdk-windows")
        });
        
        // For dynamic linking on Windows, we need the import library (.lib)
        let candidates = ["wstp64i4.lib", "wstpi4.lib"];
        let mut found = None;
        for c in candidates {
            let p = sdk_path.join(c);
            if p.exists() {
                found = Some(p);
                break;
            }
        }
        
        found.expect(&format!("Could not find WSTP import library (wstp64i4.lib) in {}", sdk_path.display()))
    } else {
        wolfram_app_discovery::build_scripts::wstp_static_library_path(app)
            .expect("unable to get WSTP static library path")
            .into_path_buf()
    };

    println!(
        "cargo:warning=info: linking to WSTP lib from: {}",
        lib_path.display()
    );

    let search_dir = lib_path.parent().unwrap().display().to_string();
    println!("cargo:rustc-link-search=native={}", search_dir);

    if target.contains("windows") {
        let stem = lib_path.file_stem().unwrap().to_str().unwrap();
        // Use DYNAMIC linking for Windows to avoid C++ ABI issues with static libs
        println!("cargo:rustc-link-lib=dylib={}", stem);
        
        println!("cargo:rustc-link-lib=dylib=kernel32");
        println!("cargo:rustc-link-lib=dylib=user32");
        println!("cargo:rustc-link-lib=dylib=advapi32");
        println!("cargo:rustc-link-lib=dylib=comdlg32");
        println!("cargo:rustc-link-lib=dylib=ws2_32");
        println!("cargo:rustc-link-lib=dylib=wsock32");
        println!("cargo:rustc-link-lib=dylib=rpcrt4");
    } else {
        // Fallback for other platforms (usually static)
        link_wstp_statically(&lib_path, target);
        
        if target.contains("apple-darwin") {
            println!("cargo:rustc-link-lib=framework=Foundation");
        }
        if target.contains("linux") {
            println!("cargo:rustc-link-lib=uuid")
        }
    }
}

fn link_wstp_statically(lib: &PathBuf, target: &str) {
    let mut lib = lib.clone();

    if target.contains("apple-darwin") {
        if target.contains("x86_64") {
            lib = lipo_native_library(&lib, "x86_64");
        } else if target.contains("aarch64") {
            lib = lipo_native_library(&lib, "arm64");
        }
    }

    let stem = lib.file_stem().unwrap().to_str().unwrap();
    // Trim the 'lib' prefix if it exists, as rustc adds it back.
    let link_name = stem.trim_start_matches("lib");
    println!("cargo:rustc-link-lib=static={}", link_name);
}

fn lipo_native_library(wstp_lib: &PathBuf, lipo_arch: &str) -> PathBuf {
    let wstp_lib_str = wstp_lib
        .to_str()
        .expect("could not convert WSTP archive path to str");

    let is_universal_binary = {
        let stdout = process::Command::new("file")
            .args(&[wstp_lib_str])
            .output()
            .expect("failed to run `file` system utility")
            .stdout;
        let stdout = String::from_utf8(stdout).unwrap();
        stdout.contains("Mach-O universal binary")
    };

    if !is_universal_binary {
        return PathBuf::from(wstp_lib);
    }

    let output_lib = std::env::temp_dir().join("libWSTP-thin.a");
    let output_lib_str = output_lib
        .to_str()
        .expect("could not convert WSTP archive path to str");

    let output = process::Command::new("lipo")
        .args(&[wstp_lib_str, "-thin", lipo_arch, "-output", output_lib_str])
        .output()
        .expect("failed to invoke macOS `lipo` command");

    if !output.status.success() {
        panic!("unable to lipo WSTP library: {:#?}", output);
    }

    PathBuf::from(output_lib)
}
