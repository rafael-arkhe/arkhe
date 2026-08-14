//! Contact Graph Routing (CGR) com janelas temporais e propagation_delay.
//!
//! # Correções aplicadas
//! - **P4 FIX**: propagation_delay (light-time) é componente dominante da latência
//!   em deep space. Não pode ser omitido.
//!
//! # Fórmula de arrival_time CGR
//! ```text
//! arrival_time = MAX(current_time, contact_start) + propagation_delay + transmission_time
//! propagation_delay = distance_ly * LIGHT_YEAR_SECONDS
//! transmission_time = bundle_size / data_rate
//! ```

use alloc::collections::BinaryHeap;
use alloc::string::String;
use alloc::vec::Vec;
use core::cmp::Ordering;

/// Ano-luz em segundos (velocidade da luz = 1 ly/ano).
pub const LIGHT_YEAR_SECONDS: f64 = 31_557_600.0;

/// Tipo de nó na rede.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeType {
    HotBubble,
    Wormhole,
    BlackHole,
    SupernovaRemnant,
    Gateway,
    Relay,
}

/// Nó da rede interestelar.
#[derive(Debug, Clone)]
pub struct NetworkNode {
    pub id: usize,
    pub name: String,
    pub node_type: NodeType,
    /// Posição em anos-luz (x, y, z).
    pub position_ly: (f64, f64, f64),
    /// Capacidade do buffer (bundles).
    pub buffer_cap: usize,
    /// Ocupação atual do buffer.
    pub buffer: usize,
    /// Nó falho/offline.
    pub failed: bool,
    /// Raio de Schwarzschild em metros (0 se não for BH).
    pub schwarzschild_radius_m: f64,
}

/// Janela de contato para CGR.
#[derive(Debug, Clone, Copy)]
pub struct ContactWindow {
    /// Início da janela (segundos desde epoch).
    pub start: f64,
    /// Fim da janela (segundos desde epoch).
    pub end: f64,
    /// Período de repetição (segundos). 0 = não repetitivo.
    pub period: f64,
}

impl ContactWindow {
    /// Verifica se o contato está ativo no tempo dado.
    pub fn is_active(&self, sim_time: f64) -> bool {
        if self.period > 0.0 {
            let t = sim_time % self.period;
            t >= self.start && t <= self.end
        } else {
            sim_time >= self.start && sim_time <= self.end
        }
    }
}

/// Aresta (link) da rede interestelar.
#[derive(Debug, Clone)]
pub struct NetworkEdge {
    pub from: usize,
    pub to: usize,
    /// Distância em anos-luz.
    pub distance_ly: f64,
    /// Taxa de dados (bytes/segundo).
    pub data_rate_bps: f64,
    /// Probabilidade de perda de pacote (0..1).
    pub packet_loss: f64,
    /// Janelas de contato disponíveis.
    pub contacts: Vec<ContactWindow>,
    /// Ativo no momento atual (computado a cada passo).
    pub active: bool,
}

impl NetworkEdge {
    /// Verifica se a aresta está em contato.
    pub fn is_in_contact(&self, sim_time: f64) -> bool {
        self.contacts.iter().any(|c| c.is_active(sim_time))
    }

    /// Calcula o propagation_delay (light-time) em segundos.
    ///
    /// **P4 FIX**: Este é o componente DOMINANTE da latência em deep space.
    pub fn propagation_delay_sec(&self) -> f64 {
        self.distance_ly * LIGHT_YEAR_SECONDS
    }

    /// Calcula o tempo de transmissão para um bundle de tamanho dado.
    pub fn transmission_time_sec(&self, bundle_size_bytes: usize) -> f64 {
        if self.data_rate_bps <= 0.0 {
            return f64::INFINITY;
        }
        bundle_size_bytes as f64 / self.data_rate_bps
    }

    /// Calcula o arrival_time CGR completo.
    ///
    /// # Fórmula
    /// ```text
    /// arrival = MAX(current_time, contact_start) + propagation_delay + transmission_time
    /// ```
    pub fn arrival_time(&self, current_time: f64, bundle_size_bytes: usize) -> Option<f64> {
        let contact = self.contacts.iter().find(|c| {
            let effective_start = if c.period > 0.0 {
                let cycles = (current_time / c.period).floor();
                c.start + cycles * c.period
            } else {
                c.start
            };
            effective_start >= current_time || (effective_start + (c.end - c.start)) >= current_time
        })?;

        let effective_start = if contact.period > 0.0 {
            let cycles = (current_time / contact.period).floor();
            let start = contact.start + cycles * contact.period;
            if start < current_time {
                contact.start + (cycles + 1.0) * contact.period
            } else {
                start
            }
        } else {
            contact.start
        };

        let departure = current_time.max(effective_start);
        let propagation = self.propagation_delay_sec();
        let transmission = self.transmission_time_sec(bundle_size_bytes);

        Some(departure + propagation + transmission)
    }
}

/// Estado para Dijkstra no CGR.
#[derive(Clone, Copy)]
struct RouteState {
    node: usize,
    cost: f64,
}

impl Eq for RouteState {}

impl PartialEq for RouteState {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl Ord for RouteState {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.partial_cmp(&self.cost).unwrap_or(Ordering::Equal)
    }
}

impl PartialOrd for RouteState {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Plano de contatos (conjunto de arestas com janelas temporais).
#[derive(Debug, Clone, Default)]
pub struct ContactPlan {
    pub nodes: Vec<NetworkNode>,
    pub edges: Vec<NetworkEdge>,
}

impl ContactPlan {
    /// Cria plano de contatos vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adiciona nó.
    pub fn add_node(&mut self, node: NetworkNode) {
        self.nodes.push(node);
    }

    /// Adiciona aresta.
    pub fn add_edge(&mut self, edge: NetworkEdge) {
        self.edges.push(edge);
    }

    /// Encontra o melhor caminho usando CGR-Dijkstra.
    pub fn find_path(
        &self,
        src: usize,
        dst: usize,
        current_time: f64,
        bundle_size: usize,
    ) -> Option<Vec<usize>> {
        let n = self.nodes.len();
        let mut dist = vec![f64::INFINITY; n];
        let mut prev = vec![usize::MAX; n];
        let mut heap = BinaryHeap::new();

        dist[src] = current_time;
        heap.push(RouteState { node: src, cost: current_time });

        while let Some(RouteState { node, cost }) = heap.pop() {
            if cost > dist[node] {
                continue;
            }
            if node == dst {
                break;
            }

            for edge in &self.edges {
                let other = if edge.from == node {
                    edge.to
                } else if edge.to == node {
                    edge.from
                } else {
                    continue;
                };

                if self.nodes[other].failed {
                    continue;
                }

                if !edge.is_in_contact(cost) {
                    continue;
                }

                let buf_penalty = if self.nodes[other].buffer >= self.nodes[other].buffer_cap {
                    continue;
                } else {
                    self.nodes[other].buffer as f64 / self.nodes[other].buffer_cap.max(1) as f64
                };

                let arrival = edge.arrival_time(cost, bundle_size)?;
                let new_cost = arrival + buf_penalty * 100.0;

                if new_cost < dist[other] {
                    dist[other] = new_cost;
                    prev[other] = node;
                    heap.push(RouteState { node: other, cost: new_cost });
                }
            }
        }

        let mut path = Vec::new();
        let mut cur = dst;
        while cur != usize::MAX && cur != src {
            path.push(cur);
            cur = prev[cur];
            if path.len() > n {
                return None;
            }
        }

        if cur == src {
            path.push(src);
            path.reverse();
            Some(path)
        } else {
            None
        }
    }

    /// Atualiza estado de atividade de todas as arestas.
    pub fn update_edge_states(&mut self, sim_time: f64) {
        for edge in &mut self.edges {
            edge.active = edge.is_in_contact(sim_time);
        }
    }

    /// Conta contatos ativos.
    pub fn active_contacts(&self) -> usize {
        self.edges.iter().filter(|e| e.active).count()
    }
}

/// Calcula distância euclidiana em anos-luz.
pub fn distance_ly(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    let dz = a.2 - b.2;
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Calcula fator de dilatação temporal gravitacional.
pub fn gravitational_time_dilation(schwarzschild_radius_m: f64, distance_m: f64) -> f64 {
    if schwarzschild_radius_m <= 0.0 || distance_m <= 0.0 {
        return 1.0;
    }
    let ratio = schwarzschild_radius_m / distance_m;
    if ratio >= 1.0 {
        return 0.0;
    }
    (1.0 - ratio).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cgr_basic() {
        let mut plan = ContactPlan::new();

        plan.add_node(NetworkNode {
            id: 0, name: "A".to_string(), node_type: NodeType::HotBubble,
            position_ly: (0.0, 0.0, 0.0), buffer_cap: 100, buffer: 0,
            failed: false, schwarzschild_radius_m: 0.0,
        });
        plan.add_node(NetworkNode {
            id: 1, name: "B".to_string(), node_type: NodeType::Relay,
            position_ly: (0.5, 0.0, 0.0), buffer_cap: 100, buffer: 0,
            failed: false, schwarzschild_radius_m: 0.0,
        });
        plan.add_node(NetworkNode {
            id: 2, name: "C".to_string(), node_type: NodeType::Gateway,
            position_ly: (1.0, 0.0, 0.0), buffer_cap: 100, buffer: 0,
            failed: false, schwarzschild_radius_m: 0.0,
        });

        plan.add_edge(NetworkEdge {
            from: 0, to: 1, distance_ly: 0.5,
            data_rate_bps: 1_000_000.0, packet_loss: 0.05,
            contacts: vec![ContactWindow { start: 0.0, end: 1e9, period: 0.0 }],
            active: false,
        });
        plan.add_edge(NetworkEdge {
            from: 1, to: 2, distance_ly: 0.5,
            data_rate_bps: 1_000_000.0, packet_loss: 0.05,
            contacts: vec![ContactWindow { start: 0.0, end: 1e9, period: 0.0 }],
            active: false,
        });

        let path = plan.find_path(0, 2, 0.0, 1000);
        assert!(path.is_some());
        let path = path.unwrap();
        assert_eq!(path, vec![0, 1, 2]);
    }

    #[test]
    fn test_cgr_avoids_failed_node() {
        let mut plan = ContactPlan::new();

        plan.add_node(NetworkNode {
            id: 0, name: "A".into(), node_type: NodeType::HotBubble,
            position_ly: (0.0, 0.0, 0.0), buffer_cap: 100, buffer: 0,
            failed: false, schwarzschild_radius_m: 0.0,
        });
        plan.add_node(NetworkNode {
            id: 1, name: "B".into(), node_type: NodeType::Relay,
            position_ly: (0.5, 0.0, 0.0), buffer_cap: 50, buffer: 0,
            failed: true, schwarzschild_radius_m: 0.0,
        });
        plan.add_node(NetworkNode {
            id: 2, name: "C".into(), node_type: NodeType::Gateway,
            position_ly: (1.0, 0.0, 0.0), buffer_cap: 100, buffer: 0,
            failed: false, schwarzschild_radius_m: 0.0,
        });

        // A -> C direto (sem passar por B falho)
        plan.add_edge(NetworkEdge {
            from: 0, to: 2, distance_ly: 1.0,
            data_rate_bps: 500_000.0, packet_loss: 0.1,
            contacts: vec![ContactWindow { start: 0.0, end: 1e9, period: 0.0 }],
            active: false,
        });
        plan.add_edge(NetworkEdge {
            from: 0, to: 1, distance_ly: 0.5,
            data_rate_bps: 1_000_000.0, packet_loss: 0.05,
            contacts: vec![ContactWindow { start: 0.0, end: 1e9, period: 0.0 }],
            active: false,
        });

        let path = plan.find_path(0, 2, 0.0, 1000);
        assert!(path.is_some());
        assert_eq!(path.unwrap(), vec![0, 2]); // Vai direto, evita B falho
    }

    #[test]
    fn test_buffer_full_drop() {
        let mut plan = ContactPlan::new();

        plan.add_node(NetworkNode {
            id: 0, name: "A".into(), node_type: NodeType::HotBubble,
            position_ly: (0.0, 0.0, 0.0), buffer_cap: 100, buffer: 0,
            failed: false, schwarzschild_radius_m: 0.0,
        });
        plan.add_node(NetworkNode {
            id: 1, name: "B".into(), node_type: NodeType::Relay,
            position_ly: (0.5, 0.0, 0.0), buffer_cap: 0, buffer: 0, // Buffer cheio!
            failed: false, schwarzschild_radius_m: 0.0,
        });

        plan.add_edge(NetworkEdge {
            from: 0, to: 1, distance_ly: 0.5,
            data_rate_bps: 1_000_000.0, packet_loss: 0.05,
            contacts: vec![ContactWindow { start: 0.0, end: 1e9, period: 0.0 }],
            active: false,
        });

        let path = plan.find_path(0, 1, 0.0, 1000);
        assert!(path.is_none()); // Não consegue chegar em B (buffer cheio)
    }

    #[test]
    fn test_gravitational_time_dilation() {
        let rs = 2.953e3; // Raio de Schwarzschild do Sol (m)
        let dilation = gravitational_time_dilation(rs, 1.5e11); // ~1 AU
        assert!(dilation < 1.0);
        assert!(dilation > 0.99); // A 1 AU, dilatação é mínima
    }
}
