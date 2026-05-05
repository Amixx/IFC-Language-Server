//! Build-time bundling of official IFC EXPRESS definitions.
//! The three supported schemas are downloaded during compilation and written into `OUT_DIR` so the
//! runtime can include them in the binary without storing EXPRESS files in the repository.

use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

const SCHEMAS: &[(&str, &str)] = &[
    (
        "ifc2x3_tc1.exp",
        "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/EXPRESS/IFC2X3_TC1.exp",
    ),
    (
        "ifc4_add2_tc1.exp",
        "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/EXPRESS/IFC4.exp",
    ),
    (
        "ifc4x3_add2.exp",
        "https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/IFC4X3_ADD2.exp",
    ),
];

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?).join("express");
    fs::create_dir_all(&out_dir)?;

    for (file_name, url) in SCHEMAS {
        let source = ureq::get(*url).call()?.body_mut().read_to_string()?;
        fs::write(out_dir.join(file_name), source)?;
    }

    Ok(())
}
