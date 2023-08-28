use cmake::Config;
use std::{
    env,
    error::Error,
    path::{Path, PathBuf},
};

pub fn main() -> Result<(), Box<dyn Error + 'static>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);

    let src_dir = Path::new("aeron").canonicalize()?;

    let build_dir = Config::new(&src_dir)
        .define("C_WARNINGS_AS_ERRORS", "TRUE")
        .define("STANDALONE_BUILD", "FALSE")
        .define("BUILD_AERON_DRIVER", "OFF")
        .define("BUILD_AERON_ARCHIVE_API", "OFF")
        .define("AERON_TESTS", "FALSE")
        .define("AERON_UNIT_TESTS", "FALSE")
        .define("AERON_SYSTEM_TESTS", "FALSE")
        .define("AERON_BUILD_SAMPLES", "FALSE")
        .define("AERON_BUILD_DOCUMENTATION", "FALSE")
        .define("AERON_INSTALL_TARGETS", "FALSE")
        .build_target("aeron_static")
        .build();

    let includes = src_dir.join("aeron-client/src/main/c");
    println!("cargo:include={}", includes.display());

    let libs = build_dir.join("build/lib");
    println!("cargo:rustc-link-lib=static=aeron_static");
    println!("cargo:rustc-link-search=native={}", libs.display());

    bindgen::builder()
        .use_core()
        .header("wrapper.h")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks))
        .allowlist_function("aeron_.*")
        .clang_arg(format!("-I{}", includes.display()))
        .generate()?
        .write_to_file(out_dir.join("bindings.rs"))?;

    Ok(())
}
