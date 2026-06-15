//! Taux courts stochastiques : Vasicek & Hull-White (jalon J24).
//!
//! Jusqu'ici l'actualisation du pricer était **déterministe** (`e^{-rt}`, cf.
//! décision « discount déterministe jusqu'à J24 »). Ce module introduit un
//! **taux court stochastique** `r_t` et lève cette limitation : le facteur
//! d'actualisation le long d'une trajectoire devient le compte de capitalisation
//! réalisé `D(t) = exp(−∫₀ᵗ r_s ds)`.
//!
//! ## Respect des invariants
//!
//! - L'**AST reste pur** : aucune primitive de taux n'y est ajoutée. Un taux
//!   stochastique est un **mode d'exécution** du pricer (comme MC vs EDP) :
//!   [`crate::pricer::price_under_short_rate`] rejoue n'importe quel `Contract`
//!   en remplaçant l'actualisation déterministe par l'actualisation réalisée du
//!   modèle de taux. Une obligation zéro-coupon `when(at(T), one)` ainsi price
//!   doit retrouver `P(0,T)` analytique — sans changer le contrat.
//! - Les **swaptions** sont des produits *de taux purs* : leur payoff dépend de
//!   prix d'obligations `P(T₀, Tᵢ)`, que l'algèbre actions/spot n'exprime pas
//!   (au même titre que l'asian, faute d'observable d'agrégation). Elles sont
//!   donc évaluées par un moteur de taux dédié ([`Swaption`]) — Monte-Carlo
//!   **et** analytique (décomposition de Jamshidian) — sans cas spécial produit
//!   dans le pricer compositionnel.
//!
//! ## Modèle de Vasicek
//!
//! Dynamique risque-neutre `dr = a(b − r) dt + σ dW`. Modèle **gaussien affine** :
//! la transition `r_{t+Δ} | r_t` est exactement normale (simulation exacte, pas
//! d'Euler) et le zéro-coupon est affine `P(t,T) = A(t,T) e^{−B(t,T) r_t}`.
//!
//! Hull-White (extended Vasicek) partage cette dynamique avec `b → b(t)` calibré
//! à la courbe initiale ; [`HullWhite`] implémente le cas d'une **courbe initiale
//! plate** `P(0,T) = e^{−r₀T}` (repricée exactement), sur le même trait.

use ndarray::Array2;
use rayon::prelude::*;

use crate::compiler::{compile, Plan};
use crate::numerics::norm_cdf;
use crate::observable::Path;
use crate::pricer::{cashflows_pub, summarize_pvs, McConfig, PriceResult};
use crate::simulator::mix;
use crate::{Contract, KontractError};

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;

/// Modèle de taux court à un facteur, gaussien affine.
///
/// Le pricer (jalon J24) n'utilise que cette interface : il sait simuler le taux
/// court le long d'une grille et reconstruire un zéro-coupon analytique.
pub trait ShortRateModel: Send + Sync {
    /// Simule `n_paths` trajectoires du taux court sur `grid`.
    ///
    /// Renvoie un `Array2` de forme `[n_paths, grid.len()]`, `r(0) = r₀` en
    /// première colonne (si `grid[0] == 0`).
    fn simulate_short_rate(
        &self,
        grid: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError>;

    /// Prix analytique du zéro-coupon `P(t, T)` sachant `r(t) = r_t`.
    fn zero_bond(&self, t: f64, big_t: f64, r_t: f64) -> f64;

    /// Taux court initial `r₀`.
    fn r0(&self) -> f64;

    /// Prix initial du zéro-coupon `P(0, T)`.
    fn discount_bond0(&self, big_t: f64) -> f64 {
        self.zero_bond(0.0, big_t, self.r0())
    }
}

/// Modèle de Vasicek : `dr = a(b − r) dt + σ dW`.
#[derive(Debug, Clone, PartialEq)]
pub struct Vasicek {
    /// Taux court initial `r₀`.
    pub r0: f64,
    /// Vitesse de retour à la moyenne `a > 0`.
    pub a: f64,
    /// Niveau de long terme `b`.
    pub b: f64,
    /// Volatilité `σ`.
    pub sigma: f64,
}

impl Vasicek {
    /// Construit un modèle de Vasicek.
    pub fn new(r0: f64, a: f64, b: f64, sigma: f64) -> Self {
        Vasicek { r0, a, b, sigma }
    }

    /// Coefficient `B(t,T) = (1 − e^{−aτ}) / a` (τ = T − t).
    fn bcoef(&self, tau: f64) -> f64 {
        if self.a.abs() < 1e-12 {
            tau
        } else {
            (1.0 - (-self.a * tau).exp()) / self.a
        }
    }

    /// Volatilité du prix d'obligation pour une option d'expiry `t0` sur un
    /// zéro-coupon de maturité `s` : `σ_P = σ·√((1−e^{−2a t0})/(2a))·B(t0, s)`.
    fn bond_vol(&self, t0: f64, s: f64) -> f64 {
        let var = if self.a.abs() < 1e-12 {
            t0
        } else {
            (1.0 - (-2.0 * self.a * t0).exp()) / (2.0 * self.a)
        };
        self.sigma * var.sqrt() * self.bcoef(s - t0)
    }

    /// Prix d'un **call** européen (expiry `t0`) sur le zéro-coupon `P(·, s)`,
    /// strike `x` — formule fermée de Vasicek (Black sur obligation).
    pub fn zero_bond_call(&self, t0: f64, s: f64, x: f64) -> f64 {
        let p_s = self.discount_bond0(s);
        let p_t0 = self.discount_bond0(t0);
        let sp = self.bond_vol(t0, s);
        if sp < 1e-14 {
            return (p_s - x * p_t0).max(0.0);
        }
        let h = (p_s / (p_t0 * x)).ln() / sp + sp / 2.0;
        p_s * norm_cdf(h) - x * p_t0 * norm_cdf(h - sp)
    }

    /// Prix d'un **put** européen (expiry `t0`) sur le zéro-coupon `P(·, s)`,
    /// strike `x` — symétrique du call.
    pub fn zero_bond_put(&self, t0: f64, s: f64, x: f64) -> f64 {
        let p_s = self.discount_bond0(s);
        let p_t0 = self.discount_bond0(t0);
        let sp = self.bond_vol(t0, s);
        if sp < 1e-14 {
            return (x * p_t0 - p_s).max(0.0);
        }
        let h = (p_s / (p_t0 * x)).ln() / sp + sp / 2.0;
        x * p_t0 * norm_cdf(-h + sp) - p_s * norm_cdf(-h)
    }

    /// Prix analytique d'une swaption par **décomposition de Jamshidian**.
    ///
    /// Le payoff payeur `max(0, 1 − Σ cᵢ P(T₀,Tᵢ))` se décompose, grâce à la
    /// monotonie de `P(T₀,Tᵢ)` en `r(T₀)`, en un portefeuille de **puts** sur
    /// zéro-coupons (calls pour le receveur) de strikes `Xᵢ = P(T₀,Tᵢ; r*)`, où
    /// `r*` annule la valeur du swap.
    pub fn swaption_analytic(&self, swaption: &Swaption) -> Result<f64, KontractError> {
        let coupons = swaption.coupon_flows();
        let t0 = swaption.expiry;

        // r* : racine de Σ cᵢ P(T₀, Tᵢ; r*) = 1 (décroissant en r → bisection).
        let value_at = |r: f64| -> f64 {
            coupons
                .iter()
                .map(|&(ti, ci)| ci * self.zero_bond(t0, ti, r))
                .sum::<f64>()
        };
        let r_star = bisection_decreasing(&value_at, 1.0, -2.0, 2.0, 1e-12, 200)?;

        // Strikes = prix d'obligation au taux critique.
        let price = coupons
            .iter()
            .map(|&(ti, ci)| {
                let xi = self.zero_bond(t0, ti, r_star);
                let opt = if swaption.is_payer {
                    self.zero_bond_put(t0, ti, xi)
                } else {
                    self.zero_bond_call(t0, ti, xi)
                };
                ci * opt
            })
            .sum();
        Ok(price)
    }
}

impl ShortRateModel for Vasicek {
    fn simulate_short_rate(
        &self,
        grid: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError> {
        if self.sigma < 0.0 || self.a <= 0.0 {
            return Err(KontractError::MalformedContract(
                "Vasicek: requiert a > 0 et σ ≥ 0".into(),
            ));
        }
        let n = grid.len();
        let mut data = vec![0.0f64; n_paths * n];
        data.par_chunks_mut(n.max(1))
            .enumerate()
            .for_each(|(i, row)| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(seed, i as u64));
                let mut r = self.r0;
                let mut prev_t = 0.0_f64;
                // noyau numérique : récurrence séquentielle par trajectoire.
                for (k, &t) in grid.iter().enumerate() {
                    let dt = t - prev_t;
                    if dt > 0.0 {
                        // Transition exacte : r | r_prev ~ N(mean, var).
                        let e = (-self.a * dt).exp();
                        let mean = r * e + self.b * (1.0 - e);
                        let var = self.sigma * self.sigma * (1.0 - e * e) / (2.0 * self.a);
                        let z: f64 = rng.sample(StandardNormal);
                        r = mean + var.sqrt() * z;
                    }
                    row[k] = r;
                    prev_t = t;
                }
            });
        Array2::from_shape_vec((n_paths, n), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    fn zero_bond(&self, t: f64, big_t: f64, r_t: f64) -> f64 {
        let tau = big_t - t;
        if tau <= 0.0 {
            return 1.0;
        }
        let bc = self.bcoef(tau);
        let a2 = self.a * self.a;
        let log_a = (self.b - self.sigma * self.sigma / (2.0 * a2)) * (bc - tau)
            - self.sigma * self.sigma * bc * bc / (4.0 * self.a);
        log_a.exp() * (-bc * r_t).exp()
    }

    fn r0(&self) -> f64 {
        self.r0
    }
}

/// Hull-White (extended Vasicek) calibré à une **courbe initiale plate**
/// `P(0,T) = e^{−r₀T}`.
///
/// Même dynamique gaussienne que Vasicek (`a`, `σ`), avec un drift à moyenne
/// **dépendante du temps** `θ(t)` choisi pour repricer exactement la courbe
/// plate. Le prix d'obligation s'écrit relativement à la courbe initiale :
/// `P(t,T) = (P^M(0,T)/P^M(0,t))·exp(−B(t,T)(r_t − f(0,t)) − ...)`, qui pour une
/// courbe plate `f(0,t) ≡ r₀` se simplifie.
#[derive(Debug, Clone, PartialEq)]
pub struct HullWhite {
    /// Taux court / forward instantané initial (courbe plate) `r₀`.
    pub r0: f64,
    /// Vitesse de retour à la moyenne `a > 0`.
    pub a: f64,
    /// Volatilité `σ`.
    pub sigma: f64,
}

impl HullWhite {
    /// Construit un Hull-White sur courbe plate `e^{−r₀T}`.
    pub fn new(r0: f64, a: f64, sigma: f64) -> Self {
        HullWhite { r0, a, sigma }
    }

    fn bcoef(&self, tau: f64) -> f64 {
        if self.a.abs() < 1e-12 {
            tau
        } else {
            (1.0 - (-self.a * tau).exp()) / self.a
        }
    }

    /// Drift à moyenne dépendante du temps `θ(t)` (courbe plate) :
    /// `θ(t) = a·r₀ + (σ²/2a)(1 − e^{−2at})`.
    fn theta(&self, t: f64) -> f64 {
        self.a * self.r0
            + (self.sigma * self.sigma / (2.0 * self.a)) * (1.0 - (-2.0 * self.a * t).exp())
    }
}

impl ShortRateModel for HullWhite {
    fn simulate_short_rate(
        &self,
        grid: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError> {
        if self.sigma < 0.0 || self.a <= 0.0 {
            return Err(KontractError::MalformedContract(
                "HullWhite: requiert a > 0 et σ ≥ 0".into(),
            ));
        }
        let n = grid.len();
        let mut data = vec![0.0f64; n_paths * n];
        data.par_chunks_mut(n.max(1))
            .enumerate()
            .for_each(|(i, row)| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(seed, i as u64));
                let mut r = self.r0;
                let mut prev_t = 0.0_f64;
                // noyau numérique : récurrence séquentielle par trajectoire.
                // Diffusion exacte ; drift à moyenne θ(t) intégré par la règle du
                // point milieu (ordre 2, exact dans la limite de grille fine).
                for (k, &t) in grid.iter().enumerate() {
                    let dt = t - prev_t;
                    if dt > 0.0 {
                        let e = (-self.a * dt).exp();
                        // Moyenne conditionnelle sur [prev_t, t] : θ au point milieu.
                        let theta_mid = self.theta(0.5 * (prev_t + t));
                        let mean = r * e + (theta_mid / self.a) * (1.0 - e);
                        let var = self.sigma * self.sigma * (1.0 - e * e) / (2.0 * self.a);
                        let z: f64 = rng.sample(StandardNormal);
                        r = mean + var.sqrt() * z;
                    }
                    row[k] = r;
                    prev_t = t;
                }
            });
        Array2::from_shape_vec((n_paths, n), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    fn zero_bond(&self, t: f64, big_t: f64, r_t: f64) -> f64 {
        let tau = big_t - t;
        if tau <= 0.0 {
            return 1.0;
        }
        // Courbe plate : P^M(0,u) = e^{−r₀ u}, f(0,t) = r₀.
        let pm_big = (-self.r0 * big_t).exp();
        let pm_t = (-self.r0 * t).exp();
        let bc = self.bcoef(tau);
        // ln A = ln(P^M(0,T)/P^M(0,t)) + B·f(0,t) − (σ²/4a)(1−e^{−2at})B².
        let half_var = (self.sigma * self.sigma / (4.0 * self.a))
            * (1.0 - (-2.0 * self.a * t).exp())
            * bc
            * bc;
        let log_a = (pm_big / pm_t).ln() + bc * self.r0 - half_var;
        (log_a - bc * r_t).exp()
    }

    fn r0(&self) -> f64 {
        self.r0
    }
}

/// Description d'une swaption européenne sur un swap à jambe fixe régulière.
///
/// Notionnel 1, jambe flottante au pair. Le payoff payeur à l'expiry `T₀` vaut
/// `max(0, 1 − Σ cᵢ P(T₀,Tᵢ))` avec `cᵢ = K·τᵢ` et `c_n = K·τ_n + 1`.
#[derive(Debug, Clone, PartialEq)]
pub struct Swaption {
    /// Expiry de l'option (= premier reset du swap) `T₀`.
    pub expiry: f64,
    /// Dates de paiement de la jambe fixe `T₁ < … < T_n`.
    pub payment_times: Vec<f64>,
    /// Fractions d'année `τᵢ` (mêmes longueur/ordre que `payment_times`).
    pub year_fractions: Vec<f64>,
    /// Taux fixe `K`.
    pub fixed_rate: f64,
    /// `true` = payeur (paie fixe), `false` = receveur.
    pub is_payer: bool,
}

impl Swaption {
    /// Swaption « at-the-money » standard : `n` paiements espacés de `tenor`
    /// années à partir de `expiry`, fraction d'année = `tenor`.
    pub fn level(expiry: f64, tenor: f64, n: usize, fixed_rate: f64, is_payer: bool) -> Self {
        let payment_times = (1..=n).map(|i| expiry + i as f64 * tenor).collect();
        let year_fractions = vec![tenor; n];
        Swaption {
            expiry,
            payment_times,
            year_fractions,
            fixed_rate,
            is_payer,
        }
    }

    /// Flux `(Tᵢ, cᵢ)` de la jambe fixe + notionnel : `cᵢ = K·τᵢ`, et le dernier
    /// inclut le remboursement du notionnel (`+1`).
    fn coupon_flows(&self) -> Vec<(f64, f64)> {
        let n = self.payment_times.len();
        self.payment_times
            .iter()
            .zip(self.year_fractions.iter())
            .enumerate()
            .map(|(i, (&ti, &tau))| {
                let c = self.fixed_rate * tau + if i == n - 1 { 1.0 } else { 0.0 };
                (ti, c)
            })
            .collect()
    }

    /// Valeur du swap (point de vue payeur) à l'expiry sachant `r(T₀)` :
    /// `1 − Σ cᵢ P(T₀,Tᵢ)`.
    fn payer_swap_value(&self, model: &dyn ShortRateModel, r_t0: f64) -> f64 {
        let pv_fixed: f64 = self
            .coupon_flows()
            .iter()
            .map(|&(ti, ci)| ci * model.zero_bond(self.expiry, ti, r_t0))
            .sum();
        1.0 - pv_fixed
    }
}

/// Prix Monte-Carlo d'une swaption sous un modèle de taux court quelconque.
///
/// On simule `r` jusqu'à l'expiry `T₀`, on actualise par le compte de
/// capitalisation réalisé `D(T₀) = exp(−∫₀^{T₀} r ds)` (trapèze sur la grille),
/// et on moyenne le payoff `max(0, ±swap(T₀))`.
pub fn swaption_price_mc(
    model: &dyn ShortRateModel,
    swaption: &Swaption,
    cfg: &McConfig,
    steps: usize,
) -> Result<PriceResult, KontractError> {
    if steps == 0 {
        return Err(KontractError::MalformedContract(
            "swaption_price_mc: steps doit être > 0".into(),
        ));
    }
    let t0 = swaption.expiry;
    let grid: Vec<f64> = (0..=steps).map(|k| t0 * k as f64 / steps as f64).collect();
    let rates = model.simulate_short_rate(&grid, cfg.n_paths, cfg.seed)?;

    let pvs = rates
        .outer_iter()
        .into_par_iter()
        .map(|row| {
            let r_row = row.as_slice().unwrap_or(&[]);
            let discount = discount_factor(&grid, r_row);
            let r_t0 = r_row.last().copied().unwrap_or(model.r0());
            let swap = swaption.payer_swap_value(model, r_t0);
            let payoff = if swaption.is_payer {
                swap.max(0.0)
            } else {
                (-swap).max(0.0)
            };
            discount * payoff
        })
        .collect::<Vec<f64>>();

    Ok(summarize_pvs(&pvs))
}

/// Facteur d'actualisation réalisé `D(T) = exp(−∫₀ᵀ r ds)` sur toute la grille
/// (intégration trapézoïdale du taux court).
pub fn discount_factor(grid: &[f64], rates: &[f64]) -> f64 {
    (-integrate_trapz(grid, rates)).exp()
}

/// Série des facteurs d'actualisation cumulés `D(tᵢ) = exp(−∫₀^{tᵢ} r ds)` à
/// chaque point de grille (pour l'actualisation pas-à-pas des flux d'un contrat).
pub fn cumulative_discounts(grid: &[f64], rates: &[f64]) -> Vec<f64> {
    if grid.is_empty() || rates.is_empty() {
        return Vec::new();
    }
    grid.iter()
        .zip(rates.iter())
        .scan(
            (0.0_f64, grid[0], rates[0]),
            |(acc, prev_t, prev_r), (&t, &r)| {
                let dt = t - *prev_t;
                // Trapèze sur [prev_t, t].
                *acc += 0.5 * (*prev_r + r) * dt;
                *prev_t = t;
                *prev_r = r;
                Some((-*acc).exp())
            },
        )
        .collect()
}

/// Price un `Contract` quelconque en **actualisation stochastique** : mode
/// d'exécution du pricer qui remplace l'actualisation déterministe `e^{−rt}`
/// par le compte de capitalisation réalisé du modèle de taux.
///
/// L'AST est **inchangé** : une obligation zéro-coupon `when(at(T), one)` price
/// ainsi retrouve `P(0,T)` analytique. Les flux sont réduits sur une trajectoire
/// (grille seule, sans actif equity), puis actualisés par `D(tᵢ)` réalisé.
///
/// Note : les contrats référençant un `Spot` equity (actif risqué) sortent du
/// périmètre J24 — ils demanderaient une simulation jointe taux/actif (futur).
pub fn price_under_short_rate(
    contract: &Contract,
    model: &dyn ShortRateModel,
    cfg: &McConfig,
) -> Result<PriceResult, KontractError> {
    let plan = compile(contract)?;
    // Grille dense forcée : l'intégrale ∫r exige une subdivision fine même sans
    // barrière (un européen ne fournirait que {0, T}).
    let dense = Plan {
        needs_fine_grid: true,
        ..plan
    };
    let grid = dense.time_grid(cfg.steps_per_year);
    let rates = model.simulate_short_rate(&grid, cfg.n_paths, cfg.seed)?;

    let pvs = rates
        .outer_iter()
        .into_par_iter()
        .map(|row| {
            let r_row = row.as_slice().unwrap_or(&[]);
            let discounts = cumulative_discounts(&grid, r_row);
            let path = Path::new(grid.clone());
            let flows = cashflows_pub(contract, 0, &path)?;
            Ok(flows
                .into_iter()
                .map(|(amt, ti)| amt * discounts[ti])
                .sum::<f64>())
        })
        .collect::<Result<Vec<f64>, KontractError>>()?;

    Ok(summarize_pvs(&pvs))
}

/// Intégrale trapézoïdale `∫ r dt` sur la grille.
fn integrate_trapz(grid: &[f64], rates: &[f64]) -> f64 {
    grid.windows(2)
        .zip(rates.windows(2))
        .map(|(t, r)| 0.5 * (r[0] + r[1]) * (t[1] - t[0]))
        .sum()
}

/// Bisection sur une fonction **décroissante** `f` : trouve `x` tel que
/// `f(x) = target` dans `[lo, hi]`.
fn bisection_decreasing(
    f: &dyn Fn(f64) -> f64,
    target: f64,
    lo: f64,
    hi: f64,
    tol: f64,
    max_iter: usize,
) -> Result<f64, KontractError> {
    // f décroissante : f(lo) ≥ target ≥ f(hi) attendu.
    let (mut lo, mut hi) = (lo, hi);
    let solved = (0..max_iter).try_fold((lo + hi) / 2.0, |_mid, _| {
        let mid = (lo + hi) / 2.0;
        let v = f(mid);
        if (v - target).abs() < tol {
            return Err(mid); // convergé : on sort par Err (court-circuit)
        }
        if v > target {
            lo = mid;
        } else {
            hi = mid;
        }
        Ok(mid)
    });
    match solved {
        Err(root) => Ok(root),
        Ok(mid) => Ok(mid), // pas de convergence stricte : meilleure estimation
    }
}
