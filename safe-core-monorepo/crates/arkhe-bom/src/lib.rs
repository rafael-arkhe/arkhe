#![warn(missing_docs)]

//! Bill of Materials — geração e verificação de CycloneDX ML-BOM.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Component {
    pub name: String,
    pub version: String,
    pub component_type: ComponentType,
    pub purl: Option<String>,
    pub hashes: Vec<HashEntry>,
    pub licenses: Vec<LicenseEntry>,
    pub properties: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ComponentType { Library, Model, Dataset, Container, Firmware }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HashEntry {
    pub alg: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LicenseEntry {
    pub id: Option<String>,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bom {
    pub bom_format: String,
    pub spec_version: String,
    pub serial_number: String,
    pub version: u32,
    pub components: Vec<Component>,
    pub metadata: BomMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BomMetadata {
    pub timestamp: String,
    pub tools: Vec<ToolEntry>,
    pub component: Option<Component>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub version: Option<String>,
}

impl Bom {
    pub fn new() -> Self {
        Self {
            bom_format: "CycloneDX".into(),
            spec_version: "1.6".into(),
            serial_number: format!("urn:uuid:{}", uuid::Uuid::new_v4()),
            version: 1,
            components: Vec::new(),
            metadata: BomMetadata {
                timestamp: chrono::Utc::now().to_rfc3339(),
                tools: vec![ToolEntry { name: "arkhe-bom".into(), version: Some(env!("CARGO_PKG_VERSION").into()) }],
                component: None,
            },
        }
    }

    pub fn add_component(&mut self, component: Component) {
        self.components.push(component);
    }

    pub fn to_json(&self) -> serde_json::Result<String> {
        serde_json::to_string_pretty(&serde_json::json!({
            "bomFormat": self.bom_format,
            "specVersion": self.spec_version,
            "serialNumber": self.serial_number,
            "version": self.version,
            "metadata": self.metadata,
            "components": self.components,
        }))
    }

    pub fn verify_integrity(&self) -> Vec<String> {
        let mut issues = Vec::new();
        for comp in &self.components {
            if comp.hashes.is_empty() {
                issues.push(format!("Component '{}' has no integrity hashes", comp.name));
            }
        }
        issues
    }
}

impl Default for Bom { fn default() -> Self { Self::new() } }

#[cfg(test)]
mod tests {
    use super::*;

    fn comp(name: &str, hashes: Vec<HashEntry>) -> Component {
        Component {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            component_type: ComponentType::Model,
            purl: None,
            hashes,
            licenses: Vec::new(),
            properties: HashMap::new(),
        }
    }

    #[test]
    fn new_bom_is_cyclonedx_and_empty() {
        let b = Bom::new();
        assert_eq!(b.bom_format, "CycloneDX");
        assert_eq!(b.spec_version, "1.6");
        assert!(b.serial_number.starts_with("urn:uuid:"));
        assert!(b.components.is_empty());
        assert_eq!(b.metadata.tools.len(), 1);
    }

    #[test]
    fn verify_integrity_flags_components_without_hashes() {
        let mut b = Bom::new();
        b.add_component(comp("no-hash-model", vec![]));
        b.add_component(comp(
            "hashed-model",
            vec![HashEntry { alg: "BLAKE3".into(), content: "abc123".into() }],
        ));
        let issues = b.verify_integrity();
        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("no-hash-model"));
    }

    #[test]
    fn to_json_serializes_expected_shape() {
        let mut b = Bom::new();
        b.add_component(comp("m", vec![HashEntry { alg: "BLAKE3".into(), content: "x".into() }]));
        let json = b.to_json().expect("serializes");
        assert!(json.contains("\"bomFormat\""));
        assert!(json.contains("CycloneDX"));
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["components"].as_array().unwrap().len(), 1);
    }
}
