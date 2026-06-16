//! Simulateur Monte-Carlo multi-actifs — N GBM corrélés (jalon J27).
//!
//! Extension naturelle de [`crate::fx::CorrelatedGbm2`] à N actifs quelconques
//! via une matrice de corrélation N×N (factorisation de Cholesky L). Aucun nouveau
//! combinateur AST : les payoffs basket/spread s'expriment avec l'arithmétique
//! observable existante.
//!
//! ```text
//! basket = (spot("S1") + spot("S2") + spot("S3")) / 3.0
//! spread = (spot("S1") - spot("S2")).clip(0.0)
//! ```

use crate::fx::GbmFactor;
use crate::numerics::cholesky_lower;
use crate::observable::Path;
use crate::simulator::{mix, Simulator};
use crate::KontractError;
use ndarray::Array2;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;
use rayon::prelude::*;

/// Simulateur de N GBM corrélés via une matrice de corrélation N×N.
///
/// Chaque trajectoire peuple un [`Path`] avec N sous-jacents nommés, exploitant
/// le support multi-actifs du moteur existant (HashMap dans `Path`). Les payoffs
/// basket ou spread se construisent entièrement en DSL.
///
/// La corrélation est injectée par les innovations browniennes `w = L·z` où L est
/// le facteur de Cholesky inférieur de la matrice ρ et z ~ N(0, I_N).
///
/// # Exemple
/// ```rust,ignore
/// let model = CorrelatedGbmN::new(
///     vec![
///         GbmFactor::new("S1", 100.0, 0.05, 0.20),
///         GbmFactor::new("S2", 100.0, 0.05, 0.25),
///         GbmFactor::new("S3", 100.0, 0.05, 0.15),
///     ],
///     vec![
///         vec![1.0, 0.6, 0.4],
///         vec![0.6, 1.0, 0.5],
///         vec![0.4, 0.5, 1.0],
///     ],
/// ).unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct CorrelatedGbmN {
    assets: Vec<GbmFactor>,
    /// Facteur de Cholesky inférieur L tel que ρ = L·Lᵀ.
    chol: Vec<Vec<f64>>,
}

impl CorrelatedGbmN {
    /// Construit N GBM corrélés.
    ///
    /// # Erreurs
    /// - Moins de 2 actifs.
    /// - `corr` n'est pas N×N.
    /// - Une volatilité est négative.
    pub fn new(assets: Vec<GbmFactor>, corr: Vec<Vec<f64>>) -> Result<Self, KontractError> {
        let k = assets.len();
        if k < 2 {
            return Err(KontractError::MalformedContract(
                "CorrelatedGbmN : au moins 2 actifs requis".into(),
            ));
        }
        if corr.len() != k || corr.iter().any(|row| row.len() != k) {
            return Err(KontractError::MalformedContract(format!(
                "CorrelatedGbmN : matrice de corrélation doit être {k}×{k}"
            )));
        }
        if assets.iter().any(|a| a.sigma < 0.0) {
            return Err(KontractError::MalformedContract(
                "CorrelatedGbmN : toutes les volatilités doivent être ≥ 0".into(),
            ));
        }
        Ok(CorrelatedGbmN {
            assets,
            chol: cholesky_lower(&corr),
        })
    }

    /// Évolution log-normale exacte d'un facteur le long de la grille `times`.
    fn evolve(factor: &GbmFactor, times: &[f64], normals: &[f64]) -> Vec<f64> {
        let mut s = factor.s0;
        let mut prev_t = 0.0_f64;
        // noyau numérique : récurrence séquentielle GBM log-exact (exception CLAUDE.md)
        times
            .iter()
            .zip(normals.iter())
            .map(|(&t, &z)| {
                let dt = t - prev_t;
                if dt > 0.0 {
                    s *= ((factor.mu - 0.5 * factor.sigma * factor.sigma) * dt
                        + factor.sigma * dt.sqrt() * z)
                        .exp();
                }
                prev_t = t;
                s
            })
            .collect()
    }
}

impl Simulator for CorrelatedGbmN {
    /// Interface mono-actif (premier actif). Utilisé par le compilateur ; en pratique
    /// `simulate_paths` est toujours appelé directement par le moteur MC.
    fn simulate(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError> {
        let paths = self.simulate_paths(times, n_paths, seed)?;
        let n = times.len();
        let mut data = vec![0.0_f64; n_paths * n];
        data.par_chunks_mut(n.max(1))
            .zip(paths.par_iter())
            .try_for_each(|(row, path)| {
                let series = path.spot_series(&self.assets[0].name)?;
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
        let k = self.assets.len();
        let n = times.len();
        let chol = &self.chol;
        let assets = &self.assets;

        (0..n_paths)
            .into_par_iter()
            .map(|path_idx| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(seed, path_idx as u64));

                // Par pas de temps : z ~ N(0, I_k), puis w = L·z (matvec triangulaire).
                let all_innovations: Vec<Vec<f64>> = (0..n)
                    .map(|_| {
                        let z: Vec<f64> = (0..k)
                            .map(|_| rng.sample::<f64, _>(StandardNormal))
                            .collect();
                        (0..k)
                            .map(|i| {
                                chol[i]
                                    .iter()
                                    .zip(z.iter())
                                    .take(i + 1)
                                    .map(|(l, zi)| l * zi)
                                    .sum::<f64>()
                            })
                            .collect()
                    })
                    .collect();

                // Transpose [n_steps × k] → [k × n_steps] pour l'évolution par actif.
                let normals_per_asset: Vec<Vec<f64>> = (0..k)
                    .map(|i| all_innovations.iter().map(|step| step[i]).collect())
                    .collect();

                // Évolution GBM log-exacte par actif + peuplement du Path (N actifs).
                normals_per_asset.iter().zip(assets.iter()).try_fold(
                    Path::new(times.to_vec()),
                    |path: Path, (normals, asset)| {
                        path.with_asset(asset.name.clone(), Self::evolve(asset, times, normals))
                    },
                )
            })
            .collect()
    }

    fn asset_name(&self) -> &str {
        &self.assets[0].name
    }
}
