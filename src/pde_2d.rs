//! Alternating Direction Implicit (ADI) 2D PDE Solver (jalon J20).
//!
//! Résout l'EDP de Heston 2D `(S, v)` par **schéma de Douglas** (θ = ½) : un
//! prédicteur d'Euler explicite (opérateur complet, terme croisé inclus) suivi de
//! deux corrections implicites unidirectionnelles (balayages S puis v),
//! tridiagonales (Thomas). Le terme croisé `ρσvS·∂²V/∂S∂v` est traité
//! **explicitement** (stencil central 4 points).
//!
//! **Limites assumées** (documentées suite à la revue Opus) :
//! - le terme croisé explicite rend le schéma *conditionnellement* stable
//!   (restriction de type CFL) : valable pour les grilles testées (|ρ| ≤ 0.5,
//!   σ ≤ 0.3), à raffiner si `|ρ|` est proche de 1 ou la grille `v` grossière ;
//! - la convection `κ(θ−v)V_v` près de `v_min` n'est pas *upwindée* : pour des
//!   régimes `v` faible / `κ` élevé (calibration extrême), un schéma upwind
//!   durcirait la monotonie. `v_min > 0` évite la dégénérescence en `v = 0`.

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
    /// Spot/variance correlation `ρ` (Heston leverage). Drives the mixed
    /// derivative `ρσvS·∂²V/∂S∂v`, traité explicitement dans le schéma de Douglas.
    pub rho: f64,
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
            rho: -0.5,
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
        if cfg.n_space_s < 4 || cfg.n_space_v < 4 {
            return Err(KontractError::MalformedContract(
                "n_space_s and n_space_v must be >= 4".to_string(),
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

    /// Price une option Heston par **schéma de Douglas ADI** (θ = ½), avec terme
    /// croisé `ρσvS·∂²V/∂S∂v` traité **explicitement** :
    ///
    /// ```text
    /// Y0 = V + Δt·A·V                    (Euler explicite, opérateur complet)
    /// (I − θΔt·A_S) Y1 = Y0 − θΔt·A_S·V  (correction implicite en S)
    /// (I − θΔt·A_v) V' = Y1 − θΔt·A_v·V  (correction implicite en v)
    /// ```
    /// où `A = A_S + A_v + A_mixte`, le terme de discount `−rV` est réparti pour
    /// moitié dans `A_S` et `A_v`. Conditions aux bords par linéarité (Γ = 0).
    pub fn solve_heston<F>(&self, payoff_fn: F) -> Result<Array2<f64>, KontractError>
    where
        F: Fn(f64, f64) -> f64,
    {
        let ns = self.cfg.n_space_s;
        let nv = self.cfg.n_space_v;
        let nt = self.cfg.n_time;

        // Condition terminale : payoff sur toute la grille.
        let mut v =
            Array2::from_shape_fn((ns, nv), |(i, j)| payoff_fn(self.s_grid[i], self.v_grid[j]));

        // noyau numérique : marche arrière en temps (récurrence).
        for _ in 0..nt {
            let a_s = self.apply_s(&v);
            let a_v = self.apply_v(&v);
            let a_mix = self.apply_mixed(&v);

            // Y0 = V + Δt (A_S + A_v + A_mixte) V.
            let y0 = Array2::from_shape_fn((ns, nv), |(i, j)| {
                v[[i, j]] + self.dt * (a_s[[i, j]] + a_v[[i, j]] + a_mix[[i, j]])
            });

            // Correction implicite S : (I − θΔt A_S) Y1 = Y0 − θΔt A_S V.
            let theta = 0.5;
            let rhs_s = Array2::from_shape_fn((ns, nv), |(i, j)| {
                y0[[i, j]] - theta * self.dt * a_s[[i, j]]
            });
            let y1 = self.solve_s_implicit(&rhs_s, theta)?;

            // Correction implicite v : (I − θΔt A_v) V' = Y1 − θΔt A_v V.
            let rhs_v = Array2::from_shape_fn((ns, nv), |(i, j)| {
                y1[[i, j]] - theta * self.dt * a_v[[i, j]]
            });
            v = self.solve_v_implicit(&rhs_v, theta)?;
        }

        Ok(v)
    }

    /// Opérateur S : `½vS²V_SS + (r−q)S V_S − ½rV` (intérieur ; 0 aux bords).
    fn apply_s(&self, v: &Array2<f64>) -> Array2<f64> {
        let (ns, nv) = (self.cfg.n_space_s, self.cfg.n_space_v);
        let (ds, r, q) = (self.ds, self.cfg.rate, self.cfg.dividend_yield);
        let mut out = Array2::zeros((ns, nv));
        // noyau numérique : stencils différences finies.
        for i in 1..ns - 1 {
            let si = self.s_grid[i];
            for j in 0..nv {
                let vj = self.v_grid[j];
                let v_ss = (v[[i + 1, j]] - 2.0 * v[[i, j]] + v[[i - 1, j]]) / (ds * ds);
                let v_s = (v[[i + 1, j]] - v[[i - 1, j]]) / (2.0 * ds);
                out[[i, j]] = 0.5 * vj * si * si * v_ss + (r - q) * si * v_s - 0.5 * r * v[[i, j]];
            }
        }
        out
    }

    /// Opérateur v : `½σ²v V_vv + κ(θ−v) V_v − ½rV` (intérieur ; 0 aux bords).
    fn apply_v(&self, v: &Array2<f64>) -> Array2<f64> {
        let (ns, nv) = (self.cfg.n_space_s, self.cfg.n_space_v);
        let (dv, r) = (self.dv, self.cfg.rate);
        let (sigma, kappa, theta) = (self.cfg.vol_second, self.cfg.kappa, self.cfg.theta);
        let mut out = Array2::zeros((ns, nv));
        // noyau numérique : stencils différences finies.
        for j in 1..nv - 1 {
            let vj = self.v_grid[j];
            for i in 0..ns {
                let v_vv = (v[[i, j + 1]] - 2.0 * v[[i, j]] + v[[i, j - 1]]) / (dv * dv);
                let v_v = (v[[i, j + 1]] - v[[i, j - 1]]) / (2.0 * dv);
                out[[i, j]] = 0.5 * sigma * sigma * vj * v_vv + kappa * (theta - vj) * v_v
                    - 0.5 * r * v[[i, j]];
            }
        }
        out
    }

    /// Terme croisé `ρσvS·∂²V/∂S∂v` (stencil central 4 points), explicite.
    fn apply_mixed(&self, v: &Array2<f64>) -> Array2<f64> {
        let (ns, nv) = (self.cfg.n_space_s, self.cfg.n_space_v);
        let (ds, dv) = (self.ds, self.dv);
        let (rho, sigma) = (self.cfg.rho, self.cfg.vol_second);
        let mut out = Array2::zeros((ns, nv));
        // noyau numérique : stencil mixte central.
        for i in 1..ns - 1 {
            let si = self.s_grid[i];
            for j in 1..nv - 1 {
                let vj = self.v_grid[j];
                let v_sv = (v[[i + 1, j + 1]] - v[[i + 1, j - 1]] - v[[i - 1, j + 1]]
                    + v[[i - 1, j - 1]])
                    / (4.0 * ds * dv);
                out[[i, j]] = rho * sigma * vj * si * v_sv;
            }
        }
        out
    }

    /// Résout `(I − θΔt A_S) X = rhs` colonne par colonne (tridiagonal en S),
    /// condition de bord par linéarité (Γ = 0) repliée dans le système.
    fn solve_s_implicit(
        &self,
        rhs: &Array2<f64>,
        theta: f64,
    ) -> Result<Array2<f64>, KontractError> {
        let (ns, nv) = (self.cfg.n_space_s, self.cfg.n_space_v);
        let (ds, dt, r, q) = (self.ds, self.dt, self.cfg.rate, self.cfg.dividend_yield);
        let mut out = Array2::zeros((ns, nv));
        // noyau numérique : un système tridiagonal par colonne v.
        for j in 0..nv {
            let vj = self.v_grid[j];
            let mut a = vec![0.0; ns];
            let mut b = vec![1.0; ns];
            let mut c = vec![0.0; ns];
            let mut d = vec![0.0; ns];
            for i in 1..ns - 1 {
                let si = self.s_grid[i];
                let ds2 = 0.5 * vj * si * si / (ds * ds);
                let drs = (r - q) * si / (2.0 * ds);
                a[i] = -theta * dt * (ds2 - drs);
                b[i] = 1.0 + theta * dt * (2.0 * ds2 + 0.5 * r);
                c[i] = -theta * dt * (ds2 + drs);
                d[i] = rhs[[i, j]];
            }
            // Linéarité (Γ = 0) : repli dans les lignes 1 et ns−2.
            b[1] += 2.0 * a[1];
            c[1] -= a[1];
            a[1] = 0.0;
            b[ns - 2] += 2.0 * c[ns - 2];
            a[ns - 2] -= c[ns - 2];
            c[ns - 2] = 0.0;

            let sol = self.thomas(&a, &b, &c, &d)?;
            for i in 1..ns - 1 {
                out[[i, j]] = sol[i];
            }
            out[[0, j]] = 2.0 * out[[1, j]] - out[[2, j]];
            out[[ns - 1, j]] = 2.0 * out[[ns - 2, j]] - out[[ns - 3, j]];
        }
        Ok(out)
    }

    /// Résout `(I − θΔt A_v) X = rhs` ligne par ligne (tridiagonal en v),
    /// condition de bord par linéarité (Γ = 0).
    fn solve_v_implicit(
        &self,
        rhs: &Array2<f64>,
        theta: f64,
    ) -> Result<Array2<f64>, KontractError> {
        let (ns, nv) = (self.cfg.n_space_s, self.cfg.n_space_v);
        let (dv, dt, r) = (self.dv, self.dt, self.cfg.rate);
        let (sigma, kappa, th) = (self.cfg.vol_second, self.cfg.kappa, self.cfg.theta);
        let mut out = Array2::zeros((ns, nv));
        // noyau numérique : un système tridiagonal par ligne S.
        for i in 0..ns {
            let mut a = vec![0.0; nv];
            let mut b = vec![1.0; nv];
            let mut c = vec![0.0; nv];
            let mut d = vec![0.0; nv];
            for j in 1..nv - 1 {
                let vj = self.v_grid[j];
                let dv2 = 0.5 * sigma * sigma * vj / (dv * dv);
                let drv = kappa * (th - vj) / (2.0 * dv);
                a[j] = -theta * dt * (dv2 - drv);
                b[j] = 1.0 + theta * dt * (2.0 * dv2 + 0.5 * r);
                c[j] = -theta * dt * (dv2 + drv);
                d[j] = rhs[[i, j]];
            }
            b[1] += 2.0 * a[1];
            c[1] -= a[1];
            a[1] = 0.0;
            b[nv - 2] += 2.0 * c[nv - 2];
            a[nv - 2] -= c[nv - 2];
            c[nv - 2] = 0.0;

            let sol = self.thomas(&a, &b, &c, &d)?;
            for j in 1..nv - 1 {
                out[[i, j]] = sol[j];
            }
            out[[i, 0]] = 2.0 * out[[i, 1]] - out[[i, 2]];
            out[[i, nv - 1]] = 2.0 * out[[i, nv - 2]] - out[[i, nv - 3]];
        }
        Ok(out)
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
            rho: -0.5,
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
            rho: -0.5,
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
            error < 0.03,
            "ADI Heston error too large: {:.2}%",
            error * 100.0
        );
    }

    /// Validation rigoureuse : la PDE ADI 2D (avec terme croisé) doit retrouver le
    /// prix Monte-Carlo Heston (simulateur validé en J12) à ~2 % près.
    #[test]
    fn test_heston_pde_vs_mc() {
        use crate::pricer::{price_gbm, McConfig};
        use crate::products::european_call;
        use crate::HestonSimulator;

        let (spot, v0, kappa, theta, sigma_v, rho, rate, t) =
            (100.0, 0.04, 2.0, 0.04, 0.3, -0.5, 0.05, 1.0);

        let cfg = Pde2dConfig {
            spot,
            second_spot: v0,
            rate,
            dividend_yield: 0.0,
            vol_second: sigma_v,
            kappa,
            theta,
            rho,
            maturity: t,
            n_space_s: 160,
            n_space_v: 80,
            n_time: 200,
            s_min: 1.0,
            s_max: 400.0,
            v_min: 0.0001,
            v_max: 1.0,
        };
        let solver = Pde2dSolver::new(cfg).unwrap();
        let grid = solver.solve_heston(|s, _v| (s - 100.0).max(0.0)).unwrap();
        let pde_val = solver.interpolate(&grid, spot, v0);

        let heston = HestonSimulator::new("S", spot, v0, kappa, theta, sigma_v, rho, rate);
        let mc = price_gbm(
            &european_call("S", 100.0, t, "USD"),
            &heston,
            &McConfig {
                n_paths: 200_000,
                seed: 7,
                steps_per_year: 200,
                rate,
                variance_reduction: None,
            },
        )
        .unwrap()
        .price;

        let rel = (pde_val - mc).abs() / mc;
        println!(
            "Heston ADI={pde_val:.5} vs MC={mc:.5} rel={:.4}%",
            rel * 100.0
        );
        assert!(
            rel < 0.02,
            "ADI {pde_val:.5} vs MC {mc:.5} rel {:.3}%",
            rel * 100.0
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
            rho: -0.5,
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
            rho: -0.5,
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
            rho: -0.5,
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
