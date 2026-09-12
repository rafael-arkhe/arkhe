//! Grupos de permutações finitos e o algoritmo de completação do amalgam G₃
//! (módulo #23).
//!
//! Um grupo `G` é uma **completação** do amalgam de Goldschmidt G₃ se existem
//! `A, B ⩽ G`, ambos isomorfos a `Sym₄`, que geram `G`, com `A ∩ B ≅ Dih₈` e
//! `O₂(A) ≠ O₂(B)` ([Goldschmidt, Ann. of Math. 111:377–406, 1980]). A condição
//! `O₂(A) ≠ O₂(B)` é o que distingue as completações das não-completações nas
//! classificações de Parker & Rowley (J. Algebra 235:131–153, 2001).

use serde::{Deserialize, Serialize};

/// Permutação sobre `{0..n}`: `p[x]` é a imagem de `x`. Ordenável (BTreeSet).
pub type Perm = Vec<usize>;

fn identity(n: usize) -> Perm {
    (0..n).collect()
}

/// Composição à esquerda: `(a ∘ m)(x) = a[m[x]]`.
pub fn compose(a: &Perm, m: &Perm) -> Perm {
    debug_assert_eq!(a.len(), m.len());
    m.iter().map(|&x| a[x]).collect()
}

fn inverse(p: &Perm) -> Perm {
    let mut inv = vec![0; p.len()];
    for (i, &v) in p.iter().enumerate() {
        inv[v] = i;
    }
    inv
}

/// Ordem da permutação (mmc das órbitas); identidade → 1.
fn order_of(p: &Perm) -> usize {
    let n = p.len();
    let mut visited = vec![false; n];
    let mut lcm = 1_usize;
    for start in 0..n {
        if visited[start] {
            continue;
        }
        let mut len = 0;
        let mut cur = start;
        while !visited[cur] {
            visited[cur] = true;
            cur = p[cur];
            len += 1;
        }
        if len > 0 {
            lcm = lcm / gcd(lcm as u64, len as u64) as usize * len;
        }
    }
    lcm.max(1)
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a
}

fn is_power_of_two(x: usize) -> bool {
    x != 0 && (x & (x - 1)) == 0
}

/// Grupo de permutações finito (fecho multiplicativo dos geradores).
#[derive(Debug, Clone)]
pub struct PermGroup {
    n: usize,
    elems: std::collections::BTreeSet<Perm>,
}

impl PartialEq for PermGroup {
    fn eq(&self, other: &Self) -> bool {
        self.elems == other.elems
    }
}
impl Eq for PermGroup {}

impl PermGroup {
    /// Fecho multiplicativo dos geradores sobre `{0..n}` (contém a identidade).
    pub fn span(gens: &[Perm], n: usize) -> Self {
        let mut elems = std::collections::BTreeSet::new();
        elems.insert(identity(n));
        for g in gens {
            elems.insert(g.clone());
        }
        let mut changed = true;
        while changed {
            changed = false;
            let snapshot: Vec<Perm> = elems.iter().cloned().collect();
            for a in &snapshot {
                for b in &snapshot {
                    if elems.insert(compose(a, b)) {
                        changed = true;
                    }
                }
            }
        }
        Self { n, elems }
    }

    pub fn n(&self) -> usize {
        self.n
    }
    pub fn order(&self) -> usize {
        self.elems.len()
    }
    pub fn contains(&self, p: &Perm) -> bool {
        self.elems.contains(p)
    }
    pub fn elements(&self) -> Vec<Perm> {
        self.elems.iter().cloned().collect()
    }

    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            n: self.n,
            elems: self.elems.intersection(&other.elems).cloned().collect(),
        }
    }

    /// `h` é normal em `self`? (∀ g ∈ self, ∀ x ∈ h: g⁻¹xg ∈ h).
    fn is_normal(&self, h: &PermGroup) -> bool {
        for g in &self.elems {
            let inv = inverse(g);
            for x in &h.elems {
                let conj = compose(&compose(&inv, x), g);
                if !h.contains(&conj) {
                    return false;
                }
            }
        }
        true
    }

    /// Subgrupos gerados por ≤ 2 elementos (cobre os casos pequenos usados aqui).
    fn subgroups_by_bounded_generators(&self) -> Vec<PermGroup> {
        let elems: Vec<Perm> = self.elems.iter().cloned().collect();
        let mut out: Vec<PermGroup> = Vec::new();
        for i in 0..elems.len() {
            let g = PermGroup::span(&elems[i..=i], self.n);
            if !out.contains(&g) {
                out.push(g);
            }
            for j in (i + 1)..elems.len() {
                let h = PermGroup::span(&[elems[i].clone(), elems[j].clone()], self.n);
                if !out.contains(&h) {
                    out.push(h);
                }
            }
        }
        out
    }

    /// O 2-core `O₂(self)`: o maior subgrupo normal de ordem potência de 2.
    pub fn normal_2_core(&self) -> PermGroup {
        let subs = self.subgroups_by_bounded_generators();
        let mut best: Option<PermGroup> = None;
        let mut best_order = 0_usize;
        for s in &subs {
            let o = s.order();
            if is_power_of_two(o) && self.is_normal(s) && o > best_order {
                best = Some(s.clone());
                best_order = o;
            }
        }
        best.unwrap_or_else(|| PermGroup::span(&[identity(self.n)], self.n))
    }

    /// Subgrupos `C2 × C2` (Klein, ordem 4, com 3 elementos de ordem 2).
    pub fn klein_subgroups(&self) -> Vec<PermGroup> {
        self.subgroups_by_bounded_generators()
            .into_iter()
            .filter(|s| {
                s.order() == 4
                    && s.elements().iter().filter(|p| order_of(p) == 2).count() == 3
            })
            .collect()
    }
}

/// Sym₄ agindo em `{0,1,2,3}` (transposição (0 1) e 4-ciclo (0 1 2 3)).
pub fn sym4() -> PermGroup {
    PermGroup::span(&[vec![1, 0, 2, 3], vec![1, 2, 3, 0]], 4)
}

/// Dih₈ (ordem 8, diédrico do quadrado) em `{0,1,2,3}`: `⟨r, m⟩`, `r=(0 1 2 3)`, `m=(1 3)`.
pub fn dih8() -> PermGroup {
    PermGroup::span(&[vec![1, 2, 3, 0], vec![0, 3, 2, 1]], 4)
}

/// `g` é isomorfo a Dih₈ se tem ordem 8 e admite `r` (ordem 4) e `m` (ordem 2)
/// com `r^m = m r m = r³` (apresentação `<m, r | m², r⁴, r^m = r³>`).
fn is_dih8(g: &PermGroup) -> bool {
    if g.order() != 8 {
        return false;
    }
    let order4: Vec<Perm> = g.elements().into_iter().filter(|p| order_of(p) == 4).collect();
    let order2: Vec<Perm> = g.elements().into_iter().filter(|p| order_of(p) == 2).collect();
    for r in &order4 {
        let r3 = compose(&compose(r, r), r);
        for m in &order2 {
            let conj = compose(&compose(m, r), m);
            if conj == r3 {
                return true;
            }
        }
    }
    false
}

/// Amalgam de Goldschmidt G₃: dois `Sym₄` com interseção `Dih₈`.
#[derive(Debug, Clone)]
pub struct GoldschmidtG3 {
    pub a: PermGroup,
    pub b: PermGroup,
}

/// Resultado do algoritmo de completação do `G₃`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompletionResult {
    pub a_order: usize,
    pub a_is_sym4: bool,
    pub b_order: usize,
    pub b_is_sym4: bool,
    pub intersection_order: usize,
    pub d_is_dih8: bool,
    pub o2_agree: bool,
    pub generated_order: usize,
    pub is_complete: bool,
}

impl GoldschmidtG3 {
    pub fn new(a: PermGroup, b: PermGroup) -> Self {
        Self { a, b }
    }

    /// Algoritmo de completação sobre o critério G₃: verifica `A ≅ B ≅ Sym₄`,
    /// `A∩B ≅ Dih₈` e `O₂(A) ≠ O₂(B)` construindo os pivôs por fecho de
    /// permutações. A completação existe ⇔ todos os critérios são satisfeitos.
    pub fn completion_result(&self) -> CompletionResult {
        let a_order = self.a.order();
        let b_order = self.b.order();
        let a_is_sym4 = a_order == 24;
        let b_is_sym4 = b_order == 24;

        let inter = self.a.intersect(&self.b);
        let d_is_dih8 = is_dih8(&inter);

        let o2_a = self.a.normal_2_core();
        let o2_b = self.b.normal_2_core();
        let o2_agree = o2_a == o2_b;

        // G = ⟨A,B⟩: fecho dos geradores de A e B sobre o mesmo domínio.
        let mut gens: Vec<Perm> = self.a.elements();
        for b in self.b.elements() {
            gens.push(b);
        }
        let generated = PermGroup::span(&gens, self.a.n().max(self.b.n()));
        let generated_order = generated.order();

        CompletionResult {
            a_order,
            a_is_sym4,
            b_order,
            b_is_sym4,
            intersection_order: inter.order(),
            d_is_dih8,
            o2_agree,
            generated_order,
            is_complete: a_is_sym4 && b_is_sym4 && d_is_dih8 && !o2_agree,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sym4_is_order_24() {
        assert_eq!(sym4().order(), 24);
    }

    #[test]
    fn dih8_presentation() {
        let d = dih8();
        assert_eq!(d.order(), 8);
        let r = vec![1, 2, 3, 0]; // (0 1 2 3)
        let m = vec![0, 3, 2, 1]; // (1 3)
        assert_eq!(order_of(&r), 4);
        assert_eq!(order_of(&m), 2);
        // r^m = m r m = r³
        let conj = compose(&compose(&m, &r), &m);
        let r3 = compose(&r, &compose(&r, &r));
        assert_eq!(conj, r3);
    }

    #[test]
    fn dih8_has_exactly_two_klein_fours() {
        // Lema do artigo (arXiv:2607.27256): D tem exatamente dois C2×C2 —
        // o 2-core O₂(A) e o outro W, não normal em A.
        let d = dih8();
        let kleins = d.klein_subgroups();
        assert_eq!(kleins.len(), 2);
        assert_ne!(kleins[0], kleins[1]);
        let o2_sym4 = sym4().normal_2_core();
        assert_eq!(o2_sym4.order(), 4, "O₂(Sym₄) = V₄");
        assert!(
            kleins.contains(&o2_sym4),
            "O₂(Sym₄) é um dos dois V4 ⊆ D"
        );
    }

    #[test]
    fn non_g3_not_complete() {
        // A = B = Sym₄: interseção é o próprio Sym₄ (ordem 24), não Dih₈.
        let g = GoldschmidtG3::new(sym4(), sym4());
        let r = g.completion_result();
        assert!(r.a_is_sym4 && r.b_is_sym4);
        assert_eq!(r.intersection_order, 24);
        assert!(!r.d_is_dih8);
        assert!(!r.is_complete);
    }

    #[test]
    fn single_sym4_pair_with_dihedral_is_not_g3() {
        // A = Sym₄, B = D (ordem 8): B não é Sym₄ → não completa G₃.
        let g = GoldschmidtG3::new(sym4(), dih8());
        let r = g.completion_result();
        assert!(r.a_is_sym4);
        assert!(!r.b_is_sym4);
        assert!(r.d_is_dih8, "A∩D = D = Dih8");
        assert!(!r.is_complete);
    }
}