//! Simulateur Monte-Carlo (jalon J3).
//!
//! Génère des trajectoires de prix sous un mouvement brownien géométrique (GBM).
//! Le schéma est **exact** (log-normal fermé), donc sans biais de discrétisation :
//!
//! ```text
//! S_{t+dt} = S_t · exp[ (μ − ½σ²)·dt + σ·√dt·Z ],   Z ~ N(0, 1)
//! ```
//!
//! Conventions (cf. CLAUDE.md) :
//!   - arrays via `ndarray` (`Array2` de forme `[n_paths, n_steps]`),
//!   - parallélisme via `rayon` (une trajectoire par tâche),
//!   - RNG seedable et **reproductible indépendamment de l'ordonnancement** :
//!     chaque trajectoire dérive sa propre graine de `(seed, index)`.

use ndarray::Array2;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;
use rayon::prelude::*;

use crate::observable::Path;
use crate::KontractError;

/// Mouvement brownien géométrique pour un sous-jacent unique.
#[derive(Debug, Clone, PartialEq)]
pub struct Gbm {
    /// Nom du sous-jacent simulé (doit matcher les `Spot(name)` du contrat).
    pub asset: String,
    /// Prix spot initial `S_0`.
    pub s0: f64,
    /// Drift `μ` (en risque-neutre : `r − q`).
    pub mu: f64,
    /// Volatilité `σ`.
    pub sigma: f64,
}

impl Gbm {
    /// Construit un GBM.
    pub fn new(asset: impl Into<String>, s0: f64, mu: f64, sigma: f64) -> Self {
        Gbm {
            asset: asset.into(),
            s0,
            mu,
            sigma,
        }
    }

    /// Simule `n_paths` trajectoires sur la grille `times` (en années).
    ///
    /// Renvoie un `Array2` de forme `[n_paths, times.len()]`. La simulation
    /// démarre toujours à `t = 0` avec `S_0` ; si `times[0] == 0.0`, la première
    /// colonne vaut exactement `S_0`.
    pub fn simulate(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError> {
        validate_grid(times)?;
        let n_steps = times.len();

        let mut data = vec![0.0f64; n_paths * n_steps];
        data.par_chunks_mut(n_steps.max(1))
            .enumerate()
            .for_each(|(i, row)| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(seed, i as u64));
                let mut s = self.s0;
                let mut prev_t = 0.0_f64;
                for (k, &t) in times.iter().enumerate() {
                    let dt = t - prev_t;
                    if dt > 0.0 {
                        let z: f64 = rng.sample(StandardNormal);
                        let drift = (self.mu - 0.5 * self.sigma * self.sigma) * dt;
                        let diffusion = self.sigma * dt.sqrt() * z;
                        s *= (drift + diffusion).exp();
                    }
                    row[k] = s;
                    prev_t = t;
                }
            });

        Array2::from_shape_vec((n_paths, n_steps), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    /// Variante pratique : renvoie un [`Path`] par trajectoire, prêt pour
    /// l'évaluation d'observables (jalon J5).
    pub fn simulate_paths(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Vec<Path>, KontractError> {
        let arr = self.simulate(times, n_paths, seed)?;
        arr.outer_iter()
            .map(|row| Path::new(times.to_vec()).with_asset(self.asset.clone(), row.to_vec()))
            .collect()
    }
}

/// Vérifie que la grille est non vide, à valeurs positives et croissante.
fn validate_grid(times: &[f64]) -> Result<(), KontractError> {
    if times.is_empty() {
        return Err(KontractError::InconsistentPath("grille vide".into()));
    }
    let mut prev = 0.0_f64;
    for &t in times {
        if t < prev {
            return Err(KontractError::InconsistentPath(format!(
                "grille non croissante au voisinage de {t}"
            )));
        }
        prev = t;
    }
    Ok(())
}

/// Mélange (seed, index) en une graine bien décorrélée (constante de SplitMix64).
fn mix(seed: u64, index: u64) -> u64 {
    seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}
