//! Periodic 2D square lattice with 4-nearest-neighbour stencils.

/// Periodic `n x n` phase-field lattice.
pub struct Lattice {
    /// Side length (number of sites per dimension). Total size `= n*n`.
    pub n: usize,
    /// Neighbour indices for every site: `[up, down, left, right]`.
    pub nbr: Vec<[usize; 4]>,
}

impl Lattice {
    /// Build the periodic lattice of side `n` (at least 2).
    pub fn new(n: usize) -> Self {
        assert!(n >= 2, "lattice side must be >= 2");
        let nbr = (0..n * n)
            .map(|i| {
                let y = i / n;
                let x = i % n;
                let up = ((y + 1) % n) * n + x;
                let down = ((y + n - 1) % n) * n + x;
                let left = y * n + (x + n - 1) % n;
                let right = y * n + (x + 1) % n;
                [up, down, left, right]
            })
            .collect();
        Self { n, nbr }
    }

    /// Total number of sites.
    pub fn size(&self) -> usize {
        self.n * self.n
    }

    /// Periodic Manhattan (taxicab) distance between two flat indices.
    pub fn dist(&self, a: usize, b: usize) -> usize {
        let a_y = a / self.n;
        let a_x = a % self.n;
        let b_y = b / self.n;
        let b_x = b % self.n;
        let dy = (a_y as isize - b_y as isize).unsigned_abs().min(self.n / 2);
        let dx = (a_x as isize - b_x as isize).unsigned_abs().min(self.n / 2);
        dx + dy
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbours_are_symmetric() {
        let lat = Lattice::new(4);
        for i in 0..lat.size() {
            for &j in &lat.nbr[i] {
                assert!(
                    lat.nbr[j].contains(&i),
                    "neighbour relation must be symmetric: {i}->{j}"
                );
            }
        }
    }

    #[test]
    fn four_neighbours_per_site() {
        let lat = Lattice::new(5);
        assert!(lat.nbr.iter().all(|n| n.len() == 4));
    }
}