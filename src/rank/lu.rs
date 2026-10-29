//! Dense linear systems.

/// Solves the linear system *A* **x** = **b** by Gaussian elimination with
/// partial pivoting, overwriting `b` with the solution.
///
/// # Arguments
///
/// * `a` - The matrix *A*, row-major, of size *n* × *n*, where *n* is the
///   length of `b`; it is destroyed.
///
/// * `b` - The right-hand side; on return, it contains the solution.
pub(crate) fn solve(a: &mut [f64], b: &mut [f64]) {
    let n = b.len();
    assert_eq!(a.len(), n * n);
    for k in 0..n {
        let pivot = (k..n)
            .max_by(|&i, &j| a[i * n + k].abs().total_cmp(&a[j * n + k].abs()))
            .expect("the pivot range is not empty");
        debug_assert!(a[pivot * n + k] != 0.0, "singular matrix");
        if pivot != k {
            for j in k..n {
                a.swap(k * n + j, pivot * n + j);
            }
            b.swap(k, pivot);
        }
        let diag = a[k * n + k];
        for i in k + 1..n {
            let factor = a[i * n + k] / diag;
            if factor != 0.0 {
                for j in k + 1..n {
                    a[i * n + j] -= factor * a[k * n + j];
                }
                b[i] -= factor * b[k];
            }
        }
    }
    for k in (0..n).rev() {
        let mut x = b[k];
        for j in k + 1..n {
            x -= a[k * n + j] * b[j];
        }
        b[k] = x / a[k * n + k];
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solve() {
        // 2x + y - z = 8; -3x - y + 2z = -11; -2x + y + 2z = -3 -> (2, 3, -1)
        let mut a = vec![2.0, 1.0, -1.0, -3.0, -1.0, 2.0, -2.0, 1.0, 2.0];
        let mut b = vec![8.0, -11.0, -3.0];
        solve(&mut a, &mut b);
        for (x, e) in b.iter().zip([2.0, 3.0, -1.0]) {
            assert!((x - e).abs() < 1e-12);
        }
    }
}
