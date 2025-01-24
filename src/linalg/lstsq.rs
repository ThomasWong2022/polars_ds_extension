#![allow(non_snake_case)]
use faer_traits::RealField;
use faer_traits::math_utils::is_nan;
use faer::linalg::solvers::DenseSolveCore;
use faer::mat::Mat;
use faer::{linalg::solvers::Solve, prelude::*};

use super::LinalgErrors;
use core::f64;
use std::ops::Neg;

#[inline]
pub fn has_nan<T:RealField>(mat: MatRef<T>) -> bool {
    mat.col_iter().any(|col| col.iter().any(|x| is_nan(x)))
}


#[derive(Clone, Copy, Default)]
pub enum LRSolverMethods {
    SVD,
    Choleskey,
    #[default]
    QR,
}

impl From<&str> for LRSolverMethods {
    fn from(value: &str) -> Self {
        match value {
            "qr" => Self::QR,
            "svd" => Self::SVD,
            "choleskey" => Self::Choleskey,
            _ => Self::QR,
        }
    }
}

// add elastic net
#[derive(Clone, Copy, Default, PartialEq)]
pub enum LRMethods {
    #[default]
    Normal, // Normal. Normal Equation
    L1, // Lasso, L1 regularized
    L2, // Ridge, L2 regularized
    ElasticNet,
}

impl From<&str> for LRMethods {
    fn from(value: &str) -> Self {
        match value {
            "l1" | "lasso" => Self::L1,
            "l2" | "ridge" => Self::L2,
            "elastic" => Self::ElasticNet,
            _ => Self::Normal,
        }
    }
}

/// Converts a 2-tuple of floats into LRMethods
/// The first entry is assumed to the l1 regularization factor, and
/// the second is assumed to be the l2 regularization factor
impl From<(f64, f64)> for LRMethods {
    fn from(value: (f64, f64)) -> Self {
        if value.0 > 0. && value.1 <= 0. {
            LRMethods::L1
        } else if value.0 <= 0. && value.1 > 0. {
            LRMethods::L2
        } else if value.0 > 0. && value.1 > 0. {
            LRMethods::ElasticNet
        } else {
            LRMethods::Normal
        }
    }
}

// // add elastic net ???

// #[derive(Clone, Copy, Default, PartialEq)]
// pub enum ClosedFormLRMethods {
//     #[default]
//     Normal, // Normal. Normal Equation
//     L2, // Ridge, L2 regularized
// }

// impl From<&str> for ClosedFormLRMethods {
//     fn from(value: &str) -> Self {
//         match value {
//             "l2" | "ridge" => Self::L2,
//             _ => Self::Normal,
//         }
//     }
// }

// impl From<f64> for ClosedFormLRMethods {
//     fn from(value: f64) -> Self {
//         if value > 0. {
//             Self::L2
//         } else {
//             Self::Normal
//         }
//     }
// }

pub trait LinearRegression {
    fn coefficients(&self) -> MatRef<f64>;

    /// Returns a copy of the coefficients
    fn get_coefficients(&self) -> Mat<f64> {
        self.coefficients().to_owned()
    }

    fn bias(&self) -> f64;

    fn fit_bias(&self) -> bool;

    fn fit_unchecked(&mut self, X: MatRef<f64>, y: MatRef<f64>);

    /// Fits the linear regression. Input X is any m x n matrix. Input y must be a m x 1 matrix.
    /// Note, if there is a bias term in the data, then it must be in the matrix X as the last
    /// column and has_bias must be true. This will not append a bias column to X.
    fn fit(&mut self, X: MatRef<f64>, y: MatRef<f64>) -> Result<(), LinalgErrors> {
        if X.nrows() != y.nrows() {
            return Err(LinalgErrors::DimensionMismatch);
        } else if X.nrows() < X.ncols() || X.nrows() == 0 || y.nrows() == 0 {
            return Err(LinalgErrors::NotEnoughData);
        }
        self.fit_unchecked(X, y);
        Ok(())
    }

    fn is_fit(&self) -> bool {
        !(self.coefficients().shape() == (0, 0))
    }

    fn coeffs_as_vec(&self) -> Result<Vec<f64>, LinalgErrors> {
        match self.check_is_fit() {
            Ok(_) => Ok(self
                .coefficients()
                .col(0)
                .iter()
                .copied()
                .collect::<Vec<_>>()),
            Err(e) => Err(e),
        }
    }

    fn check_is_fit(&self) -> Result<(), LinalgErrors> {
        if self.is_fit() {
            Ok(())
        } else {
            Err(LinalgErrors::MatNotLearnedYet)
        }
    }

    fn predict(&self, X: MatRef<f64>) -> Result<Mat<f64>, LinalgErrors> {
        if X.ncols() != self.coefficients().nrows() {
            Err(LinalgErrors::DimensionMismatch)
        } else if !self.is_fit() {
            Err(LinalgErrors::MatNotLearnedYet)
        } else {
            let mut result = X * self.coefficients();
            let bias = self.bias();
            if self.fit_bias() && self.bias().abs() > f64::EPSILON {
                unsafe {
                    for i in 0..result.nrows() {
                        *result.get_mut_unchecked(i, 0) += bias;
                    }
                }
            }
            Ok(result)
        }
    }
}

/// A struct that handles regular linear regression and Ridge regression.
pub struct LR {
    pub solver: LRSolverMethods,
    pub lambda: f64,
    pub coefficients: Mat<f64>, // n_features x 1 matrix, doesn't contain bias
    pub fit_bias: bool,
    pub bias: f64,
}

impl LR {
    pub fn new(solver: &str, lambda: f64, fit_bias: bool) -> Self {
        LR {
            solver: solver.into(),
            lambda: lambda,
            coefficients: Mat::new(),
            fit_bias: fit_bias,
            bias: 0.,
        }
    }

    pub fn from_values(coeffs: &[f64], bias: f64) -> Self {
        LR {
            solver: LRSolverMethods::default(),
            lambda: 0.,
            coefficients: faer::mat::Mat::from_fn(coeffs.len(), 1, |i, _| coeffs[i]),
            // from_row_major_slice(coeffs, coeffs.len(), 1).to_owned(),
            fit_bias: bias.abs() > f64::EPSILON,
            bias: bias,
        }
    }

    pub fn set_coeffs_and_bias(&mut self, coeffs: &[f64], bias: f64) {
        self.coefficients = faer::mat::Mat::from_fn(coeffs.len(), 1, |i, _| coeffs[i]);
        self.bias = bias;
        self.fit_bias = bias.abs() > f64::EPSILON;
    }
}

impl LinearRegression for LR {
    fn coefficients(&self) -> MatRef<f64> {
        self.coefficients.as_ref()
    }

    fn bias(&self) -> f64 {
        self.bias
    }

    fn fit_bias(&self) -> bool {
        self.fit_bias
    }

    fn fit_unchecked(&mut self, X: MatRef<f64>, y: MatRef<f64>) {
        let all_coefficients = if self.fit_bias {
            let ones = Mat::full(X.nrows(), 1, 1.0);
            let new = faer::concat![[X, ones]];
            faer_solve_lstsq(new.as_ref(), y, self.lambda, true, self.solver)
        } else {
            faer_solve_lstsq(X, y, self.lambda, false, self.solver)
        };
        if self.fit_bias {
            let n = all_coefficients.nrows();
            let slice = all_coefficients.col_as_slice(0);
            self.coefficients = faer::mat::Mat::from_fn(n - 1, 1, |i, _| slice[i]);
            self.bias = slice[n - 1];
        } else {
            self.coefficients = all_coefficients;
        }
    }
}

/// A struct that handles online linear regression
pub struct OnlineLR {
    pub lambda: f64,
    pub fit_bias: bool,
    pub bias: f64,
    pub coefficients: Mat<f64>, // n_features x 1 matrix, doesn't contain bias
    pub inv: Mat<f64>,          // Current Inverse of X^t X
}

impl OnlineLR {
    pub fn new(lambda: f64, fit_bias: bool) -> Self {
        OnlineLR {
            lambda: lambda,
            fit_bias: fit_bias,
            bias: 0.,
            coefficients: Mat::new(),
            inv: Mat::new(),
        }
    }

    pub fn set_coeffs_bias_inverse(
        &mut self,
        coeffs: &[f64],
        bias: f64,
        inv: MatRef<f64>,
    ) -> Result<(), LinalgErrors> {
        if coeffs.len() != inv.ncols() {
            Err(LinalgErrors::DimensionMismatch)
        } else {
            self.bias = bias;
            self.coefficients = faer::mat::Mat::from_fn(coeffs.len(), 1, |i, _| coeffs[i]);
            self.inv = inv.to_owned();
            Ok(())
        }
    }

    pub fn get_inv(&self) -> Result<MatRef<f64>, LinalgErrors> {
        if self.inv.shape() == (0, 0) {
            Err(LinalgErrors::MatNotLearnedYet)
        } else {
            Ok(self.inv.as_ref())
        }
    }

    pub fn update_unchecked(&mut self, new_x: MatRef<f64>, new_y: MatRef<f64>, c: f64) {
        if self.fit_bias() {
            let cur_coeffs = self.coefficients();
            let ones = Mat::full(new_x.nrows(), 1, 1.0);
            let new_new_x = faer::concat![[new_x, ones]];
            // Need this because of dimension issue. Coefficients doesn't contain the bias term, but
            // during fit, it was included which resulted in +1 dimension.
            // We need to take care of this.
            let nfeats = cur_coeffs.nrows();
            let mut temp_weights = Mat::<f64>::from_fn(nfeats + 1, 1, |i, j| {
                if i < nfeats {
                    *cur_coeffs.get(i, j)
                } else {
                    self.bias
                }
            });
            woodbury_step(
                self.inv.as_mut(),
                temp_weights.as_mut(),
                new_new_x.as_ref(),
                new_y,
                c,
            );
            self.coefficients = temp_weights.get(..nfeats, ..).to_owned();
            self.bias = *temp_weights.get(nfeats, 0);
        } else {
            woodbury_step(
                self.inv.as_mut(),
                self.coefficients.as_mut(),
                new_x,
                new_y,
                c,
            )
        }
    }

    pub fn update(&mut self, new_x: MatRef<f64>, new_y: MatRef<f64>, c: f64) {
        if !(has_nan(new_x) || has_nan(new_y)) {
            self.update_unchecked(new_x, new_y, c)
        }
    }
}

impl LinearRegression for OnlineLR {
    fn coefficients(&self) -> MatRef<f64> {
        self.coefficients.as_ref()
    }

    fn bias(&self) -> f64 {
        self.bias
    }

    fn fit_bias(&self) -> bool {
        self.fit_bias
    }

    fn fit_unchecked(&mut self, X: MatRef<f64>, y: MatRef<f64>) {
        if self.fit_bias {
            let actual_features = X.ncols();
            let ones = Mat::full(X.nrows(), 1, 1.0);
            let new_x = faer::concat![[X, ones]];
            let (inv, all_coefficients) = 
                faer_qr_lstsq_with_inv(new_x.as_ref(), y, self.lambda, true);

            self.inv = inv;
            self.coefficients = all_coefficients.get(..actual_features, ..).to_owned();
            self.bias = *all_coefficients.get(actual_features, 0);
        } else {
            (self.inv, self.coefficients) = 
                faer_qr_lstsq_with_inv(X.as_ref(), y, self.lambda, true);
        }
    }
}

/// A struct that handles regular linear regression and Ridge regression.
pub struct ElasticNet {
    pub l1_reg: f64,
    pub l2_reg: f64,
    pub coefficients: Mat<f64>, // n_features x 1 matrix, doesn't contain bias
    pub fit_bias: bool,
    pub bias: f64,
    pub tol: f64,
    pub max_iter: usize,
}

impl ElasticNet {
    pub fn new(l1_reg: f64, l2_reg: f64, fit_bias: bool, tol: f64, max_iter: usize) -> Self {
        ElasticNet {
            l1_reg: l1_reg,
            l2_reg: l2_reg,
            coefficients: Mat::new(),
            fit_bias: fit_bias,
            bias: 0.,
            tol: tol,
            max_iter: max_iter,
        }
    }

    pub fn from_values(coeffs: &[f64], bias: f64) -> Self {
        ElasticNet {
            l1_reg: f64::NAN,
            l2_reg: f64::NAN,
            coefficients: Mat::from_fn(coeffs.len(), 1, |i, _| coeffs[i]),
            fit_bias: bias.abs() > f64::EPSILON,
            bias: bias,
            tol: 1e-5,
            max_iter: 2000,
        }
    }

    pub fn set_coeffs_and_bias(&mut self, coeffs: &[f64], bias: f64) {
        self.coefficients = Mat::from_fn(coeffs.len(), 1, |i, _| coeffs[i]);
        self.bias = bias;
        self.fit_bias = bias.abs() > f64::EPSILON;
    }

    pub fn regularizers(&self) -> (f64, f64) {
        (self.l1_reg, self.l2_reg)
    }
}

impl LinearRegression for ElasticNet {
    fn coefficients(&self) -> MatRef<f64> {
        self.coefficients.as_ref()
    }

    fn bias(&self) -> f64 {
        self.bias
    }

    fn fit_bias(&self) -> bool {
        self.fit_bias
    }

    fn fit_unchecked(&mut self, X: MatRef<f64>, y: MatRef<f64>) {
        let all_coefficients = if self.fit_bias {
            let ones = Mat::full(X.nrows(), 1, 1.0);
            let new_x = faer::concat![[X, ones]];
            faer_coordinate_descent(
                new_x.as_ref(),
                y,
                self.l1_reg,
                self.l2_reg,
                self.fit_bias,
                self.tol,
                self.max_iter,
            )
        } else {
            faer_coordinate_descent(
                X,
                y,
                self.l1_reg,
                self.l2_reg,
                self.fit_bias,
                self.tol,
                self.max_iter,
            )
        };

        if self.fit_bias {
            let n = all_coefficients.nrows();
            let slice = all_coefficients.col_as_slice(0);
            self.coefficients = faer::mat::Mat::from_fn(n - 1, 1, |i, _| *all_coefficients.get(i, 0));
            self.bias = slice[n - 1];
        } else {
            self.coefficients = all_coefficients;
        }
    }

    fn fit(&mut self, X: MatRef<f64>, y: MatRef<f64>) -> Result<(), LinalgErrors> {
        if X.nrows() != y.nrows() {
            return Err(LinalgErrors::DimensionMismatch);
        } else if X.nrows() == 0 || y.nrows() == 0 {
            return Err(LinalgErrors::NotEnoughData);
        } // Ok to have nrows < ncols
        self.fit_unchecked(X, y);
        Ok(())
    }
}

//------------------------------------ The Basic Functions ---------------------------------------

// #[inline(always)]
// pub fn faer_solve_lstsq<T:RealField>(
//     x: MatRef<T>, 
//     y: MatRef<T>,
//     how: LRSolverMethods
// ) -> Mat<T> {
//     let lhs = x.transpose() * x;
//     let rhs = x.transpose() * y;

//     match how {
//         LRSolverMethods::SVD => match lhs.thin_svd() {
//             Ok(l) => l.solve(rhs),
//             Err(_) => lhs.col_piv_qr().solve(rhs)
//         },
//         LRSolverMethods::QR => lhs.col_piv_qr().solve(rhs),
//         LRSolverMethods::Choleskey => todo!(),
//     }
// }


/// Least square that sets all singular values below threshold to 0.
/// Returns the coefficients and the singular values
#[inline(always)]
pub fn faer_solve_lstsq_rcond(x: MatRef<f64>, y: MatRef<f64>, rcond: f64) -> (Mat<f64>, Vec<f64>) {
    let xt = x.transpose();

    // Need work here

    let svd = (xt * x).thin_svd().unwrap();
    let s = svd.S().column_vector();

    let singular_values = s
        .iter()
        .copied()
        .map(f64::sqrt)
        .collect::<Vec<_>>();

    let n = singular_values.len();

    let max_singular_value = singular_values.iter().copied().fold(f64::MIN, f64::max);
    let threshold = rcond * max_singular_value;

    // Safe, because i <= n
    let mut s_inv = Mat::<f64>::zeros(n, n);
    unsafe {
        for (i, v) in s.iter().copied().enumerate() {
            *s_inv.get_mut_unchecked(i, i) = if v >= threshold { v.recip() } else { 0. };
        }
    }

    let weights = svd.V() * s_inv * svd.U().transpose() * xt * y;
    (weights, singular_values)
}

/// Least square that sets all singular values below threshold to 0.
/// Returns the coefficients and the singular values
#[inline(always)]
pub fn faer_solve_ridge_rcond(
    x: MatRef<f64>,
    y: MatRef<f64>,
    lambda: f64,
    has_bias: bool,
    rcond: f64,
) -> (Mat<f64>, Vec<f64>) {
    let n1 = x.ncols().abs_diff(has_bias as usize);
    let xt = x.transpose();
    let mut xtx_plus = xt * x;
    // xtx + diagonal of lambda. If has bias, last diagonal element is 0.
    // Safe. Index is valid and value is initialized.
    for i in 0..n1 {
        *unsafe { xtx_plus.get_mut_unchecked(i, i) } += lambda;
    }
    // need work here
    let svd = xtx_plus.thin_svd().unwrap();
    let s = svd.S().column_vector();
    let singular_values = s
        .iter()
        .copied()
        .map(f64::sqrt)
        .collect::<Vec<_>>();

    let n = singular_values.len();

    let max_singular_value = singular_values.iter().copied().fold(f64::MIN, f64::max);
    let threshold = rcond * max_singular_value;
    // Safe, because i <= n
    let mut s_inv = Mat::<f64>::zeros(n, n);
    unsafe {
        for (i, v) in s.iter().copied().enumerate() {
            *s_inv.get_mut_unchecked(i, i) = if v >= threshold { v.recip() } else { 0. };
        }
    }

    let weights = svd.V() * s_inv * svd.U().transpose() * xt * y;
    (weights, singular_values)
}

/// Returns the coefficients for lstsq with l2 (Ridge) regularization as a nrows x 1 matrix
/// If lambda is 0, then this is the regular lstsq
#[inline(always)]
pub fn faer_solve_lstsq<T: RealField + Copy>(
    x: MatRef<T>,
    y: MatRef<T>,
    lambda: T,
    has_bias: bool,
    how: LRSolverMethods,
) -> Mat<T> {
    // Add ridge SVD with rconditional number later.

    let n1 = x.ncols().abs_diff(has_bias as usize);
    let xt = x.transpose();
    let mut xtx = xt * x;
    // xtx + diagonal of lambda. If has bias, last diagonal element is 0.
    // Safe. Index is valid and value is initialized.
    if lambda >= T::zero() && n1 >= 1 {
        unsafe {
            for i in 0..n1 {
                *xtx.get_mut_unchecked(i, i) = *xtx.get_mut_unchecked(i, i) + lambda; 
            }
        }
    }

    match how {
        LRSolverMethods::SVD => {
            match xtx.thin_svd() {
                Ok(svd) => svd.solve(xt * y),
                _ => xtx.col_piv_qr().solve(xt * y)
            }
        },
        LRSolverMethods::QR => xtx.col_piv_qr().solve(xt * y),
        LRSolverMethods::Choleskey => todo!(),
    }
}

/// Returns the coefficients for lstsq as a nrows x 1 matrix together with the inverse of XtX
/// The uses QR (column pivot) decomposition as default method to compute inverse,
/// Column Pivot QR is chosen to deal with rank deficient cases. It is also slightly
/// faster compared to other methods.
#[inline(always)]
pub fn faer_qr_lstsq_with_inv<T:RealField + Copy>(
    x: MatRef<T>, 
    y: MatRef<T>,
    lambda: T,
    has_bias: bool,
) -> (Mat<T>, Mat<T>) {

    let n1 = x.ncols().abs_diff(has_bias as usize);
    let xt = x.transpose();
    let mut xtx = xt * x;

    if lambda > T::zero() && n1 >= 1 {
        unsafe {
            for i in 0..n1 {
                *xtx.get_mut_unchecked(i, i) = *xtx.get_mut_unchecked(i, i) + lambda;
            }
        }
    }

    let qr = xtx.col_piv_qr();
    let inv = qr.inverse();
    let weights = qr.solve(xt * y);
    
    (inv, weights)
}


/// Solves the weighted least square with weights given by the user
#[inline(always)]
pub fn faer_weighted_lstsq<T: RealField>(
    x: MatRef<T>,
    y: MatRef<T>,
    w: &[T],
    how: LRSolverMethods,
) -> Mat<T> {
    
    let weights = faer::ColRef::from_slice(w);
    let w = weights.as_diagonal();

    let xt = x.transpose();
    let xtw = xt * w;
    let xtwx = &xtw * x;
    match how {
        LRSolverMethods::SVD => {
            match xtwx.thin_svd() {
                Ok(svd) => svd.solve(xtw * y),
                Err(_) => xtwx.col_piv_qr().solve(xtw * y),
            }
        }
        LRSolverMethods::QR => xtwx.col_piv_qr().solve(xtw * y),
        LRSolverMethods::Choleskey => todo!()
    }
}

#[inline(always)]
fn soft_threshold_l1(z: f64, lambda: f64) -> f64 {
    z.signum() * (z.abs() - lambda).max(0f64)
}

/// Computes Lasso/Elastic Regression coefficients by the use of Coordinate Descent.
/// The current stopping criterion is based on L Inf norm of the changes in the
/// coordinates. A better alternative might be the dual gap.
///
/// Reference:
/// https://xavierbourretsicotte.github.io/lasso_implementation.html
/// https://www.stat.cmu.edu/~ryantibs/convexopt-F18/lectures/coord-desc.pdf
/// https://github.com/minatosato/Lasso/blob/master/coordinate_descent_lasso.py
/// https://en.wikipedia.org/wiki/Lasso_(statistics)
#[inline(always)]
pub fn faer_coordinate_descent(
    x: MatRef<f64>,
    y: MatRef<f64>,
    l1_reg: f64,
    l2_reg: f64,
    has_bias: bool,
    tol: f64,
    max_iter: usize,
) -> Mat<f64> {
    let m = x.nrows() as f64;
    let ncols = x.ncols();
    let n1 = ncols.abs_diff(has_bias as usize);

    let lambda_l1 = m * l1_reg;

    let mut beta: Mat<f64> = Mat::zeros(ncols, 1);
    let mut converge = false;

    // compute column squared l2 norms.
    // (In the case of Elastic net, squared l2 norms + l2 regularization factor)
    let norms = x
        .col_iter()
        .map(|c| c.squared_norm_l2() + m * l2_reg)
        .collect::<Vec<_>>();

    let xty = x.transpose() * y;
    let xtx = x.transpose() * x;

    // Random selection often leads to faster convergence?
    for _ in 0..max_iter {
        let mut max_change = 0f64;
        for j in 0..n1 {
            // temporary set beta(j, 0) to 0.
            // Safe. The index is valid and the value is initialized.
            let before = *unsafe { beta.get_unchecked(j, 0) };
            *unsafe { beta.get_mut_unchecked(j, 0) } = 0f64;
            let xtx_j = unsafe { xtx.get_unchecked(j..j + 1, ..) };

            // Xi^t(y - X-i Beta-i)
            let main_update = xty.get(j, 0) - (xtx_j * &beta).get(0, 0);

            // update beta(j, 0).
            let after = soft_threshold_l1(main_update, lambda_l1) / norms[j];
            *unsafe { beta.get_mut_unchecked(j, 0) } = after;
            max_change = (after - before).abs().max(max_change);
        }
        // if has_bias, n1 = last index = ncols - 1 = column of bias. If has_bias is False, n = ncols
        if has_bias {
            // Safe. The index is valid and the value is initialized.
            let xx = unsafe { x.get_unchecked(.., 0..n1) };
            let bb = unsafe { beta.get_unchecked(0..n1, ..) };
            let ss = (y - xx * bb).as_ref().sum() / m;
            *unsafe { beta.get_mut_unchecked(n1, 0) } = ss;
        }
        converge = max_change < tol;
        if converge {
            break;
        }
    }

    if !converge {
        println!(
            "Lasso regression: Max number of iterations have passed and result hasn't converged."
        )
    }

    beta
}

/// Given all data, we start running a lstsq starting at position n and compute new coefficients recurisively.
/// This will return all coefficients for rows >= n. This will only be used in Polars Expressions.
pub fn faer_recursive_lstsq(
    x: MatRef<f64>,
    y: MatRef<f64>,
    n: usize,
    lambda: f64,
) -> Vec<Mat<f64>> {
    let xn = x.nrows();
    // x: size xn x m
    // y: size xn x 1
    // Vector of matrix of size m x 1
    let mut coefficients = Vec::with_capacity(xn - n + 1);
    // n >= 2, guaranteed by Python
    let x0 = x.get(..n, ..);
    let y0 = y.get(..n, ..);

    // This is because if add_bias, the 1 is added to
    // all data already. No need to let OnlineLR add the 1 for the user.
    let mut online_lr = OnlineLR::new(lambda, false);
    online_lr.fit_unchecked(x0, y0); // safe because things are checked in plugin / python functions.
    coefficients.push(online_lr.get_coefficients());
    for j in n..xn {
        let next_x = x.get(j..j + 1, ..); // 1 by m, m = # of columns
        let next_y = y.get(j..j + 1, ..); // 1 by 1
        online_lr.update(next_x, next_y, 1.0);
        coefficients.push(online_lr.get_coefficients());
    }
    coefficients
}

/// Given all data, we start running a lstsq starting at position n and compute new coefficients recurisively.
/// This will return all coefficients for rows >= n. This will only be used in Polars Expressions.
/// This supports Normal or Ridge regression
pub fn faer_rolling_lstsq(x: MatRef<f64>, y: MatRef<f64>, n: usize, lambda: f64) -> Vec<Mat<f64>> {
    let xn = x.nrows();
    // x: size xn x m
    // y: size xn x 1
    // Vector of matrix of size m x 1
    let mut coefficients = Vec::with_capacity(xn - n + 1); // xn >= n is checked in Python

    let x0 = x.get(..n, ..);
    let y0 = y.get(..n, ..);

    // This is because if add_bias, the 1 is added to
    // all data already. No need to let OnlineLR add the 1 for the user.
    let mut online_lr = OnlineLR::new(lambda, false);
    online_lr.fit_unchecked(x0, y0);
    coefficients.push(online_lr.get_coefficients());

    for j in n..xn {
        let remove_x = x.get(j - n..j - n + 1, ..);
        let remove_y = y.get(j - n..j - n + 1, ..);
        online_lr.update(remove_x, remove_y, -1.0);

        let next_x = x.get(j..j + 1, ..); // 1 by m, m = # of columns
        let next_y = y.get(j..j + 1, ..); // 1 by 1
        online_lr.update(next_x, next_y, 1.0);
        coefficients.push(online_lr.get_coefficients());
    }
    coefficients
}

/// Given all data, we start running a lstsq starting at position n and compute new coefficients recurisively.
/// This will return all coefficients for rows >= n. This will only be used in Polars Expressions.
/// If # of non-null rows in the window is < m, a Matrix with size (0, 0) will be returned.
/// This supports Normal or Ridge regression
pub fn faer_rolling_skipping_lstsq(
    x: MatRef<f64>,
    y: MatRef<f64>,
    n: usize,
    m: usize,
    lambda: f64,
) -> Vec<Mat<f64>> {
    let xn = x.nrows();
    let ncols = x.ncols();
    // x: size xn x m
    // y: size xn x 1
    // n is window size. m is min_window_size after skipping null rows. n >= m > 0.
    // Vector of matrix of size m x 1
    let mut coefficients = Vec::with_capacity(xn - n + 1); // xn >= n is checked in Python

    // Initialize the problem.
    let mut non_null_cnt_in_window = 0;
    let mut left = 0;
    let mut right = n;
    let mut x_slice: Vec<f64> = Vec::with_capacity(n * ncols);
    let mut y_slice: Vec<f64> = Vec::with_capacity(n);

    // This is because if add_bias, the 1 is added to
    // all data already. No need to let OnlineLR add the 1 for the user.
    let mut online_lr = OnlineLR::new(lambda, false);
    while right <= xn {
        // Somewhat redundant here.
        non_null_cnt_in_window = 0;
        x_slice.clear();
        y_slice.clear();
        for i in left..right {
            let x_i = x.get(i, ..);
            let y_i = y.get(i, ..);

            if !(x_i.iter().any(|x| is_nan(x)) | y_i.iter().any(|y| is_nan(y))) {
                non_null_cnt_in_window += 1;
                x_slice.extend(x_i.iter());
                y_slice.extend(y_i.iter());
            }
        }
        if non_null_cnt_in_window >= m {
            let x0 = MatRef::from_row_major_slice(&x_slice, y_slice.len(), ncols);
            // faer::mat::from_row_major_slice(&x_slice, y_slice.len(), ncols);
            let y0 = MatRef::from_column_major_slice(&y_slice, y_slice.len(), 1);
            // faer::mat::from_row_major_slice(&y_slice, y_slice.len(), 1);
            online_lr.fit_unchecked(x0, y0);
            coefficients.push(online_lr.get_coefficients());
            break;
        } else {
            left += 1;
            right += 1;
            coefficients.push(Mat::with_capacity(0, 0));
        }
    }

    if right >= xn {
        return coefficients;
    }
    // right < xn, the problem must have been initialized (inv and weights are defined.)
    for j in right..xn {
        let remove_x = x.get(j - n..j - n + 1, ..);
        let remove_y = y.get(j - n..j - n + 1, ..);
        
        if !(has_nan(remove_x) | has_nan(remove_y)) {
            non_null_cnt_in_window -= 1; // removed one non-null column
            online_lr.update_unchecked(remove_x, remove_y, -1.0); // No need to check for nan
        }

        let next_x = x.get(j..j + 1, ..); // 1 by m, m = # of columns
        let next_y = y.get(j..j + 1, ..); // 1 by 1
        if !(has_nan(next_x) | has_nan(next_y)) {
            non_null_cnt_in_window += 1;
            online_lr.update_unchecked(next_x, next_y, 1.0); // No need to check for nan
        }

        if non_null_cnt_in_window >= m {
            coefficients.push(online_lr.get_coefficients());
        } else {
            coefficients.push(Mat::with_capacity(0, 0));
        }
    }
    coefficients
}

/// Update the inverse and the weights for one step in a Woodbury update.
/// Reference: https://cpb-us-w2.wpmucdn.com/sites.gatech.edu/dist/2/436/files/2017/07/22-notes-6250-f16.pdf
/// https://en.wikipedia.org/wiki/Woodbury_matrix_identity
#[inline(always)]
fn woodbury_step(
    inverse: MatMut<f64>,
    weights: MatMut<f64>,
    new_x: MatRef<f64>,
    new_y: MatRef<f64>,
    c: f64, // +1 or -1, for a "update" and a "removal"
) {
    // It is truly amazing that the C in the Woodbury identity essentially controls the update and
    // and removal of a new record (rolling)... Linear regression seems to be designed by God to work so well

    let u = &inverse * new_x.transpose(); // corresponding to u in the reference
                                             // right = left.transpose() by the fact that if A is symmetric, invertible, A-1 is also symmetric
    let z = (c + (new_x * &u).get(0, 0)).recip();
    // Update the information matrix's inverse. Page 56 of the gatech reference
    faer::linalg::matmul::matmul(
        inverse,
        faer::Accum::Add,
        &u,
        &u.transpose(),
        z.neg(),
        Par::rayon(0), //
    ); // inv is updated

    // Difference from estimate using prior weights vs. actual next y
    let y_diff = new_y - (new_x * &weights);
    // Update weights. Page 56, after 'Then',.. in gatech reference
    faer::linalg::matmul::matmul(
        weights,
        faer::Accum::Add,
        y_diff,
        u,
        z,
        Par::rayon(0), //
    ); // weights are updated
}
