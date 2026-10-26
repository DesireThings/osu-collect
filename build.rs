use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=src/realm_wrapper.cpp");
    println!("cargo:rerun-if-changed=include/realm_wrapper.hpp");
    println!("cargo:rerun-if-changed=src/realm_bridge.rs");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let realm_core_dir = manifest_dir.join("vendor/realm-cpp/realm-core");

    let target = env::var("TARGET").unwrap_or_default();
    let is_windows_gnu = target == "x86_64-pc-windows-gnu";
    let is_msvc = target.contains("msvc");

    // True when `-C target-feature=+crt-static` is in effect (see .cargo/config.toml).
    // cc-rs reads the same env var to pick /MT, so realm-core must match or LNK2038.
    let crt_static = env::var("CARGO_CFG_TARGET_FEATURE")
        .map(|features| features.split(',').any(|f| f == "crt-static"))
        .unwrap_or(false);

    // Build realm-core only (skip cpprealm SDK which has compiler issues)
    let mut cmake_config = cmake::Config::new(&realm_core_dir);
    cmake_config
        .define("REALM_BUILD_LIB_ONLY", "ON")
        .define("REALM_ENABLE_SYNC", "OFF")
        .define("REALM_NO_TESTS", "ON")
        .define("BUILD_TESTING", "OFF")
        .define("CMAKE_BUILD_TYPE", "Release")
        .define("CMAKE_CXX_STANDARD", "17");

    if is_msvc {
        // Match realm-core's CRT to Rust's: static (/MT) when crt-static is set, otherwise
        // dynamic (/MD). A mismatch between the two halves of the link is LNK2038.
        let runtime = if crt_static {
            "MultiThreaded"
        } else {
            "MultiThreadedDLL"
        };
        cmake_config.define("CMAKE_MSVC_RUNTIME_LIBRARY", runtime);
    } else {
        cmake_config.cxxflag("-Wno-error");
    }

    let realm_build = cmake_config.build();

    let realm_lib_dir = realm_build.join("lib");
    let realm_include_dir = realm_core_dir.join("src");

    // On Windows, realm-core downloads zlib to its build directory
    let zlib_dir = realm_build.join("build/zlib/lib");
    if zlib_dir.exists() {
        if is_windows_gnu {
            let zlib_lib = zlib_dir.join("zlib.lib");
            for alias in ["libz.a", "libzlib.a"] {
                let alias_path = zlib_dir.join(alias);
                if zlib_lib.exists() && !alias_path.exists() {
                    fs::copy(&zlib_lib, alias_path).unwrap();
                }
            }
        }
        println!("cargo:rustc-link-search=native={}", zlib_dir.display());
    }

    println!("cargo:rustc-link-search=native={}", realm_lib_dir.display());
    println!("cargo:rustc-link-lib=static=realm");
    println!("cargo:rustc-link-lib=static=realm-parser");

    // Link system dependencies based on target OS
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    match target_os.as_str() {
        "linux" => {
            println!("cargo:rustc-link-lib=dylib=stdc++");
            println!("cargo:rustc-link-lib=dylib=pthread");
            println!("cargo:rustc-link-lib=dylib=z");
            println!("cargo:rustc-link-lib=dylib=crypto");
        }
        "macos" => {
            println!("cargo:rustc-link-lib=dylib=c++");
            println!("cargo:rustc-link-lib=framework=Security");
            println!("cargo:rustc-link-lib=framework=Foundation");
        }
        "windows" => {
            println!("cargo:rustc-link-lib=dylib=bcrypt");
            println!("cargo:rustc-link-lib=dylib=ws2_32");
            println!("cargo:rustc-link-lib=dylib=crypt32");
            println!("cargo:rustc-link-lib=dylib=advapi32");
            println!("cargo:rustc-link-lib=dylib=version");
            // Link zlib (downloaded by realm-core on Windows)
            println!("cargo:rustc-link-lib=static=zlib");
            // Force static C++ runtime on mingw so the EXE doesn't depend on libstdc++-6.dll.
            // `-l:libstdc++.a` targets the archive directly without flipping the linker into
            // -Bstatic mode. The group wrapper lets ld rescan so libstdc++.a's __imp_setlocale
            // / strxfrm / wcscoll refs resolve against the msvcrt import lib re-added after it.
            if is_windows_gnu {
                println!("cargo:rustc-link-arg=-Wl,--start-group");
                println!("cargo:rustc-link-arg=-l:libstdc++.a");
                println!("cargo:rustc-link-arg=-l:libwinpthread.a");
                println!("cargo:rustc-link-arg=-lmingwex");
                println!("cargo:rustc-link-arg=-lmoldname");
                println!("cargo:rustc-link-arg=-lmsvcrt");
                println!("cargo:rustc-link-arg=-lkernel32");
                println!("cargo:rustc-link-arg=-Wl,--end-group");
            }
        }
        _ => {}
    }

    // Build our C++ wrapper with cxx
    let mut cxx_builder = cxx_build::bridge("src/realm_bridge.rs");
    cxx_builder
        .file("src/realm_wrapper.cpp")
        .include(&realm_include_dir)
        .include(realm_build.join("include"))
        .include("include");

    // Suppress cc-rs' default dynamic stdc++ link on mingw; we emit static=stdc++ above.
    if is_windows_gnu {
        cxx_builder.cpp_link_stdlib(None);
    }

    if is_msvc {
        cxx_builder
            .flag("/std:c++17")
            .flag("/EHsc") // Enable C++ exception handling
            .define("WIN32_LEAN_AND_MEAN", None) // Prevent winsock.h inclusion (use winsock2.h)
            .define("NOMINMAX", None); // Prevent min/max macro conflicts
    } else {
        cxx_builder
            .flag_if_supported("-std=c++17")
            .flag_if_supported("-Wno-unused-parameter")
            .flag_if_supported("-Wno-error")
            .flag_if_supported("-Wno-maybe-uninitialized");
    }

    cxx_builder.compile("osu_realm_bridge");

    println!("cargo:rerun-if-changed=vendor/realm-cpp/realm-core");

    emit_auth_credentials();
}

/// Builds the osu! API client id/secret (for guest searches) into the binary,
/// from the environment or a local `.env`. The secret ends up in the binary
/// as plain text, so a build made this way is for your own use only: never
/// publish one. CI and release builds don't set these.
fn emit_auth_credentials() {
    println!("cargo:rerun-if-env-changed=OSU_CLIENT_ID");
    println!("cargo:rerun-if-env-changed=OSU_CLIENT_SECRET");
    println!("cargo:rerun-if-changed=.env");

    // Environment variables take precedence over `.env`.
    let id_from_env = env::var("OSU_CLIENT_ID").ok();
    let secret_from_env = env::var("OSU_CLIENT_SECRET").ok();

    let (client_id, client_secret) = match (id_from_env, secret_from_env) {
        (Some(id), Some(secret)) => (id, secret),
        _ => {
            // Fall back to .env file for local development.
            let (id, secret) = read_dot_env();
            (id.unwrap_or_default(), secret.unwrap_or_default())
        }
    };

    if !client_id.is_empty() {
        println!("cargo:rustc-env=OSU_CLIENT_ID={client_id}");
    }
    if !client_secret.is_empty() {
        println!("cargo:rustc-env=OSU_CLIENT_SECRET={client_secret}");
    }
}

fn read_dot_env() -> (Option<String>, Option<String>) {
    let Ok(contents) = fs::read_to_string(".env") else {
        return (None, None);
    };
    let mut id = None;
    let mut secret = None;
    for line in contents.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((key, val)) = line.split_once('=') {
            let val = val.trim().trim_matches('"').trim_matches('\'');
            match key.trim() {
                "OSU_CLIENT_ID" => id = Some(val.to_owned()),
                "OSU_CLIENT_SECRET" => secret = Some(val.to_owned()),
                _ => {}
            }
        }
    }
    (id, secret)
}
