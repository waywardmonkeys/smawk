//! This crate implements various functions that help speed up dynamic
//! programming, most importantly the SMAWK algorithm for finding row
//! or column minima in a totally monotone matrix with *m* rows and
//! *n* columns in time O(*m* + *n*). This is much better than the
//! brute force solution which would take O(*mn*). When *m* and *n*
//! are of the same order, this turns a quadratic function into a
//! linear function.
//!
//! # Examples
//!
//! Computing the column minima of an *m* ✕ *n* Monge matrix can be
//! done efficiently with `smawk_column_minima`:
//!
//! ```
//! extern crate ndarray;
//! extern crate smawk;
//!
//! use ndarray::arr2;
//! use smawk::smawk_column_minima;
//!
//! let matrix = arr2(&[
//!     [3, 2, 4, 5, 6],
//!     [2, 1, 3, 3, 4],
//!     [2, 1, 3, 3, 4],
//!     [3, 2, 4, 3, 4],
//!     [4, 3, 2, 1, 1],
//! ]);
//! let minima = vec![1, 1, 4, 4, 4];
//! assert_eq!(smawk_column_minima(&matrix), minima);
//! ```
//!
//! The `minima` vector gives the index of the minimum value per
//! column, so `minima[0] == 1` since the minimum value in the first
//! column is 2 (row 1). Note that the smallest row index is returned.
//!
//! # Definitions
//!
//! Some of the functions in this crate only work on matrices that are
//! *totally monotone*, which we will define below.
//!
//! ## Monotone Matrices
//!
//! We start with a helper definition. Given an *m* ✕ *n* matrix `M`,
//! we say that `M` is *monotone* when the minimum value of row `i` is
//! found to the left of the minimum value in row `i'` where `i < i'`.
//!
//! More formally, if we let `rm(i)` denote the column index of the
//! left-most minimum value in row `i`, then we have
//!
//! ```text
//! rm(0) ≤ rm(1) ≤ ... ≤ rm(m - 1)
//! ```
//!
//! This means that as you go down the rows from top to bottom, the
//! row-minima proceed from left to right.
//!
//! The algorithms in this crate deal with finding such row- and
//! column-minima.
//!
//! ## Totally Monotone Matrices
//!
//! We say that a matrix `M` is *totally monotone* when every
//! sub-matrix is monotone. A sub-matrix is formed by the intersection
//! of any two rows `i < i'` and any two columns `j < j'`.
//!
//! This is often expressed as via this equivalent condition:
//!
//! ```text
//! M[i, j] > M[i, j']  =>  M[i', j] > M[i', j']
//! ```
//!
//! for all `i < i'` and `j < j'`.
//!
//! ## Monge Property for Matrices
//!
//! A matrix `M` is said to fulfill the *Monge property* if
//!
//! ```text
//! M[i, j] + M[i', j'] ≤ M[i, j'] + M[i', j]
//! ```
//!
//! for all `i < i'` and `j < j'`. This says that given any rectangle
//! in the matrix, the sum of the top-left and bottom-right corners is
//! less than or equal to the sum of the bottom-left and upper-right
//! corners.
//!
//! All Monge matrices are totally monotone, so it is enough to
//! establish that the Monge property holds in order to use a matrix
//! with the functions in this crate. If your program is dealing with
//! unknown inputs, it can use [`is_monge`] to verify that a matrix is
//! a Monge matrix.
//!
//! [`is_monge`]: fn.is_monge.html

#![doc(html_root_url = "https://docs.rs/smawk/0.1.0")]

use ndarray::{s, Array2, ArrayView1, ArrayView2, Axis, Si};
use num_traits::{PrimInt, WrappingAdd};
use rand::{Rand, Rng};
use rand_derive::Rand;

/// Compute lane minimum by brute force.
///
/// This does a simple scan through the lane (row or column).
#[inline]
fn lane_minimum<T: Ord>(lane: ArrayView1<'_, T>) -> usize {
    lane.iter()
        .enumerate()
        .min_by_key(|&(idx, elem)| (elem, idx))
        .map(|(idx, _)| idx)
        .expect("empty lane in matrix")
}

/// Compute row minima by brute force in O(*mn*) time.
///
/// # Panics
///
/// It is an error to call this on a matrix with zero columns.
pub fn brute_force_row_minima<T: Ord>(matrix: &Array2<T>) -> Vec<usize> {
    matrix.genrows().into_iter().map(lane_minimum).collect()
}

/// Compute column minima by brute force in O(*mn*) time.
///
/// # Panics
///
/// It is an error to call this on a matrix with zero rows.
pub fn brute_force_column_minima<T: Ord>(matrix: &Array2<T>) -> Vec<usize> {
    matrix.gencolumns().into_iter().map(lane_minimum).collect()
}

/// Compute row minima in O(*m* + *n* log *m*) time.
///
/// # Panics
///
/// It is an error to call this on a matrix with zero columns.
pub fn recursive_row_minima<T: Ord>(matrix: &Array2<T>) -> Vec<usize> {
    let mut minima = vec![0; matrix.rows()];
    recursive_inner(matrix.view(), &|| Direction::Row, 0, &mut minima);
    minima
}

/// Compute column minima in O(*n* + *m* log *n*) time.
///
/// # Panics
///
/// It is an error to call this on a matrix with zero rows.
pub fn recursive_column_minima<T: Ord>(matrix: &Array2<T>) -> Vec<usize> {
    let mut minima = vec![0; matrix.cols()];
    recursive_inner(matrix.view(), &|| Direction::Column, 0, &mut minima);
    minima
}

/// The type of minima (row or column) we compute.
enum Direction {
    Row,
    Column,
}

/// Compute the minima along the given direction (`Direction::Row` for
/// row minima and `Direction::Column` for column minima).
///
/// The direction is given as a generic function argument to allow
/// monomorphization to kick in. The function calls will be inlined
/// and optimized away and the result is that the compiler generates
/// differnet code for finding row and column minima.
fn recursive_inner<T: Ord, F: Fn() -> Direction>(
    matrix: ArrayView2<'_, T>,
    dir: &F,
    offset: usize,
    minima: &mut [usize],
) {
    if matrix.is_empty() {
        return;
    }

    let axis = match dir() {
        Direction::Row => Axis(0),
        Direction::Column => Axis(1),
    };
    let mid = matrix.len_of(axis) / 2;
    let min_idx = lane_minimum(matrix.subview(axis, mid));
    minima[mid] = offset + min_idx;

    if mid == 0 {
        return; // Matrix has a single row or column, so we're done.
    }

    let top_left = match dir() {
        Direction::Row => [
            Si(0, Some(mid as isize), 1),
            Si(0, Some((min_idx + 1) as isize), 1),
        ],
        Direction::Column => [
            Si(0, Some((min_idx + 1) as isize), 1),
            Si(0, Some(mid as isize), 1),
        ],
    };
    let bot_right = match dir() {
        Direction::Row => [
            Si((mid + 1) as isize, None, 1),
            Si(min_idx as isize, None, 1),
        ],
        Direction::Column => [
            Si(min_idx as isize, None, 1),
            Si((mid + 1) as isize, None, 1),
        ],
    };
    recursive_inner(matrix.slice(&top_left), dir, offset, &mut minima[..mid]);
    recursive_inner(
        matrix.slice(&bot_right),
        dir,
        offset + min_idx,
        &mut minima[mid + 1..],
    );
}

/// Compute row minima in O(*m* + *n*) time.
///
/// This implements the SMAWK algorithm for finding row minima in a
/// totally monotone matrix.
///
/// The SMAWK algorithm is from Agarwal, Klawe, Moran, Shor, and
/// Wilbur, *Geometric applications of a matrix searching algorithm*,
/// Algorithmica 2, pp. 195-208 (1987) and the code here is a
/// translation [David Eppstein's Python code][pads].
///
/// [pads]: https://github.com/jfinkels/PADS/blob/master/pads/smawk.py
///
/// Running time on an *m* ✕ *n* matrix: O(*m* + *n*).
///
/// # Panics
///
/// It is an error to call this on a matrix with zero columns.
pub fn smawk_row_minima<T: Ord + Copy>(matrix: &Array2<T>) -> Vec<usize> {
    // Benchmarking shows that SMAWK performs roughly the same on row-
    // and column-major matrices.
    let mut minima = vec![0; matrix.rows()];
    smawk_inner(
        &|j, i| matrix[[i, j]],
        &(0..matrix.cols()).collect::<Vec<_>>(),
        &(0..matrix.rows()).collect::<Vec<_>>(),
        &mut minima,
    );
    minima
}

/// Compute column minima in O(*m* + *n*) time.
///
/// This implements the SMAWK algorithm for finding column minima in a
/// totally monotone matrix.
///
/// The SMAWK algorithm is from Agarwal, Klawe, Moran, Shor, and
/// Wilbur, *Geometric applications of a matrix searching algorithm*,
/// Algorithmica 2, pp. 195-208 (1987) and the code here is a
/// translation [David Eppstein's Python code][pads].
///
/// [pads]: https://github.com/jfinkels/PADS/blob/master/pads/smawk.py
///
/// Running time on an *m* ✕ *n* matrix: O(*m* + *n*).
///
/// # Panics
///
/// It is an error to call this on a matrix with zero rows.
pub fn smawk_column_minima<T: Ord + Copy>(matrix: &Array2<T>) -> Vec<usize> {
    let mut minima = vec![0; matrix.cols()];
    smawk_inner(
        &|i, j| matrix[[i, j]],
        &(0..matrix.rows()).collect::<Vec<_>>(),
        &(0..matrix.cols()).collect::<Vec<_>>(),
        &mut minima,
    );
    minima
}

/// Compute column minima in the given area of the matrix. The
/// `minima` slice is updated inplace.
fn smawk_inner<T: Ord + Copy, M: Fn(usize, usize) -> T>(
    matrix: &M,
    rows: &[usize],
    cols: &[usize],
    mut minima: &mut [usize],
) {
    if cols.is_empty() {
        return;
    }

    let mut stack = Vec::with_capacity(cols.len());
    for r in rows {
        // TODO: use stack.last() instead of stack.is_empty() etc
        while !stack.is_empty()
            && matrix(stack[stack.len() - 1], cols[stack.len() - 1])
                > matrix(*r, cols[stack.len() - 1])
        {
            stack.pop();
        }
        if stack.len() != cols.len() {
            stack.push(*r);
        }
    }
    let rows = &stack;

    let mut odd_cols = Vec::with_capacity(1 + cols.len() / 2);
    for (idx, c) in cols.iter().enumerate() {
        if idx % 2 == 1 {
            odd_cols.push(*c);
        }
    }

    smawk_inner(matrix, rows, &odd_cols, &mut minima);

    let mut r = 0;
    for (c, &col) in cols.iter().enumerate().filter(|(c, _)| c % 2 == 0) {
        let mut row = rows[r];
        let last_row = if c == cols.len() - 1 {
            rows[rows.len() - 1]
        } else {
            minima[cols[c + 1]]
        };
        let mut pair = (matrix(row, col), row);
        while row != last_row {
            r += 1;
            row = rows[r];
            pair = std::cmp::min(pair, (matrix(row, col), row));
        }
        minima[col] = pair.1;
    }
}

/// Compute upper-right column minima in O(*m* + *n*) time.
///
/// The input matrix must be totally monotone.
///
/// The function returns a vector of `(usize, T)`. The `usize` in the
/// tuple at index `j` tells you the row of the minimum value in
/// column `j` and the `T` value is minimum value itself.
///
/// The algorithm only considers values above the main diagonal, which
/// means that it computes values `v(j)` where:
///
/// ```text
/// v(0) = initial
/// v(j) = min { M[i, j] | i < j } for j > 0
/// ```
///
/// If we let `r(j)` denote the row index of the minimum value in
/// column `j`, the tuples in the result vector become `(r(j), M[r(j),
/// j])`.
///
/// The algorithm is an *online* algorithm, in the sense that `matrix`
/// function can refer back to previously computed column minima when
/// determining an entry in the matrix. The guarantee is that we only
/// call `matrix(i, j)` after having computed `v(i)`. This is
/// reflected in the `&[(usize, T)]` argument to `matrix`, which grows
/// as more and more values are computed.
pub fn online_column_minima<T: Copy + Ord, M: Fn(&[(usize, T)], usize, usize) -> T>(
    initial: T,
    size: usize,
    matrix: M,
) -> Vec<(usize, T)> {
    let mut result = vec![(0, initial)];

    // State used by the algorithm.
    let mut finished = 0;
    let mut base = 0;
    let mut tentative = 0;

    // Shorthand for evaluating the matrix. We need a macro here since
    // we don't want to borrow the result vector.
    macro_rules! m {
        ($i:expr, $j:expr) => {{
            assert!($i < $j, "(i, j) not above diagonal: ({}, {})", $i, $j);
            assert!(
                $i < size && $j < size,
                "(i, j) out of bounds: ({}, {}), size: {}",
                $i,
                $j,
                size
            );
            matrix(&result[..finished + 1], $i, $j)
        }};
    }

    // Keep going until we have finished all size columns. Since the
    // columns are zero-indexed, we're done when finished == size - 1.
    while finished < size - 1 {
        // First case: we have already advanced past the previous
        // tentative value. We make a new tentative value by applying
        // smawk_inner to the largest square submatrix that fits under
        // the base.
        let i = finished + 1;
        if i > tentative {
            let rows = (base..finished + 1).collect::<Vec<_>>();
            tentative = std::cmp::min(finished + rows.len(), size - 1);
            let cols = (finished + 1..tentative + 1).collect::<Vec<_>>();
            let mut minima = vec![0; tentative + 1];
            smawk_inner(&|i, j| m![i, j], &rows, &cols, &mut minima);
            for col in cols {
                let row = minima[col];
                let v = m![row, col];
                if col >= result.len() {
                    result.push((row, v));
                } else if v < result[col].1 {
                    result[col] = (row, v);
                }
            }
            finished = i;
            continue;
        }

        // Second case: the new column minimum is on the diagonal. All
        // subsequent ones will be at least as low, so we can clear
        // out all our work from higher rows. As in the fourth case,
        // the loss of tentative is amortized against the increase in
        // base.
        let diag = m![i - 1, i];
        if diag < result[i].1 {
            result[i] = (i - 1, diag);
            base = i - 1;
            tentative = i;
            finished = i;
            continue;
        }

        // Third case: row i-1 does not supply a column minimum in any
        // column up to tentative. We simply advance finished while
        // maintaining the invariant.
        if m![i - 1, tentative] >= result[tentative].1 {
            finished = i;
            continue;
        }

        // Fourth and final case: a new column minimum at tentative.
        // This allows us to make progress by incorporating rows prior
        // to finished into the base. The base invariant holds because
        // these rows cannot supply any later column minima. The work
        // done when we last advanced tentative (and undone by this
        // step) can be amortized against the increase in base.
        base = i - 1;
        tentative = i;
        finished = i;
    }

    result
}

/// Verify that a matrix is a Monge matrix.
///
/// A [Monge matrix] \(or array) is a matrix where the following
/// inequality holds:
///
/// ```text
/// M[i, j] + M[i', j'] <= M[i, j'] + M[i', j]  for all i < i', j < j'
/// ```
///
/// The inequality says that the sum of the main diagonal is less than
/// the sum of the antidiagonal. Checking this condition is done by
/// checking *n* ✕ *m* submatrices, so the running time is O(*mn*).
///
/// [Monge matrix]: https://en.wikipedia.org/wiki/Monge_array
pub fn is_monge<T: PrimInt + WrappingAdd>(matrix: &Array2<T>) -> bool {
    matrix.windows([2, 2]).into_iter().all(|sub| {
        let (x, y) = (sub[[0, 0]], sub[[1, 1]]);
        let (z, w) = (sub[[0, 1]], sub[[1, 0]]);
        match (x.checked_add(&y), z.checked_add(&w)) {
            (Some(a), Some(b)) => a <= b,
            (Some(_), None) => true,
            (None, Some(_)) => false,
            (None, None) => x.wrapping_add(&y) <= z.wrapping_add(&w),
        }
    })
}

/// A Monge matrix can be decomposed into one of these primitive
/// building blocks.
#[derive(Rand)]
enum MongePrim {
    ConstantRows,
    ConstantCols,
    UpperRightOnes,
    LowerLeftOnes,
}

impl MongePrim {
    /// Generate a Monge matrix from a primitive.
    fn to_matrix<T: Rand + PrimInt, R: Rng>(&self, m: usize, n: usize, rng: &mut R) -> Array2<T> {
        let mut matrix = Array2::from_elem((m, n), T::zero());
        // Avoid panic in UpperRightOnes and LowerLeftOnes below.
        if m == 0 || n == 0 {
            return matrix;
        }

        match *self {
            MongePrim::ConstantRows => {
                for mut row in matrix.genrows_mut() {
                    row.fill(rng.gen());
                }
            }
            MongePrim::ConstantCols => {
                for mut col in matrix.gencolumns_mut() {
                    col.fill(rng.gen());
                }
            }
            MongePrim::UpperRightOnes => {
                let i = rng.gen_range(0, (m + 1) as isize);
                let j = rng.gen_range(0, (n + 1) as isize);
                matrix.slice_mut(s![..i, -j..]).fill(T::one());
            }
            MongePrim::LowerLeftOnes => {
                let i = rng.gen_range(0, (m + 1) as isize);
                let j = rng.gen_range(0, (n + 1) as isize);
                matrix.slice_mut(s![-i.., ..j]).fill(T::one());
            }
        }

        matrix
    }
}

/// Generate a random Monge matrix.
pub fn random_monge_matrix<R: Rng, T>(m: usize, n: usize, rng: &mut R) -> Array2<T>
where
    T: Rand + PrimInt,
{
    let mut matrix = Array2::from_elem((m, n), T::zero());
    for _ in 0..(m + n) {
        let monge = if rng.gen() {
            MongePrim::LowerLeftOnes
        } else {
            MongePrim::UpperRightOnes
        };
        matrix = matrix + monge.to_matrix(m, n, rng);
    }
    matrix
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::arr2;
    use rand::XorShiftRng;

    #[test]
    fn is_monge_handles_overflow() {
        // The x + y <= z + w computations will overflow for an u8
        // matrix unless is_monge is careful.
        let matrix: Array2<u8> = arr2(&[
            [200, 200, 200, 200],
            [200, 200, 200, 200],
            [200, 200, 200, 200],
        ]);
        assert!(is_monge(&matrix));
    }

    #[test]
    fn monge_constant_rows() {
        let mut rng = XorShiftRng::new_unseeded();
        assert_eq!(
            MongePrim::ConstantRows.to_matrix(5, 4, &mut rng),
            arr2(&[
                [15u8, 15, 15, 15],
                [132, 132, 132, 132],
                [11, 11, 11, 11],
                [140, 140, 140, 140],
                [67, 67, 67, 67]
            ])
        );
    }

    #[test]
    fn monge_constant_cols() {
        let mut rng = XorShiftRng::new_unseeded();
        let matrix = MongePrim::ConstantCols.to_matrix(5, 4, &mut rng);
        assert!(is_monge(&matrix));
        assert_eq!(
            matrix,
            arr2(&[
                [15u8, 132, 11, 140],
                [15, 132, 11, 140],
                [15, 132, 11, 140],
                [15, 132, 11, 140],
                [15, 132, 11, 140]
            ])
        );
    }

    #[test]
    fn monge_upper_right_ones() {
        let mut rng = XorShiftRng::new_unseeded();
        let matrix = MongePrim::UpperRightOnes.to_matrix(5, 4, &mut rng);
        assert!(is_monge(&matrix));
        assert_eq!(
            matrix,
            arr2(&[
                [0, 0, 0, 1],
                [0, 0, 0, 1],
                [0, 0, 0, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 0]
            ])
        );
    }

    #[test]
    fn monge_lower_left_ones() {
        let mut rng = XorShiftRng::new_unseeded();
        let matrix = MongePrim::LowerLeftOnes.to_matrix(5, 4, &mut rng);
        assert!(is_monge(&matrix));
        assert_eq!(
            matrix,
            arr2(&[
                [0, 0, 0, 0],
                [0, 0, 0, 0],
                [0, 0, 0, 0],
                [1, 0, 0, 0],
                [1, 0, 0, 0]
            ])
        );
    }

    #[test]
    fn brute_force_1x1() {
        let matrix = arr2(&[[2]]);
        let minima = vec![0];
        assert_eq!(brute_force_row_minima(&matrix), minima);
        assert_eq!(brute_force_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_2x1() {
        let matrix = arr2(&[[3], [2]]);
        let minima = vec![0, 0];
        assert_eq!(brute_force_row_minima(&matrix), minima);
        assert_eq!(brute_force_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_1x2() {
        let matrix = arr2(&[[2, 1]]);
        let minima = vec![1];
        assert_eq!(brute_force_row_minima(&matrix), minima);
        assert_eq!(brute_force_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_2x2() {
        let matrix = arr2(&[[3, 2], [2, 1]]);
        let minima = vec![1, 1];
        assert_eq!(brute_force_row_minima(&matrix), minima);
        assert_eq!(brute_force_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_3x3() {
        let matrix = arr2(&[[3, 4, 4], [3, 4, 4], [2, 3, 3]]);
        let minima = vec![0, 0, 0];
        assert_eq!(brute_force_row_minima(&matrix), minima);
        assert_eq!(brute_force_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_4x4() {
        let matrix = arr2(&[[4, 5, 5, 5], [2, 3, 3, 3], [2, 3, 3, 3], [2, 2, 2, 2]]);
        let minima = vec![0, 0, 0, 0];
        assert_eq!(brute_force_row_minima(&matrix), minima);
        assert_eq!(brute_force_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn brute_force_5x5() {
        let matrix = arr2(&[
            [3, 2, 4, 5, 6],
            [2, 1, 3, 3, 4],
            [2, 1, 3, 3, 4],
            [3, 2, 4, 3, 4],
            [4, 3, 2, 1, 1],
        ]);
        let minima = vec![1, 1, 1, 1, 3];
        assert_eq!(brute_force_row_minima(&matrix), minima);
        assert_eq!(brute_force_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_1x1() {
        let matrix = arr2(&[[2]]);
        let minima = vec![0];
        assert_eq!(recursive_row_minima(&matrix), minima);
        assert_eq!(recursive_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_2x1() {
        let matrix = arr2(&[[3], [2]]);
        let minima = vec![0, 0];
        assert_eq!(recursive_row_minima(&matrix), minima);
        assert_eq!(recursive_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_1x2() {
        let matrix = arr2(&[[2, 1]]);
        let minima = vec![1];
        assert_eq!(recursive_row_minima(&matrix), minima);
        assert_eq!(recursive_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_2x2() {
        let matrix = arr2(&[[3, 2], [2, 1]]);
        let minima = vec![1, 1];
        assert_eq!(recursive_row_minima(&matrix), minima);
        assert_eq!(recursive_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_3x3() {
        let matrix = arr2(&[[3, 4, 4], [3, 4, 4], [2, 3, 3]]);
        let minima = vec![0, 0, 0];
        assert_eq!(recursive_row_minima(&matrix), minima);
        assert_eq!(recursive_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_4x4() {
        let matrix = arr2(&[[4, 5, 5, 5], [2, 3, 3, 3], [2, 3, 3, 3], [2, 2, 2, 2]]);
        let minima = vec![0, 0, 0, 0];
        assert_eq!(recursive_row_minima(&matrix), minima);
        assert_eq!(recursive_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn recursive_5x5() {
        let matrix = arr2(&[
            [3, 2, 4, 5, 6],
            [2, 1, 3, 3, 4],
            [2, 1, 3, 3, 4],
            [3, 2, 4, 3, 4],
            [4, 3, 2, 1, 1],
        ]);
        let minima = vec![1, 1, 1, 1, 3];
        assert_eq!(recursive_row_minima(&matrix), minima);
        assert_eq!(recursive_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn smawk_1x1() {
        let matrix = arr2(&[[2]]);
        let minima = vec![0];
        assert_eq!(smawk_row_minima(&matrix), minima);
        assert_eq!(smawk_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn smawk_2x1() {
        let matrix = arr2(&[[3], [2]]);
        let minima = vec![0, 0];
        assert_eq!(smawk_row_minima(&matrix), minima);
        assert_eq!(smawk_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn smawk_1x2() {
        let matrix = arr2(&[[2, 1]]);
        let minima = vec![1];
        assert_eq!(smawk_row_minima(&matrix), minima);
        assert_eq!(smawk_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn smawk_2x2() {
        let matrix = arr2(&[[3, 2], [2, 1]]);
        let minima = vec![1, 1];
        assert_eq!(smawk_row_minima(&matrix), minima);
        assert_eq!(smawk_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn smawk_3x3() {
        let matrix = arr2(&[[3, 4, 4], [3, 4, 4], [2, 3, 3]]);
        let minima = vec![0, 0, 0];
        assert_eq!(smawk_row_minima(&matrix), minima);
        assert_eq!(smawk_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn smawk_4x4() {
        let matrix = arr2(&[[4, 5, 5, 5], [2, 3, 3, 3], [2, 3, 3, 3], [2, 2, 2, 2]]);
        let minima = vec![0, 0, 0, 0];
        assert_eq!(smawk_row_minima(&matrix), minima);
        assert_eq!(smawk_column_minima(&matrix.reversed_axes()), minima);
    }

    #[test]
    fn smawk_5x5() {
        let matrix = arr2(&[
            [3, 2, 4, 5, 6],
            [2, 1, 3, 3, 4],
            [2, 1, 3, 3, 4],
            [3, 2, 4, 3, 4],
            [4, 3, 2, 1, 1],
        ]);
        let minima = vec![1, 1, 1, 1, 3];
        assert_eq!(smawk_row_minima(&matrix), minima);
        assert_eq!(smawk_column_minima(&matrix.reversed_axes()), minima);
    }

    /// Check that the brute force, recursive, and SMAWK functions
    /// give identical results on a large number of randomly generated
    /// Monge matrices.
    #[test]
    fn implementations_agree() {
        let sizes = vec![1, 2, 3, 4, 5, 10, 15, 20, 30];
        let mut rng = XorShiftRng::new_unseeded();
        for _ in 0..4 {
            for m in sizes.clone().iter() {
                for n in sizes.clone().iter() {
                    let matrix: Array2<i32> = random_monge_matrix(*m, *n, &mut rng);

                    // Compute and test row minima.
                    let brute_force = brute_force_row_minima(&matrix);
                    let recursive = recursive_row_minima(&matrix);
                    let smawk = smawk_row_minima(&matrix);
                    assert_eq!(
                        brute_force, recursive,
                        "recursive and brute force differs on:\n{:?}",
                        matrix
                    );
                    assert_eq!(
                        brute_force, smawk,
                        "SMAWK and brute force differs on:\n{:?}",
                        matrix
                    );

                    // Do the same for the column minima.
                    let brute_force = brute_force_column_minima(&matrix);
                    let recursive = recursive_column_minima(&matrix);
                    let smawk = smawk_column_minima(&matrix);
                    assert_eq!(
                        brute_force, recursive,
                        "recursive and brute force differs on:\n{:?}",
                        matrix
                    );
                    assert_eq!(
                        brute_force, smawk,
                        "SMAWK and brute force differs on:\n{:?}",
                        matrix
                    );
                }
            }
        }
    }

    #[test]
    fn online_1x1() {
        let matrix = arr2(&[[0]]);
        let minima = vec![(0, 0)];
        assert_eq!(
            online_column_minima(0, 1, |_, i, j| matrix[[i, j]],),
            minima
        );
    }

    #[test]
    fn online_2x2() {
        let matrix = arr2(&[[0, 2], [0, 0]]);
        let minima = vec![(0, 0), (0, 2)];
        assert_eq!(
            online_column_minima(0, 2, |_, i, j| matrix[[i, j]],),
            minima
        );
    }

    #[test]
    fn online_3x3() {
        let matrix = arr2(&[[0, 4, 4], [0, 0, 4], [0, 0, 0]]);
        let minima = vec![(0, 0), (0, 4), (0, 4)];
        assert_eq!(
            online_column_minima(0, 3, |_, i, j| matrix[[i, j]],),
            minima
        );
    }

    #[test]
    fn online_4x4() {
        let matrix = arr2(&[[0, 5, 5, 5], [0, 0, 3, 3], [0, 0, 0, 3], [0, 0, 0, 0]]);
        let minima = vec![(0, 0), (0, 5), (1, 3), (1, 3)];
        assert_eq!(
            online_column_minima(0, 4, |_, i, j| matrix[[i, j]],),
            minima
        );
    }

    #[test]
    fn online_5x5() {
        let matrix = arr2(&[
            [0, 2, 4, 6, 7],
            [0, 0, 3, 4, 5],
            [0, 0, 0, 3, 4],
            [0, 0, 0, 0, 4],
            [0, 0, 0, 0, 0],
        ]);
        let minima = vec![(0, 0), (0, 2), (1, 3), (2, 3), (2, 4)];
        assert_eq!(online_column_minima(0, 5, |_, i, j| matrix[[i, j]]), minima);
    }

    /// Check that the brute force and online SMAWK functions give
    /// identical results on a large number of randomly generated
    /// Monge matrices.
    #[test]
    fn online_agree() {
        let sizes = vec![1, 2, 3, 4, 5, 10, 15, 20, 30, 50];
        let mut rng = XorShiftRng::new_unseeded();
        for _ in 0..5 {
            for &size in &sizes {
                // Random totally monotone square matrix of the
                // desired size.
                let mut matrix: Array2<i32> = random_monge_matrix(size, size, &mut rng);

                // Adjust matrix so the column minima are above the
                // diagonal. The brute_force_column_minima will still
                // work just fine on such a mangled Monge matrix.
                let max = matrix.fold(0, |max, &elem| std::cmp::max(max, elem));
                for idx in 0..(size as isize) {
                    // Using the maximum value of the matrix instead
                    // of i32::max_value() makes for prettier matrices
                    // in case we want to print them.
                    matrix.slice_mut(s![idx..idx + 1, ..idx + 1]).fill(max);
                }

                // The online algorithm always returns the initial
                // value for the left-most column -- without
                // inspecting the column at all. So we fill the
                // left-most column with this value to have the brute
                // force algorithm do the same.
                let initial = 42;
                matrix.slice_mut(s![0.., ..1]).fill(initial);

                // Brute-force computation of column minima, returned
                // in the same form as online_column_minima.
                let brute_force = brute_force_column_minima(&matrix)
                    .iter()
                    .enumerate()
                    .map(|(j, &i)| (i, matrix[[i, j]]))
                    .collect::<Vec<_>>();
                let online = online_column_minima(initial, size, |_, i, j| matrix[[i, j]]);
                assert_eq!(
                    brute_force, online,
                    "brute force and online differ on:\n{:3?}",
                    matrix
                );
            }
        }
    }
}
