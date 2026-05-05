//! In-memory collection of loaded IFC schema docs by version.
//! The backend owns one collection and uses it for both hover and datatype diagnostics.
//! Startup loading is best-effort: failures are recorded instead of panicking.

use std::collections::HashMap;

use crate::schema::loader::load_official_schema;
use crate::schema::{EntityDoc, IfcVersion, SchemaDoc};

#[derive(Debug, Default)]
pub struct SchemaDocCollection {
    pub docs: HashMap<IfcVersion, SchemaDoc>,
    load_errors: Vec<String>,
}

impl SchemaDocCollection {
    pub fn new() -> Self {
        let mut collection = Self::default();

        for version in IfcVersion::supported() {
            match load_official_schema(version) {
                Ok(schema) => {
                    collection.docs.insert(version, schema);
                }
                Err(error) => {
                    collection
                        .load_errors
                        .push(format!("{}: {}", version, error));
                }
            }
        }

        collection
    }

    #[cfg(test)]
    pub fn empty() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub fn from_docs(docs: impl IntoIterator<Item = (IfcVersion, SchemaDoc)>) -> Self {
        Self {
            docs: docs.into_iter().collect(),
            load_errors: Vec::new(),
        }
    }

    pub fn get(&self, version: IfcVersion) -> Option<&SchemaDoc> {
        self.docs.get(&version)
    }

    pub fn get_entity_doc(&self, version: IfcVersion, entity_name: &str) -> Option<&EntityDoc> {
        self.get(version)?.entity(entity_name)
    }

    pub fn load_errors(&self) -> &[String] {
        &self.load_errors
    }
}
