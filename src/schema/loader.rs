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
    Fetch { url: &'static str, message: String },
    Parse(LoadExpressError),
}

impl fmt::Display for SchemaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fetch { url, message } => {
                write!(f, "failed to fetch EXPRESS schema from {url}: {message}")
            }
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
    let url = version.express_url();
    let source = ureq::get(url)
        .call()
        .map_err(|error| SchemaLoadError::Fetch {
            url,
            message: error.to_string(),
        })?
        .body_mut()
        .read_to_string()
        .map_err(|error| SchemaLoadError::Fetch {
            url,
            message: error.to_string(),
        })?;

    load_express(version, &source).map_err(SchemaLoadError::Parse)
}
