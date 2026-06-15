//! FX simple : multi-devise & corrélation spot/FX (jalon J25).
//!
//! Dernier jalon de la roadmap. Il ajoute le **change** à l'algèbre **sans
//! toucher à l'AST** ni introduire de cas spécial produit dans le pricer :
//!
//!   - une **option de change vanille** (Garman-Kohlhagen) est un simple call/put
//!     de l'algèbre sur le taux de change, price sous un `Gbm` de drift `r_d − r_f`
//!     et actualisé à `r_d` — donc déjà couvert par le moteur existant ; on fournit
//!     ici la **référence analytique** GK ;
//!   - les produits **cross-currency corrélés** (quanto, composite) requièrent la
//!     corrélation entre l'actif et le taux de change. Le quanto se ramène à un GBM
//!     mono-actif à **drift ajusté** `r_f − ρ·σ_S·σ_X` ; le composite met en jeu le
//!     **produit `S·X`** de deux GBM corrélés et demande donc une simulation
//!     **multi-actifs** : [`CorrelatedGbm2`] peuple deux sous-jacents corrélés par
//!     trajectoire, que le DSL référence par leur nom (`Spot("S")`, `Spot("X")`).
//!
//! Conventions (taux déterministes, cf. décision projet — les taux stochastiques
//! sont en J24, leur couplage au FX est laissé à une extension future) :
//!   - `X` = taux de change *domestique par unité étrangère* ;
//!   - mesure risque-neutre **domestique** ;
//!   - `r_d`, `r_f` = taux domestique / étranger ; `q_S` = dividende de l'actif.

use ndarray::Array2;
use rayon::prelude::*;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;

use crate::numerics::norm_cdf;
use crate::observable::Path;
use crate::simulator::{mix, Simulator};
use crate::KontractError;

// ============================================================================
// Références analytiques
// ============================================================================

/// Prix Garman-Kohlhagen d'un **call** de change : option sur le taux `X`,
/// strike `k`, maturité `t`, taux domestique `r_d`, étranger `r_f`, vol `σ`.
///
/// Identique à Black-Scholes avec un rendement de dividende `q = r_f`.
pub fn garman_kohlhagen_call(x0: f64, k: f64, t: f64, r_d: f64, r_f: f64, sigma: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return (x0 * (-r_f * t).exp() - k * (-r_d * t).exp()).max(0.0);
    }
    let vol_sqrt_t = sigma * t.sqrt();
    let d1 = ((x0 / k).ln() + (r_d - r_f + 0.5 * sigma * sigma) * t) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;
    x0 * (-r_f * t).exp() * norm_cdf(d1) - k * (-r_d * t).exp() * norm_cdf(d2)
}

/// Prix Garman-Kohlhagen d'un **put** de change.
pub fn garman_kohlhagen_put(x0: f64, k: f64, t: f64, r_d: f64, r_f: f64, sigma: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return (k * (-r_d * t).exp() - x0 * (-r_f * t).exp()).max(0.0);
    }
    let vol_sqrt_t = sigma * t.sqrt();
    let d1 = ((x0 / k).ln() + (r_d - r_f + 0.5 * sigma * sigma) * t) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;
    k * (-r_d * t).exp() * norm_cdf(-d2) - x0 * (-r_f * t).exp() * norm_cdf(-d1)
}

/// Taux de change à terme (parité des taux d'intérêt couverte) :
/// `F = X₀·e^{(r_d − r_f)·t}`.
pub fn fx_forward(x0: f64, t: f64, r_d: f64, r_f: f64) -> f64 {
    x0 * ((r_d - r_f) * t).exp()
}

/// Prix d'un **quanto call** : option sur l'actif étranger `S` (vol `σ_S`),
/// réglée en devise domestique à taux de change fixe (notionnel 1 unité
/// domestique par point d'indice).
///
/// La corrélation `ρ` entre `S` et le taux de change `X` (vol `σ_X`) entre par
/// l'**ajustement quanto** du drift : `μ_q = r_f − q_S − ρ·σ_S·σ_X`. La valeur
/// est alors `e^{−r_d t}·[F_q·N(d₁) − K·N(d₂)]` avec `F_q = S₀·e^{μ_q t}`.
#[allow(clippy::too_many_arguments)]
pub fn quanto_call(
    s0: f64,
    k: f64,
    t: f64,
    r_d: f64,
    r_f: f64,
    q_s: f64,
    sigma_s: f64,
    sigma_x: f64,
    rho: f64,
) -> f64 {
    let mu_q = r_f - q_s - rho * sigma_s * sigma_x;
    let fq = s0 * (mu_q * t).exp();
    black_76_call(fq, k, t, r_d, sigma_s)
}

/// Prix d'un **quanto put** (symétrique du call).
#[allow(clippy::too_many_arguments)]
pub fn quanto_put(
    s0: f64,
    k: f64,
    t: f64,
    r_d: f64,
    r_f: f64,
    q_s: f64,
    sigma_s: f64,
    sigma_x: f64,
    rho: f64,
) -> f64 {
    let mu_q = r_f - q_s - rho * sigma_s * sigma_x;
    let fq = s0 * (mu_q * t).exp();
    black_76_put(fq, k, t, r_d, sigma_s)
}

/// Black-76 : call sur un forward `f`, actualisé au taux `r`.
fn black_76_call(f: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return (-r * t).exp() * (f - k).max(0.0);
    }
    let vol_sqrt_t = sigma * t.sqrt();
    let d1 = ((f / k).ln() + 0.5 * sigma * sigma * t) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;
    (-r * t).exp() * (f * norm_cdf(d1) - k * norm_cdf(d2))
}

/// Black-76 : put sur un forward `f`.
fn black_76_put(f: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    if t <= 0.0 || sigma <= 0.0 {
        return (-r * t).exp() * (k - f).max(0.0);
    }
    let vol_sqrt_t = sigma * t.sqrt();
    let d1 = ((f / k).ln() + 0.5 * sigma * sigma * t) / vol_sqrt_t;
    let d2 = d1 - vol_sqrt_t;
    (-r * t).exp() * (k * norm_cdf(-d2) - f * norm_cdf(-d1))
}

/// Volatilité du produit de deux GBM corrélés : `√(σ_S² + σ_X² + 2ρσ_Sσ_X)`.
///
/// Le produit `S·X` de deux log-normaux corrélés est lui-même log-normal : un
/// **composite** `max(S_T·X_T − K, 0)` price comme un Black-Scholes sur `U = S·X`
/// de cette volatilité combinée. Sert de référence analytique au test composite.
pub fn composite_vol(sigma_s: f64, sigma_x: f64, rho: f64) -> f64 {
    (sigma_s * sigma_s + sigma_x * sigma_x + 2.0 * rho * sigma_s * sigma_x).sqrt()
}

// ============================================================================
// Simulateur multi-actifs : deux GBM corrélés
// ============================================================================

/// Description d'un facteur GBM : `(nom, s₀, μ, σ)`.
#[derive(Debug, Clone, PartialEq)]
pub struct GbmFactor {
    /// Nom du sous-jacent (doit matcher les `Spot(name)` du contrat).
    pub name: String,
    /// Spot initial.
    pub s0: f64,
    /// Drift (selon la mesure choisie par l'appelant).
    pub mu: f64,
    /// Volatilité.
    pub sigma: f64,
}

impl GbmFactor {
    /// Construit un facteur GBM.
    pub fn new(name: impl Into<String>, s0: f64, mu: f64, sigma: f64) -> Self {
        GbmFactor {
            name: name.into(),
            s0,
            mu,
            sigma,
        }
    }
}

/// Deux GBM **corrélés** (corrélation `ρ` des browniens) simulés conjointement.
///
/// Chaque trajectoire peuple un [`Path`] à **deux sous-jacents** (`a` et `b`), que
/// le DSL référence par leur nom — c'est le support multi-devise du moteur. Les
/// drifts sont fournis tels quels : la convention financière (mesure, ajustements
/// quanto) est portée par le module FX / l'appelant, pas par le simulateur.
#[derive(Debug, Clone, PartialEq)]
pub struct CorrelatedGbm2 {
    /// Premier facteur (sous-jacent « principal »).
    pub a: GbmFactor,
    /// Second facteur.
    pub b: GbmFactor,
    /// Corrélation `ρ ∈ [−1, 1]` entre les deux browniens.
    pub rho: f64,
}

impl CorrelatedGbm2 {
    /// Construit deux GBM corrélés.
    pub fn new(a: GbmFactor, b: GbmFactor, rho: f64) -> Self {
        CorrelatedGbm2 { a, b, rho }
    }

    /// Simule la trajectoire d'un facteur à partir de ses incréments gaussiens.
    fn evolve(factor: &GbmFactor, times: &[f64], normals: &[f64]) -> Vec<f64> {
        let mut s = factor.s0;
        let mut prev_t = 0.0_f64;
        // noyau numérique : récurrence séquentielle par trajectoire (GBM log-exact).
        times
            .iter()
            .enumerate()
            .map(|(k, &t)| {
                let dt = t - prev_t;
                if dt > 0.0 {
                    let drift = (factor.mu - 0.5 * factor.sigma * factor.sigma) * dt;
                    let diffusion = factor.sigma * dt.sqrt() * normals[k];
                    s *= (drift + diffusion).exp();
                }
                prev_t = t;
                s
            })
            .collect()
    }
}

impl Simulator for CorrelatedGbm2 {
    fn simulate(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError> {
        // Sous-jacent principal (`a`) uniquement, pour l'interface mono-actif.
        let paths = self.simulate_paths(times, n_paths, seed)?;
        let n = times.len();
        let mut data = vec![0.0f64; n_paths * n];
        data.par_chunks_mut(n.max(1))
            .zip(paths.par_iter())
            .try_for_each(|(row, path)| {
                let series = path.spot_series(&self.a.name)?;
                row.copy_from_slice(series);
                Ok::<(), KontractError>(())
            })?;
        Array2::from_shape_vec((n_paths, n), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    fn simulate_paths(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Vec<Path>, KontractError> {
        if !(-1.0..=1.0).contains(&self.rho) {
            return Err(KontractError::MalformedContract(
                "CorrelatedGbm2: ρ doit être dans [−1, 1]".into(),
            ));
        }
        if self.a.sigma < 0.0 || self.b.sigma < 0.0 {
            return Err(KontractError::MalformedContract(
                "CorrelatedGbm2: σ doit être ≥ 0".into(),
            ));
        }
        let n = times.len();
        let chol = (1.0 - self.rho * self.rho).max(0.0).sqrt(); // Cholesky 2×2.

        (0..n_paths)
            .into_par_iter()
            .map(|i| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(seed, i as u64));
                // Incréments gaussiens corrélés : z_a indépendant, z_b = ρ z_a + √(1−ρ²) w.
                let (za, zb): (Vec<f64>, Vec<f64>) = (0..n)
                    .map(|_| {
                        let z1: f64 = rng.sample(StandardNormal);
                        let z2: f64 = rng.sample(StandardNormal);
                        (z1, self.rho * z1 + chol * z2)
                    })
                    .unzip();
                let sa = Self::evolve(&self.a, times, &za);
                let sb = Self::evolve(&self.b, times, &zb);
                Path::new(times.to_vec())
                    .with_asset(self.a.name.clone(), sa)?
                    .with_asset(self.b.name.clone(), sb)
            })
            .collect()
    }

    fn asset_name(&self) -> &str {
        &self.a.name
    }
}
