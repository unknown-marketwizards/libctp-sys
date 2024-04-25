use regex::Regex;

use std::env;
use std::path::{Path, PathBuf};

mod auto_bind;

fn main() {
    let api_root = PathBuf::from("traderapi");
    let api_root = std::env::current_dir().unwrap().join(api_root);

    let api_include_path = api_root.join("include");

    let hxx_file = PathBuf::from(env::var("OUT_DIR").unwrap()).join("wrapper.hpp");
    let cxx_file = PathBuf::from(env::var("OUT_DIR").unwrap()).join("wrapper.cpp");
    auto_bind::auto_bind(&api_include_path, &hxx_file, &cxx_file);

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

    // ctp api header is clean enough, we will use blacklist instead whitelist
    let bindings = bindgen::Builder::default()
        // The input header we would like to generate
        // bindings for.
        .header(hxx_file.to_str().unwrap())
        .clang_arg(format!("-I{}", api_include_path.to_str().unwrap()))
        // Tell cargo to invalidate the built crate whenever any of the
        // included header files changed.
        .derive_debug(true)
        // make output smaller
        .layout_tests(false)
        .generate_comments(false)
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        // we will handle class mannually by `autobind.py`
        // function defined in rust
        .opaque_type("CThostFtdcTraderApi")
        .opaque_type("CThostFtdcTraderSpi")
        .opaque_type("CThostFtdcMdApi")
        .opaque_type("CThostFtdcMdSpi")
        // Finish the builder and generate the bindings.
        .generate()
        // Unwrap the Result and panic on failure.
        .expect("Unable to generate bindings");

    let outfile = PathBuf::from(env::var("OUT_DIR").unwrap()).join("bindings.rs");
    bindings
        .write_to_file(&outfile)
        .expect("Couldn't write bindings!");

    let buf = replace_trait(
        &outfile,
        &[
            "Rust_CThostFtdcMdSpi_Trait",
            "Rust_CThostFtdcTraderSpi_Trait",
        ],
    )
    .expect("Fail to replace trait!");
    std::fs::write(&outfile, &buf).expect("Fail to write converted bindings!");

    if os == "win" {
        fixed_link_name(&outfile);
    }
}

fn fixed_link_name(f_name: &Path) {
    let buf = std::fs::read_to_string(f_name)
        .unwrap()
        .replace(
            "??_DRust_CThostFtdcMdApi@@QEAAXXZ",
            "??1Rust_CThostFtdcMdApi@@QEAA@XZ",
        )
        .replace(
            "??_DRust_CThostFtdcMdSpi@@QEAAXXZ",
            "??1Rust_CThostFtdcMdSpi@@QEAA@XZ",
        )
        .replace(
            "??_DRust_CThostFtdcTraderApi@@QEAAXXZ",
            "??1Rust_CThostFtdcTraderApi@@QEAA@XZ",
        )
        .replace(
            "??_DRust_CThostFtdcTraderSpi@@QEAAXXZ",
            "??1Rust_CThostFtdcTraderSpi@@QEAA@XZ",
        );

    std::fs::write(f_name, &buf).expect("Fail to write converted bindings!");
}

fn camel_to_snake(name: &str) -> String {
    let pattern1: Regex = Regex::new(r"(.)([A-Z][a-z]+)").unwrap();
    let pattern2: Regex = Regex::new(r"([a-z0-9])([A-Z])").unwrap();

    pattern2
        .replace_all(
            pattern1.replace_all(name, r"${1}_${2}").as_ref(),
            r"${1}_${2}",
        )
        .to_lowercase()
}

fn replace_trait(f_name: &Path, traits: &[&str]) -> Result<String, Box<dyn std::error::Error>> {
    let mut buf = std::fs::read_to_string(f_name)?;
    for trait_extern in traits {
        let pattern = Regex::new(&format!(
            r#"extern \s*"C"\s*\{{\s*pub\s+fn\s+{}_(\w+)\s*\(([^)]*)\)([^;]*);\s*}}\s*"#,
            trait_extern
        ))
        .unwrap();
        let pattern_arg = Regex::new(r"\s*(\w+)\s*:\s*(.*)\s*").unwrap();

        let mut exports = vec![];
        let mut traitfuns = vec![];
        assert!(
            pattern.captures(&buf).is_some(),
            "`{}` not found in source code",
            trait_extern
        );
        for cap in pattern.captures_iter(&buf) {
            let fname = cap.get(1).unwrap().as_str().trim();
            let args: Vec<_> = cap
                .get(2)
                .unwrap()
                .as_str()
                .split(",")
                .filter(|s| s.trim().len() > 0)
                .map(|s| {
                    let c = pattern_arg.captures(s).unwrap();
                    (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str())
                })
                .collect();
            let rtn = cap.get(3).unwrap().as_str();
            let fname_camel = camel_to_snake(fname);
            if fname_camel == "drop" {
                continue;
            }
            assert!(args[0].1.trim().ends_with("c_void"));

            let mut tmp = args[1..]
                .iter()
                .map(|s| format!("{}: {}", s.0, s.1))
                .collect::<Vec<_>>();
            tmp.insert(0, "trait_obj: *mut ::std::os::raw::c_void".into());
            let args_repl = tmp.join(", ");
            let argv_repl = args[1..].iter().map(|s| s.0).collect::<Vec<_>>().join(", ");

            let export = format!(
                r#"#[no_mangle]
pub extern "C" fn {trait_extern}_{fname}({args_repl}){rtn} {{
    let trait_obj = trait_obj as *mut Box<dyn {trait_extern}>;
    let trait_obj: &mut dyn {trait_extern} = unsafe {{ &mut **trait_obj }};
    trait_obj.{fname_camel}({argv_repl})
}}
"#,
                trait_extern = trait_extern,
                fname = fname,
                args_repl = args_repl,
                rtn = rtn,
                fname_camel = fname_camel,
                argv_repl = argv_repl
            );
            exports.push(export);

            let mut tmp = args[1..]
                .iter()
                .map(|s| format!("{}: {}", s.0, s.1))
                .collect::<Vec<_>>();
            tmp.insert(0, "&mut self".into());
            let args_repl = tmp.join(", ");
            let traitfun = format!(
                r"    fn {fname_camel}({args_repl}){rtn} {{  }}",
                fname_camel = fname_camel,
                args_repl = args_repl,
                rtn = rtn
            );
            traitfuns.push(traitfun);
        }

        let exports_repl = exports.join("\n");
        let traitfuns_repl = traitfuns.join("\n");

        buf = format!(
            r#"{ori}
#[allow(unused)]
pub trait {trait_extern} {{
{traitfuns_repl}
}}

{exports_repl}
#[no_mangle]
pub extern "C" fn {trait_extern}_Drop(trait_obj: *mut ::std::os::raw::c_void) {{
    let trait_obj = trait_obj as *mut Box<dyn {trait_extern}>;
    let _r: Box<Box<dyn {trait_extern}>> = unsafe {{ Box::from_raw(trait_obj) }};
}}
"#,
            ori = pattern.replace_all(&buf, "").to_string(),
            exports_repl = exports_repl,
            trait_extern = trait_extern,
            traitfuns_repl = traitfuns_repl
        );
    }

    Ok(buf)
}
