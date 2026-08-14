//! Epistemic firewall over the ARKHE hypergraph zones (Z0–Z3).
//!
//! Rules:
//! - Z2 ↔ Z3 communication is only allowed through `TRANSLATES_TO_PRIMITIVE`.
//! - Z0 → any zone requires `CONSTRAINS_THEOREM` or `TRANSLATES_TO_PRIMITIVE`.
//! - All other edges are permitted.

use anyhow::{bail, Result};
use nostr_sdk::prelude::*;
use std::collections::HashSet;
use std::str::FromStr;

/// Zone names intentionally mirror the ARKHE design doc (`Z0_Theory` …).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Zone {
    Z0_Theory,
    Z1_Tools,
    Z2_Continuous,
    Z3_Discrete,
}

impl Zone {
    pub fn as_str(&self) -> &'static str {
        match self {
            Zone::Z0_Theory => "Z0_Theory",
            Zone::Z1_Tools => "Z1_Tools",
            Zone::Z2_Continuous => "Z2_Continuous",
            Zone::Z3_Discrete => "Z3_Discrete",
        }
    }
}

impl FromStr for Zone {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "Z0_Theory" => Ok(Zone::Z0_Theory),
            "Z1_Tools" => Ok(Zone::Z1_Tools),
            "Z2_Continuous" => Ok(Zone::Z2_Continuous),
            "Z3_Discrete" => Ok(Zone::Z3_Discrete),
            _ => bail!("unknown zone: {s}"),
        }
    }
}

/// Edge type marker used to satisfy the firewall.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeType {
    TranslatesToPrimitive,
    ConstrainsTheorem,
    MappedToControl,
    DependsOn,
}

impl EdgeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeType::TranslatesToPrimitive => "TRANSLATES_TO_PRIMITIVE",
            EdgeType::ConstrainsTheorem => "CONSTRAINS_THEOREM",
            EdgeType::MappedToControl => "MAPPED_TO_CONTROL",
            EdgeType::DependsOn => "DEPENDS_ON",
        }
    }
}

impl FromStr for EdgeType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "TRANSLATES_TO_PRIMITIVE" => Ok(EdgeType::TranslatesToPrimitive),
            "CONSTRAINS_THEOREM" => Ok(EdgeType::ConstrainsTheorem),
            "MAPPED_TO_CONTROL" => Ok(EdgeType::MappedToControl),
            "DEPENDS_ON" => Ok(EdgeType::DependsOn),
            _ => bail!("unknown edge type: {s}"),
        }
    }
}

/// Verify that an edge between two zones is permitted by the firewall.
///
/// Z2 ↔ Z3 edges require `TRANSLATES_TO_PRIMITIVE`. Z0-origin edges require
/// `CONSTRAINS_THEOREM` or `TRANSLATES_TO_PRIMITIVE`. Everything else is fine.
pub fn firewall_allows_edge(from: Zone, to: Zone, edge: EdgeType) -> bool {
    match (from, to) {
        (Zone::Z2_Continuous, Zone::Z3_Discrete)
        | (Zone::Z3_Discrete, Zone::Z2_Continuous) => {
            edge == EdgeType::TranslatesToPrimitive
        }
        (Zone::Z0_Theory, _) => {
            matches!(edge, EdgeType::ConstrainsTheorem | EdgeType::TranslatesToPrimitive)
        }
        _ => true,
    }
}

/// Validate a hyperedge over a set of nodes (by zone).
pub fn validate_hyperedge_firewall(
    nodes: &[(String, Zone)],
    edge_type: &str,
) -> Result<()> {
    let edge = EdgeType::from_str(edge_type)
        .map_err(|e| anyhow::anyhow!("unknown edge type: {edge_type}: {e}"))?;
    let zones: HashSet<Zone> = nodes.iter().map(|(_, z)| *z).collect();
    for &z1 in &zones {
        for &z2 in &zones {
            if z1 != z2 && !firewall_allows_edge(z1, z2, edge) {
                bail!(
                    "firewall violation: {:?} ↔ {:?} requires TRANSLATES_TO_PRIMITIVE, got {edge_type}",
                    z1, z2
                );
            }
        }
    }
    Ok(())
}

/// Validate a Nostr event against the firewall.
///
/// Security properties:
/// - The event signature is verified (T10) — an attacker cannot forge the
///   `source_zone`/`target_zone`/edge-type tags without the signing key.
/// - For `TRANSLATES_TO_PRIMITIVE` edges, the event must carry a
///   `translation_digest` tag equal to the SHA3-256 of the event content (T8),
///   cryptographically binding the translation claim to the payload.
pub fn validate_event_firewall(
    event: &Event,
    source_zone: Zone,
    event_kind: Kind,
) -> Result<()> {
    event.verify().map_err(|e| anyhow::anyhow!("event signature invalid: {e}"))?;

    let target_zone_str = event
        .tags()
        .iter()
        .find(|t| t.as_vec().first().map(|s| s.as_str()) == Some("target_zone"))
        .and_then(|t| t.as_vec().get(1).map(|s| s.to_string()))
        .unwrap_or_else(|| Zone::Z2_Continuous.as_str().to_string());

    let target_zone = Zone::from_str(&target_zone_str)
        .map_err(|e| anyhow::anyhow!("invalid target_zone: {target_zone_str}: {e}"))?;

    let edge_type_str = event
        .tags()
        .iter()
        .find(|t| t.as_vec().first().map(|s| s.as_str()) == Some("edge_type"))
        .and_then(|t| t.as_vec().get(1).map(|s| s.to_string()))
        .unwrap_or_else(|| EdgeType::DependsOn.as_str().to_string());

    let edge = EdgeType::from_str(&edge_type_str)
        .map_err(|e| anyhow::anyhow!("invalid edge_type: {edge_type_str}: {e}"))?;

    if event.kind() != event_kind {
        bail!("event kind mismatch");
    }

    if edge == EdgeType::TranslatesToPrimitive {
        let expected = sha3_256_hex(event.content().as_bytes());
        let claimed = event
            .tags()
            .iter()
            .find(|t| t.as_vec().first().map(|s| s.as_str()) == Some("translation_digest"))
            .and_then(|t| t.as_vec().get(1).map(|s| s.to_string()))
            .ok_or_else(|| anyhow::anyhow!("TRANSLATES_TO_PRIMITIVE requires translation_digest tag"))?;
        if claimed != expected {
            bail!("translation_digest does not match event content (firewall binding broken)");
        }
    }

    if !firewall_allows_edge(source_zone, target_zone, edge) {
        bail!(
            "firewall violation: {:?} → {:?} requires TRANSLATES_TO_PRIMITIVE, got {edge_type_str}",
            source_zone, target_zone
        );
    }
    Ok(())
}

fn sha3_256_hex(data: &[u8]) -> String {
    use sha3::{Digest, Sha3_256};
    hex::encode(Sha3_256::digest(data))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn z2_z3_requires_translation() {
        assert!(firewall_allows_edge(Zone::Z2_Continuous, Zone::Z3_Discrete, EdgeType::TranslatesToPrimitive));
        assert!(!firewall_allows_edge(Zone::Z2_Continuous, Zone::Z3_Discrete, EdgeType::DependsOn));
        assert!(!firewall_allows_edge(Zone::Z3_Discrete, Zone::Z2_Continuous, EdgeType::ConstrainsTheorem));
        assert!(firewall_allows_edge(Zone::Z3_Discrete, Zone::Z2_Continuous, EdgeType::TranslatesToPrimitive));
    }

    #[test]
    fn z0_requires_theorem_edge() {
        assert!(firewall_allows_edge(Zone::Z0_Theory, Zone::Z2_Continuous, EdgeType::ConstrainsTheorem));
        assert!(firewall_allows_edge(Zone::Z0_Theory, Zone::Z3_Discrete, EdgeType::ConstrainsTheorem));
        assert!(!firewall_allows_edge(Zone::Z0_Theory, Zone::Z3_Discrete, EdgeType::DependsOn));
    }

    #[test]
    fn hyperedge_validates() {
        let nodes = vec![
            ("z2".to_string(), Zone::Z2_Continuous),
            ("z3".to_string(), Zone::Z3_Discrete),
        ];
        assert!(validate_hyperedge_firewall(&nodes, "TRANSLATES_TO_PRIMITIVE").is_ok());
        assert!(validate_hyperedge_firewall(&nodes, "DEPENDS_ON").is_err());
    }
}
