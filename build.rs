use std::env;
use std::error::Error;
use std::fs;
use std::path::PathBuf;

use sha2::{Digest, Sha256};

const SCHEMAS: &[(&str, &str, &str)] = &[
    (
        "ifc2x3_tc1.exp",
        "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/EXPRESS/IFC2X3_TC1.exp",
        "e18a1b2c3e29f5256904c83378ccad0850f52287a8d0122d149aba4a417fe5e5",
    ),
    (
        "ifc4_add2_tc1.exp",
        "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/EXPRESS/IFC4.exp",
        "a2704ba20a1b3d0b7d9b61d6fd37d0baa3b4996ba3e90d968a1d2ca2819d1046",
    ),
    (
        "ifc4x3_add2.exp",
        "https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/IFC4X3_ADD2.exp",
        "f67c8762b13a099c28082061e6f16b9ef1284ceec34069792afc702725675860",
    ),
];

const IFC2X3_ENTITY_INDEX_URL: &str =
    "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/alphabeticalorder_entities.htm";
const IFC2X3_ENTITY_COUNT: usize = 653;

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo::rerun-if-changed=build.rs");

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let express_dir = out_dir.join("express");
    fs::create_dir_all(&express_dir)?;

    for (file_name, url, expected_sha256) in SCHEMAS {
        let source = ureq::get(*url).call()?.body_mut().read_to_vec()?;
        let actual_sha256 = format!("{:x}", Sha256::digest(&source));

        if actual_sha256 != *expected_sha256 {
            return Err(format!(
                "checksum mismatch for {}: expected {}, got {}",
                file_name, expected_sha256, actual_sha256
            )
            .into());
        }

        fs::write(express_dir.join(file_name), source)?;
    }

    let entity_index = ureq::get(IFC2X3_ENTITY_INDEX_URL)
        .call()?
        .body_mut()
        .read_to_vec()?;

    let entity_index = String::from_utf8(entity_index)?;
    let links = parse_ifc2x3_entity_links(&entity_index)?;
    fs::write(
        out_dir.join("ifc2x3_entity_doc_links.rs"),
        render_ifc2x3_entity_links(&links),
    )?;

    Ok(())
}

fn parse_ifc2x3_entity_links(html: &str) -> Result<Vec<(String, String)>, Box<dyn Error>> {
    let mut links = Vec::new();

    for line in html.lines() {
        let Some((_, after_prefix)) = line.split_once("<A HREF=\"") else {
            continue;
        };
        let Some((href, after_href)) = after_prefix.split_once('"') else {
            return Err(format!("malformed IFC2x3 entity link: {line}").into());
        };
        let Some((_, after_tag)) = after_href.split_once('>') else {
            return Err(format!("malformed IFC2x3 entity link: {line}").into());
        };
        let Some((entity_name, _)) = after_tag.split_once("</A>") else {
            return Err(format!("malformed IFC2x3 entity link: {line}").into());
        };

        if entity_name.starts_with("Ifc") && href.ends_with(".htm") {
            links.push((entity_name.to_ascii_uppercase(), href.to_string()));
        }
    }

    if links.len() != IFC2X3_ENTITY_COUNT {
        return Err(format!(
            "expected {IFC2X3_ENTITY_COUNT} IFC2x3 entity links, found {}",
            links.len()
        )
        .into());
    }

    Ok(links)
}

fn render_ifc2x3_entity_links(links: &[(String, String)]) -> String {
    let mut output = String::from("const IFC2X3_ENTITY_DOC_LINKS: &[(&str, &str)] = &[\n");

    for (entity_name, href) in links {
        output.push_str(&format!("    (\"{entity_name}\", \"{href}\"),\n"));
    }

    output.push_str("];\n");
    output
}
