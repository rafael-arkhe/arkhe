//! Thin `#[wasm_bindgen]` re-export shim.
//!
//! This module is only compiled with `--features wasm`. It hands the JSON-string
//! functions from [`crate::wasm`] to JavaScript verbatim, forwarding numeric
//! handles unchanged. All real logic lives behind the JSON facade; this file
//! contains no logic of its own, no `unsafe`, and no `extern` — the only added
//! `Complex` is that it starts the panic hook on load.

use wasm_bindgen::prelude::*;

/// Installs the panic hook once so Rust panics surface in the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

// Lifecycle
#[wasm_bindgen(js_name = hg_new)]
pub fn hg_new() -> u32 {
    crate::wasm::hg_new()
}

#[wasm_bindgen(js_name = hg_new_default)]
pub fn hg_new_default() -> u32 {
    crate::wasm::hg_new_default()
}

#[wasm_bindgen(js_name = hg_free)]
pub fn hg_free(handle: u32) {
    crate::wasm::hg_free(handle)
}

// Agents
#[wasm_bindgen(js_name = hg_add_agent)]
pub fn hg_add_agent(handle: u32, specialty: &str) -> String {
    crate::wasm::hg_add_agent(handle, specialty)
}

#[wasm_bindgen(js_name = hg_list_agents)]
pub fn hg_list_agents(handle: u32) -> String {
    crate::wasm::hg_list_agents(handle)
}

// Edges
#[wasm_bindgen(js_name = hg_spawn_edge)]
pub fn hg_spawn_edge(handle: u32, name: &str, edge_type: &str) -> String {
    crate::wasm::hg_spawn_edge(handle, name, edge_type)
}

#[wasm_bindgen(js_name = hg_list_edges)]
pub fn hg_list_edges(handle: u32) -> String {
    crate::wasm::hg_list_edges(handle)
}

#[wasm_bindgen(js_name = hg_sign)]
pub fn hg_sign(handle: u32, edge_id: &str, by_specialty: &str) -> String {
    crate::wasm::hg_sign(handle, edge_id, by_specialty)
}

#[wasm_bindgen(js_name = hg_check_consensus)]
pub fn hg_check_consensus(handle: u32, edge_id: &str) -> String {
    crate::wasm::hg_check_consensus(handle, edge_id)
}

#[wasm_bindgen(js_name = hg_transition)]
pub fn hg_transition(
    handle: u32,
    edge_id: &str,
    new_state: &str,
    by_specialty: &str,
) -> String {
    crate::wasm::hg_transition(handle, edge_id, new_state, by_specialty)
}

#[wasm_bindgen(js_name = hg_veto_edge)]
pub fn hg_veto_edge(handle: u32, edge_id: &str, reason: &str, by_specialty: &str) -> String {
    crate::wasm::hg_veto_edge(handle, edge_id, reason, by_specialty)
}

#[wasm_bindgen(js_name = hg_stats)]
pub fn hg_stats(handle: u32) -> String {
    crate::wasm::hg_stats(handle)
}

#[wasm_bindgen(js_name = wasm_version)]
pub fn wasm_version() -> String {
    concat!("0.8.0-wasm (", env!("CARGO_PKG_VERSION"), ")").to_string()
}