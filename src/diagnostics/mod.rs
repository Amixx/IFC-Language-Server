//! Diagnostic entry points.
//! Providers stay focused on one validation concern and operate on the parsed `Document` plus
//! runtime schema docs, without introducing extra service layers.

use tower_lsp::lsp_types::Diagnostic;

use crate::document::Document;
use crate::schema::SchemaDoc;

pub mod datatype;
pub mod syntax;

pub fn collect_with_schema_name(
    document: &Document,
    schema: &SchemaDoc,
    schema_name: Option<&str>,
) -> Vec<Diagnostic> {
    let mut diagnostics = syntax::collect(document);
    diagnostics.extend(datatype::collect_with_schema_name(
        document,
        schema,
        schema_name,
    ));
    diagnostics
}
