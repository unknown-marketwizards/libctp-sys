use std::env;
use std::path::PathBuf;

fn main() {
    let api_root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("traderapi");

    let api_include_path = api_root.join("include");

    let cxx_file = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .join("cxx")
        .join("wrapper.cpp");

    let os = if cfg!(target_os = "windows") {
        "win"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        panic!("can not build on this platform.")
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "64"
    } else if cfg!(target_arch = "x86") {
        "32"
    } else {
        panic!("can not build on this platform.")
    };

    cc::Build::new()
        .cpp(true)
        .include(api_include_path.to_str().unwrap())
        .file(cxx_file.to_str().unwrap())
        .flag_if_supported("-std=c++17")
        .flag_if_supported("-w")
        .compile("wrapper");

    println!(
        "cargo:rustc-link-search={}",
        api_root
            .join("lib")
            .join(format!("{}{}", os, arch))
            .display()
    );

    println!("cargo:rustc-link-lib=dylib=thostmduserapi_se");
    println!("cargo:rustc-link-lib=dylib=thosttraderapi_se");
}
