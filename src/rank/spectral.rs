//! Katz's index and the spectral structure of the adjacency matrix.
//!
//! The graph is given by successor lists, and *A* is its adjacency matrix:
//! *a*ᵢⱼ = 1 if and only if there is an arc *i* → *j*. Scores are row
//! vectors, oriented by incoming paths.

use super::{lu, sccs};

/// The relative tolerance within which the spectral radius of a class is
/// considered equal to ρ.
const BASIC_TOLERANCE: f64 = 1E-9;

/// The relative gap between the Collatz–Wielandt bounds at which the power
/// iteration stops.
const PERRON_TOLERANCE: f64 = 1E-13;

/// The maximum number of steps of the power iteration.
const PERRON_ITERATIONS: usize = 1_000_000;

/// Returns Katz's index of a graph given by successor lists, normalized.
///
/// This is κ(β) = *k*(β) / ‖*k*(β)‖₁, where *k*(β) = **1** Σ_{*t* ≥ 0}
/// β^*t* *A*^*t* = **1** (*I* − β*A*)⁻¹. In particular, κ(0) is uniform.
///
/// The series converges, and the result is meaningful, only for 0 ≤ β <
/// 1/ρ, where ρ is the [spectral radius](Spectrum::rho) of *A*; when ρ = 0,
/// *A* is nilpotent and every β ≥ 0 is allowed.
///
/// # Implementation Notes
///
/// The vector *k*(β) is the solution of the dense system (*I* − β*A*ᵀ)
/// *k*ᵀ = **1**ᵀ.
pub fn katz(succ: &[Vec<usize>], beta: f64) -> Vec<f64> {
    let n = succ.len();
    let mut a = vec![0.0; n * n];
    for i in 0..n {
        a[i * n + i] = 1.0;
    }
    for (j, list) in succ.iter().enumerate() {
        for &i in list {
            a[i * n + j] -= beta;
        }
    }
    let mut k = vec![1.0; n];
    lu::solve(&mut a, &mut k);
    let sum: f64 = k.iter().sum();
    for x in &mut k {
        *x /= sum;
    }
    k
}

/// Returns the scalar product of two vectors.
fn dot(x: &[f64], y: &[f64]) -> f64 {
    x.iter().zip(y).map(|(a, b)| a * b).sum()
}

/// Returns whether a spectral radius equals ρ within [`BASIC_TOLERANCE`].
fn is_basic(radius: f64, rho: f64) -> bool {
    rho > 0.0 && (radius - rho).abs() <= BASIC_TOLERANCE * rho
}

/// Returns the spectral radius and the left Perron vector, normalized in ℓ1,
/// of the adjacency matrix of a strongly connected graph with *m* > 1 nodes
/// and the given arcs.
///
/// The right Perron vector is the left Perron vector of the transposed
/// graph, that is, of the reversed arcs.
///
/// # Implementation Notes
///
/// This is the power method on the primitive matrix *M* = *A* + *I*,
/// starting from the uniform vector: *x* ← *xM* / ‖*xM*‖₁. Since *x* is
/// positive, the Collatz–Wielandt bounds lo = minᵢ (*xM*)ᵢ / *x*ᵢ and hi =
/// maxᵢ (*xM*)ᵢ / *x*ᵢ enclose the spectral radius of *M*, which is ρ + 1;
/// the iteration stops when hi − lo ≤ [`PERRON_TOLERANCE`] · hi, or after
/// [`PERRON_ITERATIONS`] steps, and returns (lo + hi) / 2 − 1.
///
/// The products are sparse, so each step costs *O*(*m* + arcs).
fn left_perron(m: usize, arcs: &[(usize, usize)]) -> (f64, Vec<f64>) {
    debug_assert!(m > 1);
    let mut x = vec![1.0 / m as f64; m];
    let mut z = vec![0.0; m];
    let mut estimate = 0.0;
    for _ in 0..PERRON_ITERATIONS {
        z.copy_from_slice(&x);
        for &(i, j) in arcs {
            z[j] += x[i];
        }
        let (mut lo, mut hi, mut norm) = (f64::INFINITY, 0.0_f64, 0.0);
        for (&zi, &xi) in z.iter().zip(&x) {
            let ratio = zi / xi;
            lo = lo.min(ratio);
            hi = hi.max(ratio);
            norm += zi;
        }
        for (xi, &zi) in x.iter_mut().zip(&z) {
            *xi = zi / norm;
        }
        estimate = (lo + hi) / 2.0;
        if hi - lo <= PERRON_TOLERANCE * hi {
            break;
        }
    }
    (estimate - 1.0, x)
}

/// A strongly connected component of the graph, with its spectral data.
#[derive(Clone, Debug, PartialEq)]
struct Class {
    /// The nodes, sorted; the local index of a node is its position here.
    nodes: Vec<usize>,
    /// The arcs of *A*_CC, as pairs of local indices.
    arcs: Vec<(usize, usize)>,
    /// The spectral radius ρ_C of *A*_CC.
    radius: f64,
    /// The left and right Perron vectors of *A*_CC, normalized in ℓ1, if
    /// the class is basic.
    perron: Option<(Vec<f64>, Vec<f64>)>,
}

/// The spectral structure of the adjacency matrix *A* of a graph, as needed
/// by Katz's index and eigenvector centrality.
///
/// The *classes* are the strongly connected components *C* of the graph;
/// each has a spectral radius ρ_C, the spectral radius of the restriction
/// *A*_CC of *A* to *C*, and ρ, the spectral radius of *A*, is the maximum
/// of the ρ_C. When ρ > 0, the classes with ρ_C = ρ (within a relative
/// tolerance of 10⁻⁹) are *basic*.
#[derive(Clone, Debug, PartialEq)]
pub struct Spectrum {
    /// The successor lists.
    succ: Vec<Vec<usize>>,
    /// The classes, in topological order of the condensation: if there is
    /// an arc from a class *D* to a class *C*, *D* comes first.
    classes: Vec<Class>,
    /// The index in `classes` of the class of each node.
    class_of: Vec<usize>,
    /// The spectral radius of *A*.
    rho: f64,
}

impl Spectrum {
    /// Computes the spectral structure of a graph given by successor lists.
    ///
    /// # Implementation Notes
    ///
    /// The spectral radius of a class with one node is 1 if the node has a
    /// self-loop and 0 otherwise, and its Perron vectors are (1). For larger
    /// classes, the spectral radius and the left Perron vector come from the
    /// power method on *A*_CC + *I*, which is primitive; the right Perron
    /// vector is computed in the same way, on the transpose, only for basic
    /// classes.
    pub fn new(succ: &[Vec<usize>]) -> Self {
        let n = succ.len();
        let mut components = sccs(succ);
        components.reverse();
        let mut class_of = vec![0; n];
        let mut local = vec![0; n];
        for (c, nodes) in components.iter().enumerate() {
            for (i, &v) in nodes.iter().enumerate() {
                class_of[v] = c;
                local[v] = i;
            }
        }

        // Local arcs, spectral radius, and left Perron vector of each class.
        let spectra: Vec<_> = components
            .into_iter()
            .enumerate()
            .map(|(c, nodes)| {
                let mut arcs = Vec::new();
                for (i, &v) in nodes.iter().enumerate() {
                    for &w in &succ[v] {
                        if class_of[w] == c {
                            arcs.push((i, local[w]));
                        }
                    }
                }
                let (radius, left) = if nodes.len() == 1 {
                    (if arcs.is_empty() { 0.0 } else { 1.0 }, vec![1.0])
                } else {
                    left_perron(nodes.len(), &arcs)
                };
                (nodes, arcs, radius, left)
            })
            .collect();

        let rho = spectra
            .iter()
            .fold(0.0, |max: f64, &(_, _, radius, _)| max.max(radius));
        let classes = spectra
            .into_iter()
            .map(|(nodes, arcs, radius, left)| {
                let perron = is_basic(radius, rho).then(|| {
                    let right = if nodes.len() == 1 {
                        vec![1.0]
                    } else {
                        let reversed: Vec<(usize, usize)> =
                            arcs.iter().map(|&(i, j)| (j, i)).collect();
                        left_perron(nodes.len(), &reversed).1
                    };
                    (left, right)
                });
                Class {
                    nodes,
                    arcs,
                    radius,
                    perron,
                }
            })
            .collect();

        Self {
            succ: succ.to_vec(),
            classes,
            class_of,
            rho,
        }
    }

    /// Returns the spectral radius ρ of *A*.
    ///
    /// It is zero if and only if the graph has no cycles (self-loops
    /// included), and at least one otherwise.
    pub fn rho(&self) -> f64 {
        self.rho
    }

    /// Returns the basic classes, that is, the strongly connected components
    /// *C* with ρ_C = ρ, when ρ > 0; returns no classes when ρ = 0.
    ///
    /// Each class is sorted, and classes are sorted by their smallest node.
    pub fn dominant_classes(&self) -> Vec<Vec<usize>> {
        let mut dominant: Vec<Vec<usize>> = self
            .classes
            .iter()
            .filter(|class| is_basic(class.radius, self.rho))
            .map(|class| class.nodes.clone())
            .collect();
        dominant.sort_unstable_by_key(|nodes| nodes[0]);
        dominant
    }

    /// Returns the limit of [Katz's index](katz) κ(β) as β → 1/ρ.
    ///
    /// Let ν, the index of ρ, be the maximum number of basic classes on a
    /// path of the condensation. Then (1 − βρ)^ν *k*(β) tends to a nonzero
    /// nonnegative vector, and κ(β) tends to its normalization. The limit is
    /// supported on the basic classes that end a chain of ν basic classes,
    /// and on the nodes they reach; in particular, when ν > 1 a basic class
    /// upstream of another one has zero score.
    ///
    /// This method must be called only when ρ > 0.
    ///
    /// # Implementation Notes
    ///
    /// Write *s* = 1 − βρ, and let *h*(*C*) be the maximum number of basic
    /// classes on a path of the condensation ending in *C*. Then the
    /// restriction *k*_C of *k* to a class *C* is *c*_C *s*^−*h*(*C*) +
    /// *O*(*s*^1−*h*(*C*)), and the leading coefficients *c*_C can be
    /// computed exactly class by class, in topological order, from *k*_C
    /// (*I* − β*A*_CC) = **1**_C + β Σ_{*D*→*C*} *k*_D *A*_DC.
    ///
    /// Let *u*_H(*C*) = Σ_{*D*→*C*, *h*(*D*) = *H*} *c*_D *A*_DC / ρ, plus
    /// **1**_C when *H* = 0, and let *H* be the maximum *h*(*D*) over the
    /// classes *D* with an arc into *C* (0 if there are none). Then:
    ///
    /// - if *C* is not basic, *h*(*C*) = *H* and *c*_C = *u*_H(*C*) (*I* −
    ///   *A*_CC/ρ)⁻¹, which is computed by solving a dense system;
    /// - if *C* is basic, *h*(*C*) = *H* + 1 and *c*_C = (*u*_H(*C*) *r*_C
    ///   / (*l*_C *r*_C)) *l*_C, where *l*_C and *r*_C are the left and
    ///   right Perron vectors of *A*_CC, as the pole of (*I* − β*A*_CC)⁻¹
    ///   at β = 1/ρ is *r*_C *l*_C / (*s* *l*_C *r*_C).
    ///
    /// The limit is the normalization of the vector that is *c*_C on the
    /// classes with *h*(*C*) = ν and zero elsewhere.
    pub fn katz_limit(&self) -> Vec<f64> {
        debug_assert!(self.rho > 0.0, "the Katz limit requires ρ > 0");
        let n = self.succ.len();
        // Predecessors in other classes.
        let mut pred = vec![Vec::new(); n];
        for (v, list) in self.succ.iter().enumerate() {
            for &w in list {
                if self.class_of[w] != self.class_of[v] {
                    pred[w].push(v);
                }
            }
        }

        // The leading coefficients, by node, and h, by class.
        let mut coeff = vec![0.0; n];
        let mut h = vec![0; self.classes.len()];
        for (c, class) in self.classes.iter().enumerate() {
            let height = class
                .nodes
                .iter()
                .flat_map(|&v| &pred[v])
                .map(|&p| h[self.class_of[p]])
                .max()
                .unwrap_or(0);
            // u_H(C), where H = height
            let ones = if height == 0 { 1.0 } else { 0.0 };
            let mut u: Vec<f64> = class
                .nodes
                .iter()
                .map(|&v| {
                    let inflow: f64 = pred[v]
                        .iter()
                        .filter(|&&p| h[self.class_of[p]] == height)
                        .map(|&p| coeff[p])
                        .sum();
                    inflow / self.rho + ones
                })
                .collect();

            if let Some((l, r)) = &class.perron {
                let scale = dot(&u, r) / dot(l, r);
                for (&v, &x) in class.nodes.iter().zip(l) {
                    coeff[v] = scale * x;
                }
                h[c] = height + 1;
            } else {
                // (I − A_CCᵀ/ρ) c_Cᵀ = u_Hᵀ
                let m = class.nodes.len();
                let mut a = vec![0.0; m * m];
                for i in 0..m {
                    a[i * m + i] = 1.0;
                }
                for &(i, j) in &class.arcs {
                    a[j * m + i] -= 1.0 / self.rho;
                }
                lu::solve(&mut a, &mut u);
                for (&v, &x) in class.nodes.iter().zip(&u) {
                    coeff[v] = x;
                }
                h[c] = height;
            }
        }

        let nu = h.iter().copied().max().unwrap_or(0);
        for (v, x) in coeff.iter_mut().enumerate() {
            if h[self.class_of[v]] != nu {
                *x = 0.0;
            }
        }
        let sum: f64 = coeff.iter().sum();
        for x in &mut coeff {
            *x /= sum;
        }
        coeff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rank::tests::assert_close;

    /// Returns the successor lists of a graph with `n` nodes and the given
    /// arcs.
    fn succ_of(n: usize, arcs: &[(usize, usize)]) -> Vec<Vec<usize>> {
        let mut succ = vec![Vec::new(); n];
        for &(s, t) in arcs {
            succ[s].push(t);
        }
        succ
    }

    /// a ⇄ b plus b → b: ρ is the golden ratio.
    fn golden() -> Vec<Vec<usize>> {
        vec![vec![1], vec![0, 1]]
    }

    /// a ⇄ b → c ⇄ d: two basic classes in a chain (ν = 2).
    fn chained() -> Vec<Vec<usize>> {
        vec![vec![1], vec![0, 2], vec![3], vec![2]]
    }

    /// Two 2-cycles in parallel, a source into both, both into a dangling
    /// node.
    fn parallel() -> Vec<Vec<usize>> {
        succ_of(
            6,
            &[
                (0, 1),
                (1, 0),
                (2, 3),
                (3, 2),
                (4, 0),
                (4, 2),
                (1, 5),
                (3, 5),
            ],
        )
    }

    /// The periodic a ⇄ b, c → a.
    fn periodic() -> Vec<Vec<usize>> {
        vec![vec![1], vec![0], vec![0]]
    }

    /// A chain of three self-loops into a plain node (ν = 3).
    fn self_loops() -> Vec<Vec<usize>> {
        succ_of(4, &[(0, 0), (0, 1), (1, 1), (1, 2), (2, 2), (2, 3)])
    }

    /// A triangle with a self-loop on a, feeding a (non-basic) 2-cycle.
    fn triangle() -> Vec<Vec<usize>> {
        succ_of(5, &[(0, 0), (0, 1), (1, 2), (2, 0), (2, 3), (3, 4), (4, 3)])
    }

    #[test]
    fn test_rho() {
        assert_eq!(Spectrum::new(&[vec![1], vec![2], vec![]]).rho(), 0.0); // DAG
        assert!((Spectrum::new(&[vec![1], vec![2], vec![0]]).rho() - 1.0).abs() < 1e-12);
        // a ⇄ b plus b → b: ρ is the golden ratio
        assert!((Spectrum::new(&golden()).rho() - std::f64::consts::GOLDEN_RATIO).abs() < 1e-12);
        assert!((Spectrum::new(&[vec![0]]).rho() - 1.0).abs() < 1e-12);
    }

    #[test]
    fn test_katz_zero_is_uniform() {
        assert_close(&katz(&golden(), 0.0), &[0.5, 0.5], 1e-15);
    }

    #[test]
    fn test_katz_dag_beta_one() {
        // a → b → c, β = 1: (1, 2, 3) / 6
        assert_close(
            &katz(&[vec![1], vec![2], vec![]], 1.0),
            &[1.0 / 6.0, 1.0 / 3.0, 0.5],
            1e-12,
        );
    }

    #[test]
    fn test_katz_limit_values() {
        // a ⇄ b → c ⇄ d (ν = 2): zero upstream
        assert_close(
            &Spectrum::new(&chained()).katz_limit(),
            &[0.0, 0.0, 0.5, 0.5],
            1e-12,
        );
        assert_close(
            &Spectrum::new(&parallel()).katz_limit(),
            &[1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0, 1.0 / 6.0, 0.0, 1.0 / 3.0],
            1e-12,
        );
        assert_close(
            &Spectrum::new(&periodic()).katz_limit(),
            &[0.5, 0.5, 0.0],
            1e-12,
        );
        assert_close(
            &Spectrum::new(&self_loops()).katz_limit(),
            &[0.0, 0.0, 0.5, 0.5],
            1e-12,
        );
    }

    #[test]
    fn test_katz_limit_matches_near_limit() {
        for succ in [chained(), parallel(), periodic(), self_loops(), triangle()] {
            let s = Spectrum::new(&succ);
            assert_close(&s.katz_limit(), &katz(&succ, (1.0 - 1e-6) / s.rho()), 1e-5);
        }
    }

    #[test]
    fn test_dominant_classes() {
        // a 3-cycle and a 5-cycle (both ρ = 1) are both dominant; a lone node
        // is not
        let succ = succ_of(
            9,
            &[
                (0, 1),
                (1, 2),
                (2, 0),
                (3, 4),
                (4, 5),
                (5, 6),
                (6, 7),
                (7, 3),
                (8, 0),
            ],
        );
        assert_eq!(
            Spectrum::new(&succ).dominant_classes(),
            vec![vec![0, 1, 2], vec![3, 4, 5, 6, 7]]
        );
        assert!(
            Spectrum::new(&[vec![1], vec![]])
                .dominant_classes()
                .is_empty()
        );
        // triangle with self-loop beats a 2-cycle downstream
        assert_eq!(
            Spectrum::new(&triangle()).dominant_classes(),
            vec![vec![0, 1, 2]]
        );
    }

    #[test]
    fn test_katz_sums_to_one() {
        for succ in [golden(), parallel(), triangle()] {
            let rho = Spectrum::new(&succ).rho();
            for t in [0.5, 0.99] {
                let sum: f64 = katz(&succ, t / rho).iter().sum();
                assert!((sum - 1.0).abs() < 1e-12, "βρ {t}: {sum}");
            }
        }
    }
}
