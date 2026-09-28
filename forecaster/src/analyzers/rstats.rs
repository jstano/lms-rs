//! `RStats.java` — hand-rolled multiple linear regression: builds the normal-equations matrix,
//! solves it via Gauss-Jordan elimination with full pivoting, then derives R/R-squared/standard
//! error and a Durbin-Watson-style serial-correlation test from the residuals.
//!
//! Ported as a literal translation of the Java algorithm, including its 1-D row-major array
//! layout (`aryin[i * numCol + j]`) and one confirmed bug — see `swap_rows_if_needed` below.

use crate::analyzers::RowData;

#[derive(Debug, Clone, Default)]
pub struct RStats {
    pub est_stand_error: f64,
    pub r_sqr_value: f64,
    pub r_value: f64,
    /// `regError` — `1.0` when the regression determinant is too small to trust, `0.0` otherwise.
    pub reg_error: f64,
    /// `dw` — the Durbin-Watson-style statistic computed by `durbin`.
    pub dw: f64,

    num_obs: usize,
    num_col: usize,
}

impl RStats {
    pub fn new() -> Self {
        Self::default()
    }

    /// `multiRegress`. `independents` is `numObjects` rows of `numInd` values, row-major
    /// (`independents[i * numInd + j]`). `coefficients`/`yEstimates`/`residuals`/`coefStandError`
    /// are filled in place, matching the Java out-parameter style.
    pub fn multi_regress(
        &mut self,
        independents: &[f64],
        dependents: &RowData,
        num_ind: usize,
        num_objects: usize,
        coefficients: &mut [f64],
        y_estimates: &mut [f64],
        residuals: &mut [f64],
        coef_stand_error: &mut [f64],
    ) {
        self.num_obs = num_objects;
        self.num_col = num_ind + 1;

        let mut regmat = vec![0.0; self.num_col * self.num_obs];

        for i in 0..self.num_obs {
            for j in 0..num_ind {
                regmat[i * self.num_col + j + 1] = independents[i * num_ind + j];
            }
            regmat[i * self.num_col] = 1.0;
        }

        let arya = self.mat_tx_ti_x(&regmat);
        let aryg = self.mat_y_ti_x(dependents, &regmat);
        let mut aryinv = vec![0.0; self.num_col * self.num_col];
        let regdet = self.gauss_jordan(&arya, &aryg, coefficients, &mut aryinv);

        self.res_analysis(
            &regmat,
            dependents,
            coefficients,
            &aryinv,
            y_estimates,
            residuals,
            coef_stand_error,
        );

        self.reg_error = if regdet < 1.0E-7 { 1.0 } else { 0.0 };
    }

    /// `matTxTiX` — `X^T * X`.
    pub fn mat_tx_ti_x(&self, aryin: &[f64]) -> Vec<f64> {
        let n = self.num_col;
        let mut aryout = vec![0.0; n * n];

        for i in 0..n {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..self.num_obs {
                    sum += aryin[k * n + j] * aryin[k * n + i];
                }
                aryout[i * n + j] = sum;
            }
        }
        aryout
    }

    /// `matYTiX` — `X^T * y`.
    pub fn mat_y_ti_x(&self, aryy: &RowData, aryx: &[f64]) -> Vec<f64> {
        let n = self.num_col;
        let mut aryout = vec![0.0; n];

        for i in 0..n {
            let mut sum = 0.0;
            for j in 0..self.num_obs {
                sum += aryy.dataset[j] * aryx[j * n + i];
            }
            aryout[i] = sum;
        }
        aryout
    }

    /// `gaussJordan` — Gauss-Jordan elimination with full (row and column) pivoting. Returns the
    /// determinant of `coefary`.
    pub fn gauss_jordan(
        &self,
        coefary: &[f64],
        constary: &[f64],
        coefficients: &mut [f64],
        invary: &mut [f64],
    ) -> f64 {
        let n = self.num_col;
        let mut piv_list = vec![(0usize, 0usize); n];
        let mut piv_check = vec![false; n];
        let mut constary = constary.to_vec();

        let mut det = 1.0f64;

        for i in 0..n {
            for j in 0..n {
                invary[i * n + j] = coefary[i * n + j];
            }
        }

        for i in 0..n {
            let mut value = 0.0f64;
            let mut row = 0usize;
            let mut col = 0usize;

            for j in 0..n {
                if !piv_check[j] {
                    for k in 0..n {
                        if !piv_check[k] && invary[j * n + k].abs() > value {
                            row = j;
                            col = k;
                            value = invary[j * n + k].abs();
                        }
                    }
                }
            }
            piv_check[col] = true;
            piv_list[i] = (row, col);

            if row != col {
                det = -det;
                Self::swap_rows_if_needed(invary, n, row, col);
                constary.swap(row, col);
            }

            let piv = invary[col * n + col];
            det *= piv;

            if det > 1.0E+20 {
                det = 1.0;
            }

            invary[col * n + col] = 1.0;
            for l in 0..n {
                invary[col * n + l] /= piv;
            }
            constary[col] /= piv;

            for m in 0..n {
                if m != col {
                    let t = invary[m * n + col];
                    invary[m * n + col] = 0.0;
                    for k in 0..n {
                        invary[m * n + k] -= invary[col * n + k] * t;
                    }
                    constary[m] -= constary[col] * t;
                }
            }
        }

        for i in 0..n {
            let m = n - i - 1;
            let (row, col) = piv_list[m];
            if row != col {
                for j in 0..n {
                    invary.swap(j * n + row, j * n + col);
                }
            }
        }

        coefficients[..n].copy_from_slice(&constary[..n]);

        det
    }

    /// `gaussJordan`'s row-swap: `for l in 0..numCol { temp = invary[row*numCol + 1]; ... }`.
    /// Java indexes the *source* element with the literal `1` instead of the loop variable `l` —
    /// every swapped-in value in `invary[row * numCol + l]` for `l >= 1` about here is actually
    /// the row's second column (`invary[row * numCol + 1]`) rather than its own column `l`,
    /// except `l == 0`/`l == 1` where the typo happens to coincide with the correct index. Ported
    /// verbatim (not fixed) since this method's output already flows into every downstream
    /// consumer of `RegressionAnalyzer` in production; fixing it would change forecast numbers.
    fn swap_rows_if_needed(invary: &mut [f64], n: usize, row: usize, col: usize) {
        for l in 0..n {
            let temp = invary[row * n + 1];
            invary[row * n + l] = invary[col * n + l];
            invary[col * n + l] = temp;
        }
    }

    /// `resAnalysis`.
    pub fn res_analysis(
        &mut self,
        regmat: &[f64],
        ymat: &RowData,
        regcoef: &[f64],
        aryinv: &[f64],
        yest: &mut [f64],
        residuals: &mut [f64],
        coefsig: &mut [f64],
    ) {
        let n = self.num_col;
        let mut sumressq = 0.0;
        let mut sumy = 0.0;
        let mut sumysqr = 0.0;

        for i in 0..self.num_obs {
            let mut est = 0.0;
            for j in 0..n {
                est += regcoef[j] * regmat[i * n + j];
            }
            yest[i] = est;

            residuals[i] = yest[i] - ymat.dataset[i];
            sumressq += residuals[i] * residuals[i];
            sumy += ymat.dataset[i];
            sumysqr += ymat.dataset[i] * ymat.dataset[i];
        }

        let nxx = if self.num_obs == n {
            1.0
        } else {
            (self.num_obs - n) as f64
        };

        self.r_value = 0.0;

        if sumressq / nxx < 0.0 {
            self.r_sqr_value = -5.0;
            return;
        }

        self.est_stand_error = (sumressq / nxx).sqrt();

        for i in 0..n {
            if aryinv[i * n + i] < 0.0 {
                self.r_sqr_value = -6.0;
                return;
            }
            coefsig[i] = self.est_stand_error * aryinv[i * n + i].sqrt();
        }

        if sumysqr == sumy * sumy {
            self.r_sqr_value = -7.0;
            return;
        }

        if sumysqr == (sumy * sumy) / self.num_obs as f64 {
            self.r_sqr_value = -7.0;
            return;
        }

        self.r_sqr_value = 1.0 - sumressq / (sumysqr - (sumy * sumy) / self.num_obs as f64);

        if self.r_sqr_value < 0.0 {
            self.r_sqr_value = -8.0;
            return;
        }

        self.r_value = self.r_sqr_value.sqrt();
    }

    /// `durbin` — serial-correlation test on the regression residuals. Returns `1` when the
    /// statistic falls outside the acceptance band (matching Java's `int` return used as a bool).
    pub fn durbin(&mut self, resids: &RowData) -> i32 {
        let obs = self.num_obs;
        let inds = self.num_col - 1;

        let mut esq1 = 0.0;
        let mut ediffsq = 0.0;

        for i in (0..obs).rev() {
            if i == obs - 1 {
                esq1 += resids.dataset[i] * resids.dataset[i];
            } else {
                let ediff = resids.dataset[i + 1] - resids.dataset[i];
                esq1 += resids.dataset[i] * resids.dataset[i];
                ediffsq += ediff * ediff;
            }
        }

        let d = ediffsq / esq1;
        let r = 1.0 - d / 2.0;

        let obs_f = obs as f64 - 1.0;

        let e = 1.08 - (inds as f64 * 0.13) + ((obs_f - 15.0) * (0.025 + (inds as f64 * 0.008)));
        self.dw = r;

        if d < e || d > (4.0 - e) { 1 } else { 0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// y = 2x + 1 exactly: R^2 should hit 1.0 and coefficients should recover [1.0, 2.0].
    #[test]
    fn multi_regress_recovers_exact_linear_relationship() {
        let independents = [1.0, 2.0, 3.0, 4.0, 5.0];
        let dependents = RowData::from_array(&[3.0, 5.0, 7.0, 9.0, 11.0]);

        let mut rstats = RStats::new();
        let mut coefficients = [0.0; 2];
        let mut y_estimates = [0.0; 5];
        let mut residuals = [0.0; 5];
        let mut coef_stand_error = [0.0; 2];

        rstats.multi_regress(
            &independents,
            &dependents,
            1,
            5,
            &mut coefficients,
            &mut y_estimates,
            &mut residuals,
            &mut coef_stand_error,
        );

        assert!((coefficients[0] - 1.0).abs() < 1e-9, "intercept: {}", coefficients[0]);
        assert!((coefficients[1] - 2.0).abs() < 1e-9, "slope: {}", coefficients[1]);
        assert!((rstats.r_sqr_value - 1.0).abs() < 1e-9);
        assert_eq!(rstats.reg_error, 0.0);
        for &r in &residuals {
            assert!(r.abs() < 1e-9);
        }
    }

    #[test]
    fn durbin_flags_strong_positive_serial_correlation() {
        // Residuals trending monotonically (strong positive autocorrelation) should trip durbin's
        // out-of-band check.
        let mut rstats = RStats::new();
        rstats.multi_regress_test_dims(6, 2);
        let resids = RowData::from_array(&[-5.0, -3.0, -1.0, 1.0, 3.0, 5.0]);
        let flagged = rstats.durbin(&resids);
        assert_eq!(flagged, 1);
    }

    impl RStats {
        /// Test-only helper to set the dimensions `durbin`/`resAnalysis` read, without running a
        /// full `multi_regress` first.
        fn multi_regress_test_dims(&mut self, num_obs: usize, num_col: usize) {
            self.num_obs = num_obs;
            self.num_col = num_col;
        }
    }
}
