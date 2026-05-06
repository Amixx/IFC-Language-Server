//! Supported IFC schema versions and version-specific metadata.
//! These mappings provide schema names and hover documentation links for the bundled official
//! EXPRESS definitions.

use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum IfcVersion {
    Ifc2x3Tc1,
    Ifc4Add2Tc1,
    Ifc4x3Add2,
}

impl IfcVersion {
    pub fn supported() -> [Self; 3] {
        [Self::Ifc2x3Tc1, Self::Ifc4Add2Tc1, Self::Ifc4x3Add2]
    }

    pub fn from_schema_name(schema_name: &str) -> Option<Self> {
        match schema_name {
            "IFC2X3" => Some(Self::Ifc2x3Tc1),
            "IFC4" => Some(Self::Ifc4Add2Tc1),
            "IFC4X3_ADD2" => Some(Self::Ifc4x3Add2),
            _ => None,
        }
    }

    pub fn schema_name(self) -> &'static str {
        match self {
            Self::Ifc2x3Tc1 => "IFC2X3",
            Self::Ifc4Add2Tc1 => "IFC4",
            Self::Ifc4x3Add2 => "IFC4X3_ADD2",
        }
    }

    pub(crate) fn documentation_url(self, entity_name: &str) -> String {
        match self {
            Self::Ifc2x3Tc1 => {
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC2x3/TC1/HTML/alphabeticalorder_entities.htm".to_string()
            }
            Self::Ifc4Add2Tc1 => format!(
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC4/ADD2_TC1/HTML/link/{}.htm",
                entity_name.to_ascii_lowercase()
            ),
            Self::Ifc4x3Add2 => format!(
                "https://standards.buildingsmart.org/IFC/RELEASE/IFC4_3/HTML/lexical/{}.htm",
                entity_name
            ),
        }
    }
}

impl fmt::Display for IfcVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.schema_name())
    }
}
