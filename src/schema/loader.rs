//! Loading boundary for official and custom EXPRESS schema text.
//! `load_express` converts EXPRESS text into a resolved `SchemaDoc`; the official schemas are
//! bundled into the binary at compile time and parsed from those embedded strings at startup.

use std::error::Error;
use std::fmt;

use crate::schema::parser::parse_express_source;
use crate::schema::resolver::resolve_schema;
use crate::schema::{IfcVersion, SchemaDoc};

#[derive(Debug)]
pub enum LoadExpressError {
    Parse(String),
    MissingSchema,
}

impl fmt::Display for LoadExpressError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(message) => write!(f, "failed to parse EXPRESS schema: {message}"),
            Self::MissingSchema => write!(f, "EXPRESS input does not contain a schema"),
        }
    }
}

impl Error for LoadExpressError {}

#[derive(Debug)]
pub(crate) enum SchemaLoadError {
    Parse(LoadExpressError),
}

impl fmt::Display for SchemaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(error) => write!(f, "{error}"),
        }
    }
}

impl Error for SchemaLoadError {}

pub fn load_express(version: IfcVersion, source: &str) -> Result<SchemaDoc, LoadExpressError> {
    let raw = parse_express_source(source)?;
    Ok(resolve_schema(version, raw))
}

pub(crate) fn load_official_schema(version: IfcVersion) -> Result<SchemaDoc, SchemaLoadError> {
    load_express(version, official_express_source(version)).map_err(SchemaLoadError::Parse)
}

fn official_express_source(version: IfcVersion) -> &'static str {
    match version {
        IfcVersion::Ifc2x3Tc1 => include_str!(concat!(env!("OUT_DIR"), "/express/ifc2x3_tc1.exp")),
        IfcVersion::Ifc4Add2Tc1 => {
            include_str!(concat!(env!("OUT_DIR"), "/express/ifc4_add2_tc1.exp"))
        }
        IfcVersion::Ifc4x3Add2 => {
            include_str!(concat!(env!("OUT_DIR"), "/express/ifc4x3_add2.exp"))
        }
    }
}
