mod generator;

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use generator::generate_schema_model;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let input_dir = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/express"));
    let output_dir = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data/schema-models"));

    fs::create_dir_all(&output_dir)?;

    for entry in fs::read_dir(&input_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !is_express_file(&path) {
            continue;
        }

        let source = fs::read_to_string(&path)?;
        let model = generate_schema_model(&source)?;
        let output_path = output_dir.join(output_name(&path)?);
        let json = serde_json::to_string_pretty(&model)?;
        fs::write(output_path, json)?;
    }

    Ok(())
}

fn is_express_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("exp"))
}

fn output_name(path: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .ok_or("input file name should have a valid stem")?;
    Ok(format!("{}_schema_model.json", stem.to_ascii_lowercase()))
}
