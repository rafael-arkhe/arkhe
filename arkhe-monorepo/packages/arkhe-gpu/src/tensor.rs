//! Tensores da camada GPU (bloco 1011, v390.2).
//!
//! Superfície mínima real: um id opaco + forma. O conteúdo fica no backend —
//! a paridade com o núcleo Lean (I536–I538) não depende desta dimensão.

/// Id opaco de um tensor alocado numa instância de backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TensorId(pub(crate) u64);

impl TensorId {
    /// Valor cru do id (o backend garante unicidade na sua instância).
    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

/// Forma N-dimensional de um tensor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shape {
    dims: Vec<usize>,
}

impl Shape {
    /// Forma a partir das dimensões (não-vazia).
    #[must_use]
    pub fn new(dims: Vec<usize>) -> Self {
        Self { dims }
    }

    /// Dimensões da forma.
    #[must_use]
    pub fn dims(&self) -> &[usize] {
        &self.dims
    }

    /// N.º total de elementos (`∏ dims`); `0` para forma vazia.
    #[must_use]
    pub fn numel(&self) -> usize {
        self.dims.iter().product()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shape_numel_and_ids() {
        let s = Shape::new(vec![2, 4, 8]);
        assert_eq!(s.numel(), 64);
        assert_eq!(Shape::new(vec![1024]).numel(), 1024);
        assert_eq!(TensorId(7).raw(), 7);
        assert_eq!(TensorId(7), TensorId(7));
        assert_ne!(TensorId(7), TensorId(8));
    }
}