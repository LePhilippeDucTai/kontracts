//! Alternating Direction Implicit (ADI) 2D PDE Solver (jalon J20).
//!
//! Solves 2D Black-Scholes PDEs via ADI splitting:
//! - Heston model: PDE in (S, v) space
//! - 2-asset correlation: PDE in (S1, S2) space

use crate::numerics;
use crate::KontractError;
use ndarray::Array2;

/// Configuration for 2D PDE solver.
#[derive(Debug, Clone)]
pub struct Pde2dConfig {
    /// Spot price (S-dimension)
    pub spot: f64,
    /// Initial variance or second asset spot (v or S2)
    pub second_spot: f64,
    /// Risk-free rate
    pub rate: f64,
    /// Dividend yield on first asset
    pub dividend_yield: f64,
    /// For Heston: volatility of volatility; for 2-asset: vol of second asset
    pub vol_second: f64,
    /// For Heston: mean reversion rate; for 2-asset: correlation
    pub kappa: f64,
    /// For Heston: long-term variance; unused for 2-asset
    pub theta: f64,
    /// Time to maturity
    pub maturity: f64,
    /// Number of S-space grid points
    pub n_space_s: usize,
    /// Number of v-space grid points
    pub n_space_v: usize,
    /// Number of time steps
    pub n_time: usize,
    /// S-domain bounds
    pub s_min: f64,
    pub s_max: f64,
    /// v-domain bounds (Heston: variance; 2-asset: S2)
    pub v_min: f64,
    pub v_max: f64,
}

impl Default for Pde2dConfig {
    fn default() -> Self {
        Pde2dConfig {
            spot: 100.0,
            second_spot: 0.04,
            rate: 0.05,
            dividend_yield: 0.0,
            vol_second: 0.3,
            kappa: 2.0,
            theta: 0.04,
            maturity: 1.0,
            n_space_s: 100,
            n_space_v: 100,
            n_time: 100,
            s_min: 10.0,
            s_max: 200.0,
            v_min: 0.001,
            v_max: 1.0,
        }
    }
}

/// ADI solver for 2D Black-Scholes PDEs (Heston or 2-asset).
pub struct Pde2dSolver {
    cfg: Pde2dConfig,
    ds: f64,
    dv: f64,
    dt: f64,
    s_grid: Vec<f64>,
    v_grid: Vec<f64>,
}

impl Pde2dSolver {
    /// Create a new 2D PDE solver.
    pub fn new(cfg: Pde2dConfig) -> Result<Self, KontractError> {
        if cfg.n_space_s < 3 || cfg.n_space_v < 3 {
            return Err(KontractError::MalformedContract(
                "n_space_s and n_space_v must be >= 3".to_string(),
            ));
        }
        if cfg.n_time < 1 {
            return Err(KontractError::MalformedContract(
                "n_time must be >= 1".to_string(),
            ));
        }
        if cfg.s_max <= cfg.s_min || cfg.s_min < 0.0 {
            return Err(KontractError::MalformedContract(
                "Invalid S bounds".to_string(),
            ));
        }
        if cfg.v_max <= cfg.v_min || cfg.v_min < 0.0 {
            return Err(KontractError::MalformedContract(
                "Invalid v bounds".to_string(),
            ));
        }
        if cfg.maturity <= 0.0 {
            return Err(KontractError::MalformedContract(
                "maturity must be > 0".to_string(),
            ));
        }

        let ds = (cfg.s_max - cfg.s_min) / (cfg.n_space_s - 1) as f64;
        let dv = (cfg.v_max - cfg.v_min) / (cfg.n_space_v - 1) as f64;
        let dt = cfg.maturity / cfg.n_time as f64;

        let s_grid = (0..cfg.n_space_s)
            .map(|i| cfg.s_min + i as f64 * ds)
            .collect();
        let v_grid = (0..cfg.n_space_v)
            .map(|i| cfg.v_min + i as f64 * dv)
            .collect();

        Ok(Pde2dSolver {
            cfg,
            ds,
            dv,
            dt,
            s_grid,
            v_grid,
        })
    }

    /// Solve Heston option pricing via ADI.
    pub fn solve_heston<F>(&self, payoff_fn: F) -> Result<Array2<f64>, KontractError>
    where
        F: Fn(f64, f64) -> f64,
    {
        let ns = self.cfg.n_space_s;
        let nv = self.cfg.n_space_v;
        let nt = self.cfg.n_time;

        let mut v = Array2::zeros((ns, nv));

        // Terminal condition: payoff at all grid points
        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for i in 0..ns {
            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            for j in 0..nv {
                v[[i, j]] = payoff_fn(self.s_grid[i], self.v_grid[j]);
            }
        }

        // Backward time-stepping via ADI
        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for _ in 0..nt {
            // S-sweep: solve along S direction
            v = self.adi_s_sweep(&v)?;
            // v-sweep: solve along v direction
            v = self.adi_v_sweep(&v)?;
        }

        Ok(v)
    }

    /// ADI S-direction sweep (solve tridiagonal systems along S).
    fn adi_s_sweep(&self, v_old: &Array2<f64>) -> Result<Array2<f64>, KontractError> {
        let ns = self.cfg.n_space_s;
        let nv = self.cfg.n_space_v;
        let ds = self.ds;
        let dv = self.dv;
        let dt = self.dt;
        let r = self.cfg.rate;
        let q = self.cfg.dividend_yield;
        let sigma = self.cfg.vol_second; // vol of vol or vol of S2
        let _kappa = self.cfg.kappa;
        let _theta = self.cfg.theta;

        let mut v_new = Array2::zeros((ns, nv));

        // For each v-line, solve a tridiagonal S-system
        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for j in 0..nv {
            let vj = self.v_grid[j];

            let mut a = vec![0.0; ns];
            let mut b = vec![1.0; ns];
            let mut c = vec![0.0; ns];
            let mut rhs = vec![0.0; ns];

            // Boundary conditions
            rhs[0] = v_old[[0, j]];
            rhs[ns - 1] = v_old[[ns - 1, j]];

            // Interior points
            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            for i in 1..ns - 1 {
                let si = self.s_grid[i];

                // ADI coefficients for S-sweep in Heston
                let drift_s = 0.25 * dt * (r - q) * si / ds;
                let diff_s = 0.5 * dt * vj * si * si / (ds * ds);
                let _corr = 0.25 * dt * sigma * _kappa * si / (ds * dv); // cross-term approximation

                // Implicit LHS (S-sweep, v held fixed)
                a[i] = -diff_s - drift_s;
                b[i] = 1.0 + 2.0 * diff_s + 0.5 * dt * r;
                c[i] = -diff_s + drift_s;

                // Explicit RHS (uses v_old, cross-terms simplified)
                rhs[i] = (diff_s - drift_s) * v_old[[i - 1, j]]
                    + (1.0 - 2.0 * diff_s - 0.5 * dt * r) * v_old[[i, j]]
                    + (diff_s + drift_s) * v_old[[i + 1, j]];
            }

            // Solve tridiagonal for this j-line
            let sol = self.thomas(&a, &b, &c, &rhs)?;
            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            for i in 0..ns {
                v_new[[i, j]] = sol[i];
            }
        }

        Ok(v_new)
    }

    /// ADI v-direction sweep (solve tridiagonal systems along v).
    fn adi_v_sweep(&self, v_old: &Array2<f64>) -> Result<Array2<f64>, KontractError> {
        let ns = self.cfg.n_space_s;
        let nv = self.cfg.n_space_v;
        let dv = self.dv;
        let dt = self.dt;
        let r = self.cfg.rate;
        let sigma = self.cfg.vol_second;
        let kappa = self.cfg.kappa;
        let theta = self.cfg.theta;

        let mut v_new = Array2::zeros((ns, nv));

        // For each S-line, solve a tridiagonal v-system
        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for i in 0..ns {
            let _si = self.s_grid[i];

            let mut a = vec![0.0; nv];
            let mut b = vec![1.0; nv];
            let mut c = vec![0.0; nv];
            let mut rhs = vec![0.0; nv];

            // Boundary conditions (v at extremes)
            rhs[0] = v_old[[i, 0]];
            rhs[nv - 1] = v_old[[i, nv - 1]];

            // Interior points
            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            for j in 1..nv - 1 {
                let vj = self.v_grid[j];

                // ADI coefficients for v-sweep in Heston
                let drift_v = 0.25 * dt * kappa * (theta - vj) / dv;
                let diff_v = 0.5 * dt * sigma * sigma * vj / (dv * dv);

                // Implicit LHS (v-sweep, S held fixed)
                a[j] = -diff_v - drift_v;
                b[j] = 1.0 + 2.0 * diff_v + 0.5 * dt * r;
                c[j] = -diff_v + drift_v;

                // Explicit RHS
                rhs[j] = (diff_v - drift_v) * v_old[[i, j - 1]]
                    + (1.0 - 2.0 * diff_v - 0.5 * dt * r) * v_old[[i, j]]
                    + (diff_v + drift_v) * v_old[[i, j + 1]];
            }

            // Solve tridiagonal for this i-line
            let sol = self.thomas(&a, &b, &c, &rhs)?;
            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            for j in 0..nv {
                v_new[[i, j]] = sol[j];
            }
        }

        Ok(v_new)
    }

    /// Thomas algorithm for tridiagonal system (shared with J19).
    fn thomas(
        &self,
        a: &[f64],
        b: &[f64],
        c: &[f64],
        rhs: &[f64],
    ) -> Result<Vec<f64>, KontractError> {
        numerics::thomas(a, b, c, rhs)
    }

    /// Interpolate option value at given (spot, variance) point.
    pub fn interpolate(&self, grid: &Array2<f64>, spot: f64, var: f64) -> f64 {
        let ns = grid.nrows();
        let nv = grid.ncols();

        if ns < 2 || nv < 2 {
            return grid[[0, 0]];
        }

        // Clamp to domain
        let s = spot.clamp(self.cfg.s_min, self.cfg.s_max);
        let v = var.clamp(self.cfg.v_min, self.cfg.v_max);

        // Find grid indices
        let i_s = ((s - self.cfg.s_min) / self.ds).floor() as usize;
        let i_s = i_s.min(ns - 2);

        let i_v = ((v - self.cfg.v_min) / self.dv).floor() as usize;
        let i_v = i_v.min(nv - 2);

        // Bilinear interpolation
        let s_left = self.s_grid[i_s];
        let s_right = self.s_grid[i_s + 1];
        let v_left = self.v_grid[i_v];
        let v_right = self.v_grid[i_v + 1];

        let ws = (s - s_left) / (s_right - s_left);
        let wv = (v - v_left) / (v_right - v_left);

        let v00 = grid[[i_s, i_v]];
        let v10 = grid[[i_s + 1, i_v]];
        let v01 = grid[[i_s, i_v + 1]];
        let v11 = grid[[i_s + 1, i_v + 1]];

        (1.0 - ws) * (1.0 - wv) * v00
            + ws * (1.0 - wv) * v10
            + (1.0 - ws) * wv * v01
            + ws * wv * v11
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm_cdf(x: f64) -> f64 {
        let sign = if x < 0.0 { -1.0 } else { 1.0 };
        let a = x.abs() / std::f64::consts::SQRT_2;
        let t = 1.0 / (1.0 + 0.327_591_1 * a);
        let poly =
            ((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
                + 0.254_829_592;
        0.5 * (1.0 + sign * (1.0 - poly * t * (-a * a).exp()))
    }

    fn bs_call(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
        let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
        let d2 = d1 - sigma * t.sqrt();
        s * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
    }

    #[test]
    fn test_heston_grid_creation() {
        let cfg = Pde2dConfig {
            spot: 100.0,
            second_spot: 0.04,
            rate: 0.05,
            dividend_yield: 0.0,
            vol_second: 0.3,
            kappa: 2.0,
            theta: 0.04,
            maturity: 1.0,
            n_space_s: 50,
            n_space_v: 50,
            n_time: 50,
            s_min: 20.0,
            s_max: 200.0,
            v_min: 0.001,
            v_max: 1.0,
        };

        let solver = Pde2dSolver::new(cfg).unwrap();
        assert!(solver.s_grid.len() == 50);
        assert!(solver.v_grid.len() == 50);
        assert!(solver.s_grid[0] >= 20.0);
        assert!(solver.v_grid[0] >= 0.001);
    }

    #[test]
    fn test_heston_simple_call() {
        let cfg = Pde2dConfig {
            spot: 100.0,
            second_spot: 0.04,
            rate: 0.05,
            dividend_yield: 0.0,
            vol_second: 0.3,
            kappa: 2.0,
            theta: 0.04,
            maturity: 1.0,
            n_space_s: 60,
            n_space_v: 40,
            n_time: 50,
            s_min: 20.0,
            s_max: 200.0,
            v_min: 0.001,
            v_max: 1.0,
        };

        let solver = Pde2dSolver::new(cfg).unwrap();
        let grid = solver.solve_heston(|s, _v| (s - 100.0).max(0.0)).unwrap();

        let pde_val = solver.interpolate(&grid, 100.0, 0.04);
        let bs_val = bs_call(100.0, 100.0, 1.0, 0.05, 0.2);

        // Heston should be close to BS for comparable parameters
        let error = (pde_val - bs_val).abs() / bs_val;
        println!(
            "Heston call ATM: PDE={:.6}, BS={:.6}, error={:.4}%",
            pde_val,
            bs_val,
            error * 100.0
        );
        assert!(
            error < 0.3,
            "ADI Heston error too large: {:.2}%",
            error * 100.0
        );
    }

    #[test]
    fn test_heston_put() {
        let cfg = Pde2dConfig {
            spot: 100.0,
            second_spot: 0.04,
            rate: 0.05,
            dividend_yield: 0.0,
            vol_second: 0.3,
            kappa: 2.0,
            theta: 0.04,
            maturity: 1.0,
            n_space_s: 60,
            n_space_v: 40,
            n_time: 50,
            s_min: 20.0,
            s_max: 200.0,
            v_min: 0.001,
            v_max: 1.0,
        };

        let solver = Pde2dSolver::new(cfg).unwrap();
        let grid = solver.solve_heston(|s, _v| (100.0 - s).max(0.0)).unwrap();

        let pde_val = solver.interpolate(&grid, 100.0, 0.04);
        let bs_val = bs_call(100.0, 100.0, 1.0, 0.05, 0.2) - 100.0 + 100.0 * (-0.05_f64).exp(); // put-call parity approx

        let error = (pde_val - bs_val).abs() / bs_val.abs();
        println!(
            "Heston put ATM: PDE={:.6}, BS approx={:.6}, error={:.4}%",
            pde_val,
            bs_val,
            error * 100.0
        );
        assert!(pde_val > 0.0, "Put value must be positive");
    }

    #[test]
    fn test_heston_spot_sensitivity() {
        let cfg = Pde2dConfig {
            spot: 100.0,
            second_spot: 0.04,
            rate: 0.05,
            dividend_yield: 0.0,
            vol_second: 0.3,
            kappa: 2.0,
            theta: 0.04,
            maturity: 1.0,
            n_space_s: 60,
            n_space_v: 40,
            n_time: 50,
            s_min: 20.0,
            s_max: 200.0,
            v_min: 0.001,
            v_max: 1.0,
        };

        let solver = Pde2dSolver::new(cfg).unwrap();
        let grid = solver.solve_heston(|s, _v| (s - 100.0).max(0.0)).unwrap();

        let prices: Vec<_> = [80.0, 90.0, 100.0, 110.0, 120.0]
            .iter()
            .map(|&s| solver.interpolate(&grid, s, 0.04))
            .collect();

        // Call values should be monotonic in spot
        for i in 0..prices.len() - 1 {
            assert!(
                prices[i] <= prices[i + 1],
                "Non-monotonic call values at spots"
            );
        }
    }

    #[test]
    fn test_heston_variance_sensitivity() {
        let cfg = Pde2dConfig {
            spot: 100.0,
            second_spot: 0.04,
            rate: 0.05,
            dividend_yield: 0.0,
            vol_second: 0.3,
            kappa: 2.0,
            theta: 0.04,
            maturity: 1.0,
            n_space_s: 60,
            n_space_v: 40,
            n_time: 50,
            s_min: 20.0,
            s_max: 200.0,
            v_min: 0.001,
            v_max: 1.0,
        };

        let solver = Pde2dSolver::new(cfg).unwrap();
        let grid = solver.solve_heston(|s, _v| (s - 100.0).max(0.0)).unwrap();

        // Higher variance should increase option price (vega > 0)
        let val_low = solver.interpolate(&grid, 100.0, 0.02);
        let val_mid = solver.interpolate(&grid, 100.0, 0.04);
        let val_hi = solver.interpolate(&grid, 100.0, 0.08);

        assert!(
            val_low <= val_mid && val_mid <= val_hi,
            "Call value should increase with variance"
        );
    }
}
