//! Validação topológica via Topological Data Analysis (TDA) para designs e motores.
//!
//! Módulo #3: filtração Vietoris–Rips sobre uma nuvem de pontos (ex.: geometria de
//! motor / laminação elétrica) com homologia persistente 0-dimensional por union-find,
//! números de Betti e persistência total. Valida conectividade, robustez e
//! consistência interna do diagrama.

use serde::{Deserialize, Serialize};

/// Ponto em n-dimensões com um identificador consistente.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub coords: Vec<f64>,
}

impl Point {
    pub fn new(coords: Vec<f64>) -> Self {
        Self { coords }
    }
}

fn dist(a: &Point, b: &Point) -> f64 {
    a.coords
        .iter()
        .zip(&b.coords)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

/// Uma aresta da filtração VR com sua distância (valor de filtração).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Edge {
    i: usize,
    j: usize,
    w: f64,
}

/// Persistência 0-dimensional: par (nascimento, morte) de um componente.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct PersistencePair {
    pub birth: f64,
    pub death: f64,
}

impl PersistencePair {
    pub fn lifetime(&self) -> f64 {
        self.death - self.birth
    }
}

/// Diagrama de persistência completo para análise.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistenceDiagram {
    pub pairs: Vec<PersistencePair>,
    pub total_persistence: f64,
}

/// Motor de TDA com filtração Vietoris–Rips.
#[derive(Debug, Clone)]
pub struct TdaEngine {
    points: Vec<Point>,
}

struct UnionFind {
    parent: Vec<usize>,
    size: Vec<usize>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self {
            parent: (0..n).collect(),
            size: vec![1; n],
        }
    }
    fn find(&mut self, x: usize) -> usize {
        if self.parent[x] != x {
            self.parent[x] = self.find(self.parent[x]);
        }
        self.parent[x]
    }
    fn union(&mut self, a: usize, b: usize) -> bool {
        let ra = self.find(a);
        let rb = self.find(b);
        if ra == rb {
            return false;
        }
        let (hi, lo) = if self.size[ra] >= self.size[rb] {
            (ra, rb)
        } else {
            (rb, ra)
        };
        self.parent[lo] = hi;
        self.size[hi] += self.size[lo];
        true
    }
}

impl TdaEngine {
    /// Constrói o motor a partir de uma nuvem de pontos (≥ 2 pontos).
    pub fn new(points: Vec<Point>) -> Self {
        assert!(points.len() >= 2, "TDA exige pelo menos 2 pontos");
        Self { points }
    }

    fn edges_sorted(&self) -> Vec<Edge> {
        let n = self.points.len();
        let mut edges: Vec<Edge> = Vec::with_capacity(n * (n - 1) / 2);
        for i in 0..n {
            for j in (i + 1)..n {
                edges.push(Edge {
                    i,
                    j,
                    w: dist(&self.points[i], &self.points[j]),
                });
            }
        }
        edges.sort_by(|a, b| a.w.total_cmp(&b.w));
        edges
    }

    /// Nº de Betti no raio `r`: Betti-0 = componentes, Betti-1 = ciclos independentes.
    pub fn betti(&self, r: f64) -> (usize, usize) {
        let n = self.points.len();
        let mut uf = UnionFind::new(n);
        let mut merges = 0;
        for e in self.edges_sorted() {
            if e.w > r {
                break;
            }
            if uf.union(e.i, e.j) {
                merges += 1;
            }
        }
        let components = n - merges;
        let edges_in = self.edges_sorted().iter().take_while(|e| e.w <= r).count();
        // Betti-1 = E - V + C (fórmula de Euler para complexo clique de grau 1).
        let cycles = edges_in + components - n;
        (components, cycles.max(0))
    }

    /// Diagrama de persistência 0-dimensional (nascimento/morte por union-find).
    pub fn persistence_0d(&self) -> PersistenceDiagram {
        let n = self.points.len();
        let mut uf = UnionFind::new(n);
        let mut birth: Vec<f64> = vec![0.0; n];
        let mut pairs = Vec::with_capacity(n);
        // Nascimentos: todo componente nasce no seu ponto (aparência do vértice em r=0).
        birth.fill(0.0);
        let mut alive: std::collections::HashSet<usize> = (0..n).collect();
        for e in self.edges_sorted() {
            let ra = uf.find(e.i);
            let rb = uf.find(e.j);
            if ra == rb {
                continue;
            }
            // Morte do componente mais jovem ao mergulhar.
            let born_ra = birth[ra];
            let born_rb = birth[rb];
            let (keep, die) = if born_ra.max(born_rb) == born_ra {
                (ra, rb)
            } else {
                (rb, ra)
            };
            pairs.push(PersistencePair {
                birth: born_ra.max(born_rb),
                death: e.w,
            });
            alive.remove(&die);
            uf.union(e.i, e.j);
            // Re-atualiza nascimento da raiz dominante.
            let root = uf.find(e.i);
            birth[root] = birth[keep];
            alive.insert(root);
        }
        // Componentes eternos: morte = infinito, representada como f64::INFINITY.
        for &c in alive.iter() {
            pairs.push(PersistencePair {
                birth: birth[c],
                death: f64::INFINITY,
            });
        }
        pairs.sort_by(|a, b| a.birth.total_cmp(&b.birth));
        let total_persistence = pairs.iter().map(|p| p.lifetime().clamp(0.0, 1e9)).sum();
        PersistenceDiagram {
            pairs,
            total_persistence,
        }
    }

    /// Valida a topologia: conectado em `r`, persistência total mínima e
    /// consistência interna do diagrama (persistência deve exceder a escala).
    pub fn is_valid(&self, r: f64) -> bool {
        let (b0, _) = self.betti(r);
        if b0 != 1 {
            return false; // desconexo no raio de trabalho → estrutura fraturada
        }
        let d = self.persistence_0d();
        if d.pairs.is_empty() {
            return false;
        }
        // Consistência interna: a persistência total (componentes não-eternos)
        // deve dominar a escala de ruído (10% do diâmetro).
        let finite: Vec<_> = d.pairs.iter().filter(|p| p.death.is_finite()).collect();
        let diag = self
            .edges_sorted()
            .last()
            .map(|e| e.w)
            .unwrap_or(0.0);
        let floor = 0.1 * diag;
        let total: f64 = finite.iter().map(|p| p.lifetime()).sum();
        total >= floor
    }
}

/// Validação de topologia de motor: fechada, conectada e sem componentes flutuantes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MotorTopology {
    pub points: usize,
    pub closed: bool,
    pub betti_0: usize,
    pub betti_1: usize,
    pub adequate_volume: bool,
}

impl MotorTopology {
    /// Valida uma topologia de motor dada nuvem de pontos da carcaça.
    pub fn validate(points: &[Point], working_radius: f64) -> Self {
        let engine = TdaEngine::new(points.to_vec());
        let (b0, b1) = engine.betti(working_radius);
        let closed = b0 == 1 && b1 >= 1; // uma componente com ao menos um ciclo
        Self {
            points: points.len(),
            closed,
            betti_0: b0,
            betti_1: b1,
            adequate_volume: points.len() >= 8,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(w: f64, n: usize) -> Vec<Point> {
        let mut pts = Vec::new();
        for iy in 0..n {
            for ix in 0..n {
                pts.push(Point::new(vec![ix as f64 * w, iy as f64 * w]));
            }
        }
        pts
    }

    #[test]
    fn grid_is_connected_and_valid() {
        let pts = grid(1.0, 6);
        let engine = TdaEngine::new(pts.clone());
        let (b0, b1) = engine.betti(1.2);
        assert_eq!(b0, 1, "grade conectada deve ter 1 componente");
        assert!(b1 >= 1, "grade 2D deve ter ciclos (Betti-1 ≥ 1), obteve {b1}");
        assert!(engine.is_valid(1.2));
    }

    #[test]
    fn persistence_diagram_is_monotonic() {
        let pts = grid(1.0, 5);
        let d = TdaEngine::new(pts).persistence_0d();
        assert!(d.pairs.iter().all(|p| p.death.is_infinite() || p.death >= p.birth));
        assert_eq!(d.pairs.iter().filter(|p| p.death.is_infinite()).count(), 1);
    }

    #[test]
    fn disconnected_points_fail_validation() {
        // Dois aglomerados distantes → falha no raio pequeno.
        let mut pts = grid(1.0, 3);
        pts.extend(grid(1.0, 3).into_iter().map(|mut p| {
            p.coords[0] += 100.0;
            p
        }));
        let engine = TdaEngine::new(pts);
        assert!(!engine.is_valid(1.5), "dois aglomerados no raio curto deve falhar");
    }

    #[test]
    fn motor_topology_detects_open_structure() {
        // Linha aberta: Betti-1 = 0.
        let pts: Vec<Point> = (0..10).map(|i| Point::new(vec![i as f64, 0.0])).collect();
        let mt = MotorTopology::validate(&pts, 1.2);
        assert!(!mt.closed, "linha aberta não forma malha fechada");
        let flat = grid(1.0, 5);
        let mt2 = MotorTopology::validate(&flat, 1.2);
        assert!(mt2.closed, "grade plana com loops deve contar como fechada ao raio certo");
    }
}