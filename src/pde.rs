//! Crank-Nicolson 1D PDE Solver (jalon J19).
//!
//! Schéma Crank-Nicolson (θ = ½, ordre 2 en temps et espace) de l'EDP de
//! Black-Scholes, résolu en marche arrière depuis le payoff terminal. Conditions
//! aux limites de **linéarité** (Γ = ∂²V/∂S² = 0), exactes pour les payoffs
//! vanille linéaires dans les ailes (call/put).
//!
//! **Limite assumée** : la condition Γ = 0 suppose un payoff *linéaire aux bords*
//! du domaine `[s_min, s_max]`. Pour un payoff à courbure au bord (p.ex. un
//! butterfly chevauchant `s_max`), élargir le domaine ou repasser à un Dirichlet
//! analytique.

use crate::numerics;
use crate::KontractError;
use ndarray::Array1;

#[derive(Debug, Clone)]
pub struct PdeConfig {
    pub spot: f64,
    pub sigma: f64,
    pub rate: f64,
    pub dividend_yield: f64,
    pub maturity: f64,
    pub n_space: usize,
    pub n_time: usize,
    pub s_min: f64,
    pub s_max: f64,
    pub psor_tolerance: f64,
    pub psor_max_iterations: usize,
    pub sor_omega: f64,
}

impl Default for PdeConfig {
    fn default() -> Self {
        PdeConfig {
            spot: 100.0,
            sigma: 0.2,
            rate: 0.05,
            dividend_yield: 0.0,
            maturity: 1.0,
            n_space: 500,
            n_time: 200,
            s_min: 10.0,
            s_max: 200.0,
            psor_tolerance: 1e-6,
            psor_max_iterations: 100,
            sor_omega: 1.5,
        }
    }
}

pub struct PdeSolver {
    cfg: PdeConfig,
    space_grid: Array1<f64>,
    dx: f64,
    dt: f64,
    coeff_a: Vec<f64>,
    coeff_b: Vec<f64>,
    coeff_c: Vec<f64>,
}

impl PdeSolver {
    pub fn new(cfg: PdeConfig) -> Result<Self, KontractError> {
        if cfg.n_space < 4 {
            return Err(KontractError::MalformedContract(
                "n_space must be >= 4".to_string(),
            ));
        }
        if cfg.n_time < 1 {
            return Err(KontractError::MalformedContract(
                "n_time must be >= 1".to_string(),
            ));
        }
        if cfg.s_max <= cfg.s_min || cfg.s_min < 0.0 {
            return Err(KontractError::MalformedContract(
                "Invalid space bounds".to_string(),
            ));
        }
        if cfg.maturity <= 0.0 {
            return Err(KontractError::MalformedContract(
                "maturity must be > 0".to_string(),
            ));
        }

        let dx = (cfg.s_max - cfg.s_min) / (cfg.n_space - 1) as f64;
        let dt = cfg.maturity / cfg.n_time as f64;

        let space_grid = Array1::from_vec(
            (0..cfg.n_space)
                .map(|i| cfg.s_min + i as f64 * dx)
                .collect(),
        );

        let n = cfg.n_space;
        let mut coeff_a = vec![0.0; n];
        let mut coeff_b = vec![1.0; n];
        let mut coeff_c = vec![0.0; n];

        let r = cfg.rate;
        let q = cfg.dividend_yield;
        let sigma = cfg.sigma;
        // Coefficients Crank-Nicolson (θ = ½) de l'EDP de Black-Scholes :
        //   αᵢ = ¼Δt(σ²jᵢ² − (r−q)jᵢ),  βᵢ = −½Δt(σ²jᵢ² + r),  γᵢ = ¼Δt(σ²jᵢ² + (r−q)jᵢ)
        // avec jᵢ = Sᵢ/ΔS. Implicite : aᵢ = −αᵢ, bᵢ = 1−βᵢ, cᵢ = −γᵢ.
        let drift_factor = 0.25 * dt * (r - q);
        let diff_factor = 0.25 * dt * sigma * sigma;
        let rate_coef = 0.5 * dt * r;

        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for i in 1..n - 1 {
            let si = space_grid[i];
            let s_factor = si / dx;
            let drift_coef = drift_factor * s_factor; // ¼Δt(r−q)jᵢ
            let diff_coef = diff_factor * s_factor * s_factor; // ¼Δtσ²jᵢ²

            // aᵢ = −αᵢ = −(diff − drift) ; cᵢ = −γᵢ = −(diff + drift).
            coeff_a[i] = -diff_coef + drift_coef;
            coeff_b[i] = 1.0 + 2.0 * diff_coef + rate_coef;
            coeff_c[i] = -diff_coef - drift_coef;
        }

        // Condition aux limites de **linéarité** (Γ = ∂²V/∂S² = 0 aux bords),
        // exacte pour les payoffs vanille (call/put linéaires loin de la monnaie)
        // et agnostique au produit. On élimine V₀ = 2V₁ − V₂ et
        // V_{n−1} = 2V_{n−2} − V_{n−3} en les repliant dans les lignes interne 1 et
        // n−2 (préserve la structure tridiagonale), les lignes 0 et n−1 devenant
        // triviales — les valeurs aux bords sont reconstruites par extrapolation
        // après résolution. Remplace l'ancien gel de Dirichlet (source d'erreur).
        coeff_b[1] += 2.0 * coeff_a[1];
        coeff_c[1] -= coeff_a[1];
        coeff_a[1] = 0.0;
        coeff_b[n - 2] += 2.0 * coeff_c[n - 2];
        coeff_a[n - 2] -= coeff_c[n - 2];
        coeff_c[n - 2] = 0.0;

        Ok(PdeSolver {
            cfg,
            space_grid,
            dx,
            dt,
            coeff_a,
            coeff_b,
            coeff_c,
        })
    }

    pub fn solve_european<F>(&self, payoff_fn: F) -> Result<Array1<f64>, KontractError>
    where
        F: Fn(f64) -> f64,
    {
        let mut v = Array1::from_vec(self.space_grid.iter().map(|&s| payoff_fn(s)).collect());
        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for _ in 0..self.cfg.n_time {
            v = self.cn_step(&v, false)?;
        }
        Ok(v)
    }

    pub fn solve_american<F>(&self, payoff_fn: F) -> Result<Array1<f64>, KontractError>
    where
        F: Fn(f64) -> f64,
    {
        let n = self.cfg.n_space;
        let payoff = Array1::from_vec(self.space_grid.iter().map(|&s| payoff_fn(s)).collect());
        let mut v = payoff.clone();

        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for _ in 0..self.cfg.n_time {
            v = self.cn_step(&v, true)?;
            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            for i in 0..n {
                v[i] = v[i].max(payoff[i]);
            }
        }
        Ok(v)
    }

    pub fn space_grid(&self) -> Array1<f64> {
        self.space_grid.clone()
    }

    pub fn interpolate(&self, grid: &Array1<f64>, spot: f64) -> f64 {
        let n = grid.len();
        if n < 2 {
            return grid[0];
        }
        let s = spot.clamp(self.cfg.s_min, self.cfg.s_max);
        let idx = ((s - self.cfg.s_min) / self.dx).floor() as usize;
        let idx = idx.min(n - 2);

        let s_left = self.space_grid[idx];
        let s_right = self.space_grid[idx + 1];
        let v_left = grid[idx];
        let v_right = grid[idx + 1];

        let w = (s - s_left) / (s_right - s_left);
        v_left * (1.0 - w) + v_right * w
    }

    fn cn_step(&self, v_old: &Array1<f64>, american: bool) -> Result<Array1<f64>, KontractError> {
        let n = v_old.len();
        let dx = self.dx;
        let dt = self.dt;
        let r = self.cfg.rate;
        let q = self.cfg.dividend_yield;
        let sigma = self.cfg.sigma;
        let s = &self.space_grid;

        let mut rhs = Array1::zeros(n);

        // Lignes de bord triviales (la valeur y est reconstruite par linéarité
        // après résolution — cf. repliement dans `new`).
        rhs[0] = 0.0;
        rhs[n - 1] = 0.0;

        // Membre de droite explicite (θ = ½) : +αᵢ V_{i−1} + (1+βᵢ) Vᵢ + γᵢ V_{i+1}.
        let drift_factor = 0.25 * dt * (r - q);
        let diff_factor = 0.25 * dt * sigma * sigma;
        let rate_coef = 0.5 * dt * r;

        // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
        for i in 1..n - 1 {
            let si = s[i];
            let s_factor = si / dx;
            let drift_coef = drift_factor * s_factor;
            let diff_coef = diff_factor * s_factor * s_factor;

            rhs[i] = (diff_coef - drift_coef) * v_old[i - 1]
                + (1.0 - 2.0 * diff_coef - rate_coef) * v_old[i]
                + (diff_coef + drift_coef) * v_old[i + 1];
        }

        if american {
            let payoff = v_old.clone();
            let mut v_new = v_old.clone();

            // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
            for _ in 0..self.cfg.psor_max_iterations {
                let mut res_max: f64 = 0.0;

                // noyau numérique : boucle conservée (cf. CLAUDE.md exceptions)
                for i in 1..n - 1 {
                    let v_pred =
                        (rhs[i] - self.coeff_a[i] * v_new[i - 1] - self.coeff_c[i] * v_new[i + 1])
                            / self.coeff_b[i];
                    let v_proj = v_pred.max(payoff[i]);
                    res_max = res_max.max((v_proj - v_new[i]).abs());
                    v_new[i] += self.cfg.sor_omega * (v_proj - v_new[i]);
                }
                // Bords par linéarité (Γ = 0) à chaque balayage.
                v_new[0] = 2.0 * v_new[1] - v_new[2];
                v_new[n - 1] = 2.0 * v_new[n - 2] - v_new[n - 3];

                if res_max < self.cfg.psor_tolerance {
                    break;
                }
            }
            Ok(v_new)
        } else {
            let mut v_new = self.thomas(&self.coeff_a, &self.coeff_b, &self.coeff_c, &rhs)?;
            // Reconstruction des bords par linéarité (Γ = 0).
            v_new[0] = 2.0 * v_new[1] - v_new[2];
            v_new[n - 1] = 2.0 * v_new[n - 2] - v_new[n - 3];
            Ok(v_new)
        }
    }

    fn thomas(
        &self,
        a: &[f64],
        b: &[f64],
        c: &[f64],
        rhs: &Array1<f64>,
    ) -> Result<Array1<f64>, KontractError> {
        let rhs_vec: Vec<f64> = rhs.to_vec();
        let x_vec = numerics::thomas(a, b, c, &rhs_vec)?;
        Ok(Array1::from(x_vec))
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
    fn test_european_call_vs_black_scholes() {
        let cfg = PdeConfig {
            spot: 100.0,
            sigma: 0.2,
            rate: 0.05,
            dividend_yield: 0.0,
            maturity: 1.0,
            n_space: 500,
            n_time: 5000,
            s_min: 20.0,
            s_max: 200.0,
            psor_tolerance: 1e-6,
            psor_max_iterations: 100,
            sor_omega: 1.5,
        };

        let solver = PdeSolver::new(cfg).unwrap();
        let grid = solver.solve_european(|s| (s - 100.0).max(0.0)).unwrap();

        let bs_val = bs_call(100.0, 100.0, 1.0, 0.05, 0.2);
        let pde_val = solver.interpolate(&grid, 100.0);

        let error = (pde_val - bs_val).abs() / bs_val;
        println!(
            "Call ATM: PDE={:.6}, BS={:.6}, error={:.4}%",
            pde_val,
            bs_val,
            error * 100.0
        );
        assert!(error < 0.005, "Error: {:.2}%", error * 100.0);
    }

    #[test]
    fn test_american_put_vs_european() {
        let cfg = PdeConfig {
            spot: 100.0,
            sigma: 0.2,
            rate: 0.05,
            dividend_yield: 0.0,
            maturity: 1.0,
            n_space: 400,
            n_time: 3000,
            s_min: 20.0,
            s_max: 200.0,
            psor_tolerance: 1e-6,
            psor_max_iterations: 100,
            sor_omega: 1.5,
        };

        let solver = PdeSolver::new(cfg).unwrap();
        let eu_grid = solver.solve_european(|s| (100.0 - s).max(0.0)).unwrap();
        let us_grid = solver.solve_american(|s| (100.0 - s).max(0.0)).unwrap();

        let eu_val = solver.interpolate(&eu_grid, 100.0);
        let us_val = solver.interpolate(&us_grid, 100.0);

        assert!(us_val >= eu_val - 0.01);
    }

    #[test]
    fn test_itm_american_put() {
        let cfg = PdeConfig {
            spot: 80.0,
            sigma: 0.2,
            rate: 0.05,
            dividend_yield: 0.0,
            maturity: 1.0,
            n_space: 400,
            n_time: 3000,
            s_min: 20.0,
            s_max: 200.0,
            psor_tolerance: 1e-6,
            psor_max_iterations: 100,
            sor_omega: 1.5,
        };

        let solver = PdeSolver::new(cfg).unwrap();
        let eu_grid = solver.solve_european(|s| (100.0 - s).max(0.0)).unwrap();
        let us_grid = solver.solve_american(|s| (100.0 - s).max(0.0)).unwrap();

        let eu_val = solver.interpolate(&eu_grid, 80.0);
        let us_val = solver.interpolate(&us_grid, 80.0);

        assert!(us_val > eu_val + 0.05);
    }

    #[test]
    fn test_spot_sensitivity() {
        let cfg = PdeConfig {
            spot: 100.0,
            sigma: 0.2,
            rate: 0.05,
            dividend_yield: 0.0,
            maturity: 1.0,
            n_space: 400,
            n_time: 2000,
            s_min: 20.0,
            s_max: 200.0,
            psor_tolerance: 1e-6,
            psor_max_iterations: 100,
            sor_omega: 1.5,
        };

        let solver = PdeSolver::new(cfg).unwrap();
        let grid = solver.solve_european(|s| (s - 100.0).max(0.0)).unwrap();

        let spots = [80.0, 90.0, 100.0, 110.0, 120.0];
        let prices: Vec<_> = spots
            .iter()
            .map(|&s| solver.interpolate(&grid, s))
            .collect();

        for i in 0..prices.len() - 1 {
            assert!(prices[i] <= prices[i + 1]);
        }
    }

    #[test]
    fn test_vol_sensitivity() {
        let mut prices = vec![];
        for sigma in &[0.1, 0.2, 0.3] {
            let cfg = PdeConfig {
                spot: 100.0,
                sigma: *sigma,
                rate: 0.05,
                dividend_yield: 0.0,
                maturity: 1.0,
                n_space: 300,
                n_time: 1500,
                s_min: 20.0,
                s_max: 200.0,
                psor_tolerance: 1e-6,
                psor_max_iterations: 100,
                sor_omega: 1.5,
            };

            let solver = PdeSolver::new(cfg).unwrap();
            let grid = solver.solve_european(|s| (s - 100.0).max(0.0)).unwrap();
            prices.push(solver.interpolate(&grid, 100.0));
        }

        for i in 0..prices.len() - 1 {
            assert!(prices[i] <= prices[i + 1]);
        }
    }
}
