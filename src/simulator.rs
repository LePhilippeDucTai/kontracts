//! Simulateur Monte-Carlo (jalons J3, J12).
//!
//! # GBM (J3)
//!
//! Génère des trajectoires de prix sous un mouvement brownien géométrique (GBM).
//! Le schéma est **exact** (log-normal fermé), donc sans biais de discrétisation :
//!
//! ```text
//! S_{t+dt} = S_t · exp[ (μ − ½σ²)·dt + σ·√dt·Z ],   Z ~ N(0, 1)
//! ```
//!
//! # Heston (J12)
//!
//! Modèle stochastique à volatilité bidimensionnelle (spot, variance) :
//!
//! ```text
//! dS = r·S·dt + √v·S·dW_S
//! dv = κ(θ - v)·dt + σ_v·√v·dW_v
//! dW_S·dW_v = ρ·dt
//! ```
//!
//! Schéma Euler-Milstein ; la variance est planchée à 0 (troncature simple).
//!
//! # Dupire (J12)
//!
//! Volatilité locale déterministe `σ_loc(S, t)` extraite d'une surface de prix
//! d'options européennes via la formule de Dupire :
//!
//! ```text
//! σ_loc(K, T)² = 2·∂C/∂T / (K²·∂²C/∂K²)
//! ```
//!
//! Simulation Euler avec interpolation bilinéaire de σ_loc sur la grille.
//!
//! # Conventions (cf. CLAUDE.md)
//!
//!   - arrays via `ndarray` (`Array2` de forme `[n_paths, n_steps]`),
//!   - parallélisme via `rayon` (une trajectoire par tâche),
//!   - RNG seedable et **reproductible indépendamment de l'ordonnancement** :
//!     chaque trajectoire dérive sa propre graine de `(seed, index)`.
//!
//! ## Point d'extension : le trait [`Simulator`] (jalon J11)
//!
//! Le pricer ne dépend que de l'**interface** [`Simulator`], jamais d'un modèle
//! concret. C'est le **seul** point d'extension du moteur : Heston, Dupire, SABR,
//! Rough Bergomi (J12+) s'y branchent en implémentant ce trait, sans toucher à
//! l'AST, au compilateur, ni à la logique de pricing. Un simulateur produit des
//! trajectoires (`Array2` de spots, ou directement des [`Path`]) ; il connaît
//! ses propres sous-jacents et n'expose aucune sémantique de produit.

use ndarray::Array2;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;
use rayon::prelude::*;

use crate::observable::Path;
use crate::KontractError;

/// Interface des simulateurs de trajectoires Monte-Carlo (jalon J11).
///
/// C'est l'abstraction par laquelle le pricer obtient ses trajectoires. Tout
/// modèle (GBM aujourd'hui ; Heston, Dupire, SABR… en J12+) l'implémente. Le
/// trait est `Send + Sync` pour autoriser le partage entre threads `rayon`.
///
/// Un simulateur est responsable de ses propres sous-jacents : il sait quels
/// actifs il génère et les place dans les [`Path`] produits. Le moteur de pricing
/// reste donc totalement agnostique au modèle.
pub trait Simulator: Send + Sync {
    /// Simule `n_paths` trajectoires sur la grille `times` (en années).
    ///
    /// Renvoie un `Array2` de forme `[n_paths, times.len()]` pour le sous-jacent
    /// principal du simulateur. Pour les simulateurs multi-actifs (J12+), cette
    /// méthode reste centrée sur l'actif principal ; [`Simulator::simulate_paths`]
    /// est le point d'entrée recommandé pour le pricing.
    fn simulate(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError>;

    /// Renvoie un [`Path`] par trajectoire, prêt pour l'évaluation d'observables.
    ///
    /// Implémentation par défaut : construit les [`Path`] à partir de la sortie de
    /// [`Simulator::simulate`], en associant chaque trajectoire au sous-jacent
    /// renvoyé par [`Simulator::asset_name`]. Les simulateurs multi-actifs peuvent
    /// surcharger cette méthode pour peupler plusieurs sous-jacents par `Path`.
    fn simulate_paths(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Vec<Path>, KontractError> {
        let arr = self.simulate(times, n_paths, seed)?;
        let asset = self.asset_name();
        arr.outer_iter()
            .map(|row| Path::new(times.to_vec()).with_asset(asset.to_string(), row.to_vec()))
            .collect()
    }

    /// Nom du sous-jacent principal produit par ce simulateur.
    ///
    /// Sert à l'implémentation par défaut de [`Simulator::simulate_paths`] pour
    /// étiqueter les trajectoires (doit matcher les `Spot(name)` du contrat).
    fn asset_name(&self) -> &str;

    /// Simule des trajectoires **antithétiques** (paires base + −Z) pour la
    /// réduction de variance (jalon J15).
    ///
    /// Renvoie `Some((paths_base, paths_anti))` si le simulateur supporte les
    /// variables antithétiques, `None` sinon. L'implémentation par défaut renvoie
    /// `None` ; les simulateurs GBM la surchargent.
    #[allow(clippy::type_complexity)]
    fn simulate_antithetic_paths(
        &self,
        _times: &[f64],
        _n_half: usize,
        _seed: u64,
    ) -> Result<Option<(Vec<Path>, Vec<Path>)>, KontractError> {
        Ok(None)
    }

    /// Renvoie `(s0, sigma)` pour la construction d'une variable de contrôle
    /// (call ATM, jalon J15). Renvoie `None` si le simulateur n'est pas un GBM
    /// ou ne peut pas fournir ces paramètres.
    fn gbm_params(&self) -> Option<(f64, f64)> {
        None
    }

    /// Simule une **paire couplée** fine / grossière pour Multilevel MC (jalon J18).
    ///
    /// Au niveau `level`, le pas fin est `Δt_f = T / 2^level` et le pas grossier
    /// est `Δt_c = T / 2^(level−1)` (deux fois plus gros). La clé de MLMC est que
    /// les deux discrétisations **partagent les mêmes incréments browniens** : à
    /// chaque pas grossier correspond la somme de deux incréments fins. C'est ce
    /// couplage qui fait décroître la variance `Var(V_fine − V_coarse)` avec le
    /// niveau.
    ///
    /// Renvoie `Some((fine_paths, coarse_paths))` où :
    /// - `fine_paths`  : grille `[0, Δt_f, 2Δt_f, …, T]` (2^level + 1 points) ;
    /// - `coarse_paths`: grille `[0, Δt_c, 2Δt_c, …, T]` (2^(level−1) + 1 points) ;
    /// - les deux vecteurs ont `n_paths` trajectoires **appariées** (même indice =
    ///   mêmes aléas sous-jacents).
    ///
    /// Le niveau 0 est dégénéré (1 seul pas, pas de niveau grossier) : renvoie
    /// `Some((fine, vec![]))`.
    ///
    /// L'implémentation par défaut renvoie `None` (modèle non couplable). GBM et
    /// Heston la surchargent ; ces deux modèles suffisent au critère du jalon.
    #[allow(clippy::type_complexity)]
    fn simulate_level_pair(
        &self,
        _t_max: f64,
        _level: usize,
        _n_paths: usize,
        _seed: u64,
    ) -> Result<Option<(Vec<Path>, Vec<Path>)>, KontractError> {
        Ok(None)
    }
}

/// Grille uniforme `[0, T/n, 2T/n, …, T]` (n+1 points).
fn uniform_grid(t_max: f64, n_steps: usize) -> Vec<f64> {
    let n = n_steps.max(1);
    (0..=n).map(|k| t_max * (k as f64) / (n as f64)).collect()
}

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

/// Le GBM est le simulateur de référence (jalon J11). L'implémentation du trait
/// délègue aux méthodes inhérentes : la logique numérique GBM reste **inchangée**
/// (équivalence bit-pour-bit), seule l'interface est ajoutée.
impl Simulator for Gbm {
    fn simulate(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError> {
        Gbm::simulate(self, times, n_paths, seed)
    }

    fn simulate_paths(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Vec<Path>, KontractError> {
        Gbm::simulate_paths(self, times, n_paths, seed)
    }

    fn asset_name(&self) -> &str {
        &self.asset
    }

    #[allow(clippy::type_complexity)]
    fn simulate_antithetic_paths(
        &self,
        times: &[f64],
        n_half: usize,
        seed: u64,
    ) -> Result<Option<(Vec<Path>, Vec<Path>)>, KontractError> {
        use crate::variance_reduction::simulate_antithetic_gbm;
        let (bases, antis) = simulate_antithetic_gbm(
            &self.asset,
            self.s0,
            self.mu,
            self.sigma,
            times,
            n_half,
            seed,
        )?;
        Ok(Some((bases, antis)))
    }

    fn gbm_params(&self) -> Option<(f64, f64)> {
        Some((self.s0, self.sigma))
    }

    #[allow(clippy::type_complexity)]
    fn simulate_level_pair(
        &self,
        t_max: f64,
        level: usize,
        n_paths: usize,
        seed: u64,
    ) -> Result<Option<(Vec<Path>, Vec<Path>)>, KontractError> {
        let n_fine = 1usize << level; // 2^level
        let fine_grid = uniform_grid(t_max, n_fine);
        let dt_f = t_max / n_fine as f64;
        let drift_f = (self.mu - 0.5 * self.sigma * self.sigma) * dt_f;
        let sqrt_dt_f = dt_f.sqrt();

        // Niveau grossier : un pas pour deux pas fins (sauf niveau 0).
        let coarse: bool = level > 0;
        let n_coarse = if coarse { 1usize << (level - 1) } else { 0 };
        let coarse_grid = if coarse {
            uniform_grid(t_max, n_coarse)
        } else {
            Vec::new()
        };
        let dt_c = if coarse { t_max / n_coarse as f64 } else { 0.0 };
        let drift_c = (self.mu - 0.5 * self.sigma * self.sigma) * dt_c;

        let mut fine = Vec::with_capacity(n_paths);
        let mut coarse_paths = Vec::with_capacity(n_paths);

        // Génération séquentielle des aléas par trajectoire (seed dérivée de
        // (seed, level, index)), parallélisée ensuite si nécessaire. Ici on reste
        // simple : une trajectoire par boucle (le pricer parallélise déjà).
        let rows: Vec<(Vec<f64>, Vec<f64>)> = (0..n_paths)
            .into_par_iter()
            .map(|i| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(mix(seed, level as u64 + 1), i as u64));

                // Trajectoire fine.
                let mut row_f = vec![0.0f64; fine_grid.len()];
                let mut s_f = self.s0;
                row_f[0] = s_f;
                // On mémorise les incréments fins pour reconstruire le grossier.
                let mut incs = vec![0.0f64; n_fine];
                for k in 0..n_fine {
                    let z: f64 = rng.sample(StandardNormal);
                    incs[k] = z;
                    s_f *= (drift_f + self.sigma * sqrt_dt_f * z).exp();
                    row_f[k + 1] = s_f;
                }

                // Trajectoire grossière : Δt_c = 2·Δt_f, incrément = somme de deux
                // incréments fins (couplage MLMC), variance 2·dt_f = dt_c.
                let row_c = if coarse {
                    let mut row_c = vec![0.0f64; coarse_grid.len()];
                    let mut s_c = self.s0;
                    row_c[0] = s_c;
                    for j in 0..n_coarse {
                        let dw = incs[2 * j] + incs[2 * j + 1]; // ~ N(0, 2)
                                                                // diffusion = σ·√dt_f·dW (car dW porte déjà 2 pas de √dt_f)
                        s_c *= (drift_c + self.sigma * sqrt_dt_f * dw).exp();
                        row_c[j + 1] = s_c;
                    }
                    row_c
                } else {
                    Vec::new()
                };

                (row_f, row_c)
            })
            .collect();

        for (row_f, row_c) in rows {
            fine.push(Path::new(fine_grid.clone()).with_asset(self.asset.clone(), row_f)?);
            if coarse {
                coarse_paths
                    .push(Path::new(coarse_grid.clone()).with_asset(self.asset.clone(), row_c)?);
            }
        }

        Ok(Some((fine, coarse_paths)))
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
pub(crate) fn mix(seed: u64, index: u64) -> u64 {
    seed ^ index.wrapping_mul(0x9E37_79B9_7F4A_7C15)
}

// ============================================================================
// Heston (J12) — simulateur stochastique à 2 dimensions (spot, variance)
// ============================================================================

/// Paramètres du modèle de Heston.
///
/// Dynamiques risque-neutres :
///
/// ```text
/// dS = r·S·dt + √v·S·dW_S
/// dv = κ(θ - v)·dt + σ_v·√v·dW_v
/// dW_S·dW_v = ρ·dt
/// ```
///
/// La variance `v` est planchée à 0 (troncature simple) ; la condition de Feller
/// `2κθ ≥ σ_v²` n'est pas vérifiée — c'est à l'utilisateur de calibrer
/// correctement ses paramètres.
#[derive(Debug, Clone)]
pub struct HestonSimulator {
    /// Nom du sous-jacent (doit matcher les `Spot(name)` du contrat).
    pub asset: String,
    /// Prix spot initial `S_0`.
    pub s0: f64,
    /// Variance initiale `v_0` (variance, pas volatilité).
    pub v0: f64,
    /// Taux de retour à la moyenne `κ`.
    pub kappa: f64,
    /// Variance à long terme `θ`.
    pub theta: f64,
    /// Vol de vol `σ_v`.
    pub sigma_v: f64,
    /// Corrélation spot-vol `ρ ∈ [-1, 1]`.
    pub rho: f64,
    /// Taux risque-neutre `r` (drift + discount).
    pub r: f64,
}

impl HestonSimulator {
    /// Construit un `HestonSimulator` depuis ses paramètres.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asset: impl Into<String>,
        s0: f64,
        v0: f64,
        kappa: f64,
        theta: f64,
        sigma_v: f64,
        rho: f64,
        r: f64,
    ) -> Self {
        HestonSimulator {
            asset: asset.into(),
            s0,
            v0,
            kappa,
            theta,
            sigma_v,
            rho,
            r,
        }
    }

    /// Simule une trajectoire Heston avec schéma Euler-Milstein 2D.
    ///
    /// Les deux mouvements browniens corrélés sont construits par décomposition
    /// de Cholesky :
    ///
    /// ```text
    /// dW_S = Z1·√dt
    /// dW_v = (ρ·Z1 + √(1−ρ²)·Z2)·√dt
    /// ```
    fn simulate_one_path(&self, times: &[f64], rng: &mut ChaCha8Rng) -> Vec<f64> {
        let n_steps = times.len();
        let mut row = vec![0.0f64; n_steps];

        let rho_perp = (1.0 - self.rho * self.rho).max(0.0).sqrt();
        let mut s = self.s0;
        let mut v = self.v0.max(0.0);
        let mut prev_t = 0.0_f64;

        for (k, &t) in times.iter().enumerate() {
            let dt = t - prev_t;
            if dt > 0.0 {
                let sqrt_dt = dt.sqrt();
                let z1: f64 = rng.sample(StandardNormal);
                let z2: f64 = rng.sample(StandardNormal);

                let sqrt_v = v.sqrt();
                let dw_s = z1 * sqrt_dt;
                let dw_v = (self.rho * z1 + rho_perp * z2) * sqrt_dt;

                // Euler log-spot (évite les spots négatifs)
                s *= (self.r * dt - 0.5 * v * dt + sqrt_v * dw_s).exp();

                // Euler variance avec plancher à 0
                v = (v + self.kappa * (self.theta - v) * dt + self.sigma_v * sqrt_v * dw_v)
                    .max(0.0);
            }
            // Stocker le spot après évolution (même convention que Gbm :
            // si dt=0, s est inchangé, donc row[0] = s0 quand times[0] = 0).
            row[k] = s;
            prev_t = t;
        }
        row
    }
}

impl Simulator for HestonSimulator {
    fn simulate(
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
                let path = self.simulate_one_path(times, &mut rng);
                row.copy_from_slice(&path);
            });

        Array2::from_shape_vec((n_paths, n_steps), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    fn asset_name(&self) -> &str {
        &self.asset
    }

    #[allow(clippy::type_complexity)]
    fn simulate_level_pair(
        &self,
        t_max: f64,
        level: usize,
        n_paths: usize,
        seed: u64,
    ) -> Result<Option<(Vec<Path>, Vec<Path>)>, KontractError> {
        let n_fine = 1usize << level;
        let fine_grid = uniform_grid(t_max, n_fine);
        let coarse: bool = level > 0;
        let n_coarse = if coarse { 1usize << (level - 1) } else { 0 };
        let coarse_grid = if coarse {
            uniform_grid(t_max, n_coarse)
        } else {
            Vec::new()
        };

        let rho_perp = (1.0 - self.rho * self.rho).max(0.0).sqrt();
        let dt_f = t_max / n_fine as f64;

        // Schéma Euler-Milstein commun, piloté par une suite d'incréments (z1, z2)
        // fins. Le grossier rejoue les incréments fins par paires sommées (dt_c =
        // 2·dt_f), garantissant le couplage browninien des deux discrétisations.
        let simulate_from_incs = |incs: &[(f64, f64)], grid_len: usize, dt: f64| -> Vec<f64> {
            let sqrt_dt = dt.sqrt();
            let mut row = vec![0.0f64; grid_len];
            let mut s = self.s0;
            let mut v = self.v0.max(0.0);
            row[0] = s;
            for (step, &(z1, z2)) in incs.iter().enumerate() {
                let sqrt_v = v.sqrt();
                let dw_s = z1 * sqrt_dt;
                let dw_v = (self.rho * z1 + rho_perp * z2) * sqrt_dt;
                s *= (self.r * dt - 0.5 * v * dt + sqrt_v * dw_s).exp();
                v = (v + self.kappa * (self.theta - v) * dt + self.sigma_v * sqrt_v * dw_v)
                    .max(0.0);
                row[step + 1] = s;
            }
            row
        };

        let rows: Vec<(Vec<f64>, Vec<f64>)> = (0..n_paths)
            .into_par_iter()
            .map(|i| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(mix(seed, level as u64 + 1), i as u64));

                // Incréments fins en N(0,1) ; le pas fin les met à l'échelle √dt_f.
                let mut incs_f = Vec::with_capacity(n_fine);
                for _ in 0..n_fine {
                    let z1: f64 = rng.sample(StandardNormal);
                    let z2: f64 = rng.sample(StandardNormal);
                    incs_f.push((z1, z2));
                }
                let row_f = simulate_from_incs(&incs_f, fine_grid.len(), dt_f);

                let row_c = if coarse {
                    // Pas grossier : dW couplé = (z_a + z_b)/√2 → N(0,1) à l'échelle
                    // √dt_c = √(2·dt_f), donc dW_c·√dt_c = (z_a+z_b)·√dt_f.
                    let mut incs_c = Vec::with_capacity(n_coarse);
                    for j in 0..n_coarse {
                        let (z1a, z2a) = incs_f[2 * j];
                        let (z1b, z2b) = incs_f[2 * j + 1];
                        let inv_sqrt2 = std::f64::consts::FRAC_1_SQRT_2;
                        incs_c.push(((z1a + z1b) * inv_sqrt2, (z2a + z2b) * inv_sqrt2));
                    }
                    let dt_c = t_max / n_coarse as f64;
                    simulate_from_incs(&incs_c, coarse_grid.len(), dt_c)
                } else {
                    Vec::new()
                };

                (row_f, row_c)
            })
            .collect();

        let mut fine = Vec::with_capacity(n_paths);
        let mut coarse_paths = Vec::with_capacity(n_paths);
        for (row_f, row_c) in rows {
            fine.push(Path::new(fine_grid.clone()).with_asset(self.asset.clone(), row_f)?);
            if coarse {
                coarse_paths
                    .push(Path::new(coarse_grid.clone()).with_asset(self.asset.clone(), row_c)?);
            }
        }

        Ok(Some((fine, coarse_paths)))
    }
}

/// Constructeur fonctionnel pratique (alias de [`HestonSimulator::new`]).
#[allow(clippy::too_many_arguments)]
pub fn heston_from_params(
    asset: &str,
    s0: f64,
    v0: f64,
    kappa: f64,
    theta: f64,
    sigma_v: f64,
    rho: f64,
    r: f64,
) -> HestonSimulator {
    HestonSimulator::new(asset, s0, v0, kappa, theta, sigma_v, rho, r)
}

// ============================================================================
// Dupire (J12) — simulateur à volatilité locale sur grille
// ============================================================================

/// Simulateur à **volatilité locale** de Dupire.
///
/// La surface `σ_loc(S, t)` est stockée sur une grille bidimensionnelle
/// `(time_grid × spot_grid)` et interpolée bilinéairement à chaque pas.
///
/// Extraction depuis une surface d'options européennes via [`dupire_from_gbm_calls`].
#[derive(Debug, Clone)]
pub struct DupireSimulator {
    /// Nom du sous-jacent.
    pub asset: String,
    /// Prix spot initial `S_0`.
    pub s0: f64,
    /// Taux risque-neutre.
    pub r: f64,
    /// Grille de spots (axe colonne de `local_vol`), croissante.
    pub spot_grid: Vec<f64>,
    /// Grille de temps (axe ligne de `local_vol`), croissante, en années.
    pub time_grid: Vec<f64>,
    /// Surface de vol locale `σ_loc(t, S)`, shape `[n_times, n_spots]`.
    pub local_vol: Array2<f64>,
}

impl DupireSimulator {
    /// Renvoie `σ_loc(S, t)` par interpolation bilinéaire sur la grille.
    ///
    /// Hors grille : clamp sur les bords (extrapolation plate).
    fn sigma_loc(&self, s: f64, t: f64) -> f64 {
        let (ti, tf) = interp_index(&self.time_grid, t);
        let (si, sf) = interp_index(&self.spot_grid, s);

        let n_t = self.local_vol.nrows();
        let n_s = self.local_vol.ncols();

        let ti1 = (ti + 1).min(n_t - 1);
        let si1 = (si + 1).min(n_s - 1);

        let v00 = self.local_vol[[ti, si]];
        let v01 = self.local_vol[[ti, si1]];
        let v10 = self.local_vol[[ti1, si]];
        let v11 = self.local_vol[[ti1, si1]];

        // Interpolation bilinéaire
        let v0 = v00 * (1.0 - sf) + v01 * sf;
        let v1 = v10 * (1.0 - sf) + v11 * sf;
        v0 * (1.0 - tf) + v1 * tf
    }

    /// Simule une trajectoire Dupire (Euler log-normal avec σ_loc(S, t)).
    fn simulate_one_path(&self, times: &[f64], rng: &mut ChaCha8Rng) -> Vec<f64> {
        let n_steps = times.len();
        let mut row = vec![0.0f64; n_steps];
        let mut s = self.s0;
        let mut prev_t = 0.0_f64;

        for (k, &t) in times.iter().enumerate() {
            let dt = t - prev_t;
            if dt > 0.0 {
                // Utilise s et prev_t (= t de début du pas) pour σ_loc
                let sigma = self.sigma_loc(s, prev_t);
                let z: f64 = rng.sample(StandardNormal);
                s *= (self.r * dt - 0.5 * sigma * sigma * dt + sigma * dt.sqrt() * z).exp();
            }
            // Stocker après évolution (même convention que Gbm)
            row[k] = s;
            prev_t = t;
        }
        row
    }
}

impl Simulator for DupireSimulator {
    fn simulate(
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
                let path = self.simulate_one_path(times, &mut rng);
                row.copy_from_slice(&path);
            });

        Array2::from_shape_vec((n_paths, n_steps), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    fn asset_name(&self) -> &str {
        &self.asset
    }
}

/// Extrait un [`DupireSimulator`] depuis une grille de prix de calls européens.
///
/// L'entrée est une surface de prix `C(K, T)` sur une grille rectangulaire
/// `strikes × maturities`. Les prix sont supposés cohérents (pas de crossing de
/// call spreads). Le modèle sous-jacent peut être quelconque ; la fonction
/// applique la formule de Dupire :
///
/// ```text
/// σ_loc(K, T)² = 2·∂C/∂T / (K²·∂²C/∂K²)
/// ```
///
/// où les dérivées sont calculées par différences finies (centrées en K, en
/// avance en T). La surface résultante est clampée dans `[σ_min, σ_max]` pour
/// éviter les instabilités numériques.
///
/// # Arguments
///
/// * `asset` — nom du sous-jacent.
/// * `s0` — prix spot initial.
/// * `r` — taux risque-neutre.
/// * `strikes` — grille de strikes (croissante, ≥ 3 points).
/// * `maturities` — grille de maturités en années (croissante, ≥ 2 points).
/// * `call_prices` — matrice `[n_maturities × n_strikes]` de prix de calls.
///
/// # Errors
///
/// Renvoie [`KontractError::MalformedContract`] si les grilles sont trop petites
/// ou si `call_prices` n'a pas la bonne dimension.
pub fn dupire_from_gbm_calls(
    asset: &str,
    s0: f64,
    r: f64,
    strikes: &[f64],
    maturities: &[f64],
    call_prices: &[f64], // longueur = n_maturities * n_strikes, ligne-majeure [mat × strike]
) -> Result<DupireSimulator, KontractError> {
    let n_k = strikes.len();
    let n_t = maturities.len();

    if n_k < 3 {
        return Err(KontractError::MalformedContract(
            "Dupire : au moins 3 strikes requis pour ∂²C/∂K²".into(),
        ));
    }
    if n_t < 2 {
        return Err(KontractError::MalformedContract(
            "Dupire : au moins 2 maturités requises pour ∂C/∂T".into(),
        ));
    }
    if call_prices.len() != n_t * n_k {
        return Err(KontractError::MalformedContract(format!(
            "Dupire : call_prices.len()={} ≠ n_t×n_k={}×{}={}",
            call_prices.len(),
            n_t,
            n_k,
            n_t * n_k
        )));
    }

    // Accès C(t_i, k_j) = call_prices[i * n_k + j]
    let c = |ti: usize, ki: usize| call_prices[ti * n_k + ki];

    // On calcule n_t − 1 rangées de vol locale via différences finies en avance.
    // Chaque rangée `ti` donne σ_loc représentatif de l'intervalle [T_ti, T_{ti+1}].
    // La simulation utilisera σ_loc[ti] pour les pas de temps dans cet intervalle.
    let n_t_out = n_t - 1;
    let n_k_out = n_k;

    let sigma_min = 1e-4_f64;
    let sigma_max = 5.0_f64;

    let mut local_vol_data = vec![0.0f64; n_t_out * n_k_out];

    for ti in 0..n_t_out {
        let dt = maturities[ti + 1] - maturities[ti];
        for ki in 0..n_k_out {
            let k = strikes[ki];

            // ∂C/∂T : différence en avance entre T_ti et T_{ti+1}
            let dc_dt = (c(ti + 1, ki) - c(ti, ki)) / dt;

            // ∂C/∂K et ∂²C/∂K² : différences centrales non-uniformes à T_ti
            let dk_prev = if ki == 0 {
                strikes[1] - strikes[0]
            } else {
                strikes[ki] - strikes[ki - 1]
            };
            let dk_next = if ki == n_k - 1 {
                strikes[n_k - 1] - strikes[n_k - 2]
            } else {
                strikes[ki + 1] - strikes[ki]
            };

            let c_prev = if ki == 0 { c(ti, 0) } else { c(ti, ki - 1) };
            let c_next = if ki == n_k - 1 {
                c(ti, n_k - 1)
            } else {
                c(ti, ki + 1)
            };
            let c_mid = c(ti, ki);

            let h1 = dk_prev;
            let h2 = dk_next;

            // ∂C/∂K : différence centrale (C(K+h2) − C(K−h1)) / (h1 + h2)
            let dc_dk = (c_next - c_prev) / (h1 + h2);

            // ∂²C/∂K² : différence centrale du 2nd ordre pour grille non-uniforme
            let d2c_dk2 =
                2.0 * (c_next / h2 - c_mid * (1.0 / h1 + 1.0 / h2) + c_prev / h1) / (h1 + h2);

            // Formule de Dupire complète (taux ≠ 0) :
            // σ_loc² = 2·(∂C/∂T + r·K·∂C/∂K) / (K²·∂²C/∂K²)
            //
            // Note : ∂C/∂K < 0 (call décroissant en K), donc r·K·∂C/∂K < 0
            // réduit le numérateur. Sans ce terme (r=0), on suresti­me σ_loc.
            let numerator = 2.0 * (dc_dt + r * k * dc_dk);
            let denominator = k * k * d2c_dk2;

            let sigma_loc_sq = if denominator > 1e-14 && numerator > 0.0 {
                (numerator / denominator).min(sigma_max * sigma_max)
            } else {
                sigma_min * sigma_min
            };

            local_vol_data[ti * n_k_out + ki] = sigma_loc_sq.sqrt().clamp(sigma_min, sigma_max);
        }
    }

    let local_vol = Array2::from_shape_vec((n_t_out, n_k_out), local_vol_data)
        .map_err(|e| KontractError::InconsistentPath(e.to_string()))?;

    // Grille de temps : σ_loc[ti] est estimé par différence finie sur [T_ti, T_{ti+1}].
    // Il représente la vol locale au **milieu** `(T_ti + T_{ti+1}) / 2` (biais de
    // l'approximation en avance). On positionne chaque jalon au milieu de son
    // intervalle pour que l'interpolation bilinéaire converge vers le bon point.
    // Le clamping à plat hors grille extrapolera σ_loc[0] vers t=0 et σ_loc[n-1]
    // au-delà de la dernière maturité.
    let time_grid: Vec<f64> = (0..n_t_out)
        .map(|ti| 0.5 * (maturities[ti] + maturities[ti + 1]))
        .collect();

    Ok(DupireSimulator {
        asset: asset.to_string(),
        s0,
        r,
        spot_grid: strikes.to_vec(),
        time_grid,
        local_vol,
    })
}

// ============================================================================
// SABR (J13) — modèle CEV stochastique (smile, marché de taux)
// ============================================================================

/// Simulateur SABR (Stochastic Alpha Beta Rho).
///
/// Dynamiques risque-neutres pour le forward log-spot `f = ln S` :
///
/// ```text
/// df   = α · f^β · dW_f
/// dα   = ν · α · dW_α
/// dW_f · dW_α = ρ · dt
/// ```
///
/// où :
/// - `α` est la volatilité initiale (≈ `σ₀`),
/// - `β ∈ (0, 1]` est l'exposant CEV (β=1 → GBM-like),
/// - `ν` est la volatilité de volatilité,
/// - `ρ ∈ [-1, 1]` est la corrélation forward/vol.
///
/// Le drift risque-neutre `r` est ajouté lors de la récupération du spot :
/// `S_{t+dt} = exp(f_{t+dt}) · exp(-r · dt)` n'est **pas** la bonne façon
/// de faire — on intègre plutôt `d(ln S) = r·dt + α·S^(β-1)·dW_f` ce qui
/// revient à ajouter `r·dt` au log-forward à chaque pas.
#[derive(Debug, Clone)]
pub struct SABRSimulator {
    /// Nom du sous-jacent (doit matcher les `Spot(name)` du contrat).
    pub asset: String,
    /// Prix spot initial `S_0`.
    pub s0: f64,
    /// Volatilité initiale `α` (= `σ₀`).
    pub alpha: f64,
    /// Exposant CEV `β ∈ (0, 1]` (β=1 → GBM).
    pub beta: f64,
    /// Vol-de-vol `ν`.
    pub nu: f64,
    /// Corrélation spot/vol `ρ ∈ [-1, 1]`.
    pub rho: f64,
    /// Taux risque-neutre `r` (drift + discount).
    pub r: f64,
}

impl SABRSimulator {
    /// Construit un `SABRSimulator`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asset: impl Into<String>,
        s0: f64,
        alpha: f64,
        beta: f64,
        nu: f64,
        rho: f64,
        r: f64,
    ) -> Self {
        SABRSimulator {
            asset: asset.into(),
            s0,
            alpha,
            beta,
            nu,
            rho,
            r,
        }
    }

    /// Simule une trajectoire SABR par schéma d'Euler.
    ///
    /// Deux browniens corrélés par décomposition de Cholesky :
    /// ```text
    /// dW_f = Z1 · √dt
    /// dW_α = (ρ·Z1 + √(1−ρ²)·Z2) · √dt
    /// ```
    fn simulate_one_path(&self, times: &[f64], rng: &mut ChaCha8Rng) -> Vec<f64> {
        let n_steps = times.len();
        let mut row = vec![0.0f64; n_steps];

        let rho_perp = (1.0 - self.rho * self.rho).max(0.0).sqrt();

        // Travailler en log-spot pour éviter les spots négatifs
        let mut ln_s = self.s0.ln();
        let mut alpha = self.alpha.max(1e-10);
        let mut prev_t = 0.0_f64;

        // Stocker S_0 à t=0 (même convention que GBM : row[0] = s0 si times[0]=0)
        if !times.is_empty() {
            row[0] = self.s0;
        }

        for (k, &t) in times.iter().enumerate() {
            let dt = t - prev_t;
            if dt > 0.0 {
                let sqrt_dt = dt.sqrt();
                let z1: f64 = rng.sample(StandardNormal);
                let z2: f64 = rng.sample(StandardNormal);

                let dw_f = z1 * sqrt_dt;
                let dw_alpha = (self.rho * z1 + rho_perp * z2) * sqrt_dt;

                // S courant (avant évolution) = exp(ln_s)
                let s_cur = ln_s.exp();

                // Évolution du log-spot :
                // d(ln S) = r·dt - ½·α²·S^(2β-2)·dt + α·S^(β-1)·dW_f
                // En Euler : ln_s += r·dt + α·s_cur^(β-1)·dW_f - ½·(α·s_cur^(β-1))²·dt
                let sigma_eff = alpha * s_cur.powf(self.beta - 1.0);
                ln_s += self.r * dt - 0.5 * sigma_eff * sigma_eff * dt + sigma_eff * dw_f;

                // Évolution de la vol SABR : dα = ν·α·dW_α (log-normal)
                // Utiliser le schéma exact pour α (log-normal) : αexp
                alpha *= (self.nu * dw_alpha - 0.5 * self.nu * self.nu * dt).exp();
                alpha = alpha.max(1e-10); // plancher pour stabilité numérique
            }
            row[k] = ln_s.exp();
            prev_t = t;
        }
        row
    }
}

impl Simulator for SABRSimulator {
    fn simulate(
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
                let path = self.simulate_one_path(times, &mut rng);
                row.copy_from_slice(&path);
            });

        Array2::from_shape_vec((n_paths, n_steps), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    fn asset_name(&self) -> &str {
        &self.asset
    }
}

/// Constructeur fonctionnel pratique pour [`SABRSimulator`].
#[allow(clippy::too_many_arguments)]
pub fn sabr_from_params(
    asset: &str,
    s0: f64,
    alpha: f64,
    beta: f64,
    nu: f64,
    rho: f64,
    r: f64,
) -> SABRSimulator {
    SABRSimulator::new(asset, s0, alpha, beta, nu, rho, r)
}

// ============================================================================
// Merton Jump-Diffusion (J13) — GBM + sauts de Poisson composés
// ============================================================================

/// Simulateur de Merton à sauts.
///
/// Dynamiques risque-neutres :
///
/// ```text
/// dS/S = (r − λ·κ) · dt + σ · dW + J · dN(λ)
/// ```
///
/// où :
/// - `λ` = intensité des sauts (sauts/an),
/// - `J` = multiplicateur de saut (log-normal : `ln J ~ N(μ_j − σ_j²/2, σ_j²)`),
/// - `κ = E[J] − 1 = exp(μ_j) − 1` (ajustement risque-neutre),
/// - `σ_j` = volatilité de saut.
///
/// La simulation utilise un schéma log-Euler avec distribution de Poisson pour
/// le nombre de sauts à chaque pas, et des sauts log-normaux composés.
#[derive(Debug, Clone)]
pub struct MertonJumpSimulator {
    /// Nom du sous-jacent.
    pub asset: String,
    /// Prix spot initial `S_0`.
    pub s0: f64,
    /// Taux risque-neutre `r`.
    pub r: f64,
    /// Volatilité de diffusion `σ`.
    pub sigma: f64,
    /// Intensité de Poisson `λ` (sauts/an).
    pub lambda: f64,
    /// Rendement moyen de saut `μ_j` (log de la moyenne du multiplicateur).
    pub mu_j: f64,
    /// Volatilité de saut `σ_j`.
    pub sigma_j: f64,
}

impl MertonJumpSimulator {
    /// Construit un `MertonJumpSimulator`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asset: impl Into<String>,
        s0: f64,
        r: f64,
        sigma: f64,
        lambda: f64,
        mu_j: f64,
        sigma_j: f64,
    ) -> Self {
        MertonJumpSimulator {
            asset: asset.into(),
            s0,
            r,
            sigma,
            lambda,
            mu_j,
            sigma_j,
        }
    }

    /// `E[J] − 1` : ajustement risque-neutre pour les sauts (κ dans la littérature).
    ///
    /// Pour `ln J ~ N(μ_j − σ_j²/2, σ_j²)`, `E[J] = exp(μ_j)`.
    fn kappa(&self) -> f64 {
        self.mu_j.exp() - 1.0
    }

    /// Simule une trajectoire Merton (Euler log-normal + Poisson).
    ///
    /// À chaque pas :
    /// 1. Tire `N ~ Poisson(λ·dt)` (nombre de sauts).
    /// 2. Si N > 0, tire N sauts log-normaux et les multiplie.
    /// 3. Applique le schéma Euler ajusté par `−λ·κ·dt`.
    fn simulate_one_path(&self, times: &[f64], rng: &mut ChaCha8Rng) -> Vec<f64> {
        let n_steps = times.len();
        let mut row = vec![0.0f64; n_steps];

        let kappa = self.kappa();
        let mut ln_s = self.s0.ln();
        let mut prev_t = 0.0_f64;

        // Paramètre log-normal des sauts : ln J ~ N(mu_ln, sigma_j²)
        // E[J] = exp(mu_ln + sigma_j²/2) = exp(mu_j) → mu_ln = mu_j - sigma_j²/2
        let mu_ln = self.mu_j - 0.5 * self.sigma_j * self.sigma_j;

        for (k, &t) in times.iter().enumerate() {
            let dt = t - prev_t;
            if dt > 0.0 {
                let sqrt_dt = dt.sqrt();

                // Diffusion GBM
                let z: f64 = rng.sample(StandardNormal);
                let drift_adj = self.r - self.lambda * kappa - 0.5 * self.sigma * self.sigma;
                ln_s += drift_adj * dt + self.sigma * sqrt_dt * z;

                // Sauts de Poisson composés
                // Nombre de sauts : simulation par inversion de la CDF de Poisson
                let n_jumps = poisson_sample(self.lambda * dt, rng);
                for _ in 0..n_jumps {
                    let zj: f64 = rng.sample(StandardNormal);
                    // ln J_i ~ N(mu_ln, sigma_j²)
                    ln_s += mu_ln + self.sigma_j * zj;
                }
            }
            row[k] = ln_s.exp();
            prev_t = t;
        }
        row
    }
}

impl Simulator for MertonJumpSimulator {
    fn simulate(
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
                let path = self.simulate_one_path(times, &mut rng);
                row.copy_from_slice(&path);
            });

        Array2::from_shape_vec((n_paths, n_steps), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    fn asset_name(&self) -> &str {
        &self.asset
    }
}

/// Construit un `MertonJumpSimulator` (alias fonctionnel).
#[allow(clippy::too_many_arguments)]
pub fn merton_from_params(
    asset: &str,
    s0: f64,
    r: f64,
    sigma: f64,
    lambda: f64,
    mu_j: f64,
    sigma_j: f64,
) -> MertonJumpSimulator {
    MertonJumpSimulator::new(asset, s0, r, sigma, lambda, mu_j, sigma_j)
}

/// Tire un entier selon la loi de Poisson de paramètre `lambda` par inversion CDF.
///
/// Pour `lambda ≤ 30` (courant en MC financier avec `λ·dt ≤ 5·dt`), cette méthode
/// est exacte et rapide. Pour les grandes intensités, elle reste correcte mais moins
/// efficace — mais SABR/Merton n'utilisent jamais λ·dt > 5 en pratique.
fn poisson_sample(lambda: f64, rng: &mut ChaCha8Rng) -> u32 {
    if lambda <= 0.0 {
        return 0;
    }
    // Algorithme de Knuth (inversion exponentielle)
    let l = (-lambda).exp();
    let mut k = 0u32;
    let mut p = 1.0_f64;
    loop {
        k += 1;
        let u: f64 = rng.gen();
        p *= u;
        if p <= l {
            return k - 1;
        }
        // Garde-fou pour éviter les boucles infinies si lambda très grand
        if k > 1000 {
            return k;
        }
    }
}

// ============================================================================
// Rough Bergomi (J14) — volatilité rugueuse via mouvement brownien fractionnaire
// ============================================================================

/// Simulateur **Rough Bergomi** (Bayer, Friz, Gatheral 2016).
///
/// La log-variance est pilotée par un **mouvement brownien fractionnaire** (fBm)
/// d'exposant de Hurst `H < 0.5`, ce qui produit des trajectoires de volatilité
/// *rugueuses* (non lisses) reproduisant le smile de volatilité observé sur les
/// marchés. Dynamiques risque-neutres :
///
/// ```text
/// dS_t            = r·S_t·dt + √v_t·S_t·dW_t
/// d(log v_t)      = (ξ·H·t^{H-½})·dt + ξ·dB_t^H
/// dW_t·dB_t       = ρ·dt
/// ```
///
/// où `B_t^H` est un fBm d'exposant `H ∈ (0, 1)` (rugueux si `H < ½`, brownien
/// standard si `H = ½`).
///
/// # Génération du fBm : décomposition de Cholesky (exacte)
///
/// Les incréments du fBm sont **à mémoire longue** (non markoviens) : on ne peut
/// pas les générer par un schéma d'Euler pas-à-pas. On génère donc la trajectoire
/// **entière** d'un coup. Pour `n` pas, on construit la matrice de covariance
///
/// ```text
/// Cov[B_i^H, B_j^H] = ½·(|i|^{2H} + |j|^{2H} − |i−j|^{2H})
/// ```
///
/// puis sa factorisation de Cholesky `Cov = L·Lᵀ`. Chaque trajectoire est alors
/// `B = L·Z` avec `Z ~ N(0, I)`. C'est **exact** mais en `O(n²)` mémoire/temps
/// pour la factorisation (partagée entre toutes les trajectoires) et `O(n²)` par
/// trajectoire pour le produit matrice-vecteur.
///
/// # Corrélation spot/vol (approximation connue)
///
/// Le fBm est piloté par les gaussiennes `Z` (`B = L·Z`). À chaque pas, le
/// brownien du spot est corrélé à la **dernière innovation** `Z[idx]` du fBm
/// (terme diagonal `L[idx][idx]·Z[idx]`) mélangée à un brownien indépendant :
/// `dW = ρ·Z[idx]·√dt + √(1−ρ²)·dB^⊥`.
///
/// **Limitation** : l'incrément complet `ΔB^H = B_{t} − B_{t−dt}` recharge aussi
/// les innovations passées (`Z[j]`, `j < idx`) du fait de la mémoire longue. En
/// ne corrélant le spot qu'à `Z[idx]`, la corrélation spot/vol **réalisée** est
/// atténuée en magnitude par rapport au paramètre `ρ` (le **signe**, donc l'effet
/// de levier et le skew, reste correct). C'est l'approximation habituelle des
/// schémas « fBm exact + spot Euler » ; une corrélation exacte demanderait de
/// projeter l'incrément complet du fBm. Acceptable pour un modèle MVP branchable ;
/// à raffiner si une calibration fine du skew sur `ρ` est requise (J21-fast).
///
/// # Bornes numériques
///
/// La matrice de covariance fBm devient mal conditionnée pour `H` très petit
/// (< 0,01) ou `n` très grand (> 1000). Plages recommandées pour J14 :
/// `H ∈ [0.05, 0.3]`, `n ≤ 250` pas. Au-delà, la Cholesky peut perdre en
/// précision (la diagonale est planchée pour rester définie positive).
#[derive(Debug, Clone)]
pub struct RoughBergomiSimulator {
    /// Nom du sous-jacent (doit matcher les `Spot(name)` du contrat).
    pub asset: String,
    /// Prix spot initial `S_0`.
    pub s0: f64,
    /// Variance initiale `v_0` (la log-vol initiale vaut `ln v_0`).
    pub v0: f64,
    /// Vol-de-vol `ξ` (amplitude du fBm).
    pub xi: f64,
    /// Exposant de Hurst `H ∈ (0, 1)` (typiquement 0,05–0,3).
    pub h: f64,
    /// Corrélation spot–vol `ρ ∈ [-1, 1]`.
    pub rho: f64,
    /// Taux risque-neutre `r` (drift + discount).
    pub r: f64,
}

impl RoughBergomiSimulator {
    /// Construit un `RoughBergomiSimulator`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        asset: impl Into<String>,
        s0: f64,
        v0: f64,
        xi: f64,
        h: f64,
        rho: f64,
        r: f64,
    ) -> Self {
        RoughBergomiSimulator {
            asset: asset.into(),
            s0,
            v0,
            xi,
            h,
            rho,
            r,
        }
    }

    /// Construit la matrice de covariance du fBm sur les **incréments de temps**
    /// de la grille, puis renvoie sa factorisation de Cholesky inférieure `L`.
    ///
    /// On indexe par les instants `times[1..]` (le premier point `t=0` porte
    /// `B_0 = 0` et n'entre pas dans la covariance). La covariance fBm continue
    /// est `Cov[B_s, B_t] = ½·(s^{2H} + t^{2H} − |s−t|^{2H})`.
    ///
    /// Renvoie une matrice `m × m` avec `m = n_steps − 1` (nombre de points de
    /// temps strictement positifs). Si `m == 0`, renvoie une matrice vide.
    fn fbm_cholesky(&self, times: &[f64]) -> Vec<Vec<f64>> {
        // Instants strictement positifs (B est défini à partir de t>0 ; B_0=0).
        let pts: Vec<f64> = times.iter().copied().filter(|&t| t > 0.0).collect();
        let m = pts.len();
        if m == 0 {
            return Vec::new();
        }

        let two_h = 2.0 * self.h;
        let mut cov = vec![vec![0.0f64; m]; m];
        for i in 0..m {
            for j in 0..m {
                let s = pts[i];
                let t = pts[j];
                let cov_ij = 0.5 * (s.powf(two_h) + t.powf(two_h) - (s - t).abs().powf(two_h));
                cov[i][j] = cov_ij;
            }
        }
        cholesky_lower(&cov)
    }

    /// Échantillonne `n_paths` trajectoires du **fBm sous-jacent** `B_t^H` sur
    /// les instants strictement positifs de `times`.
    ///
    /// Renvoie un `Array2` de forme `[n_paths, m]` où `m` est le nombre d'instants
    /// `> 0` dans `times`. Utile pour inspecter la rugosité (analyse R/S, scaling
    /// de variance `Var[B_t] = t^{2H}`) indépendamment de la dynamique de prix.
    pub fn simulate_fbm(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError> {
        validate_grid(times)?;
        let chol = self.fbm_cholesky(times);
        let m = chol.len();

        let mut data = vec![0.0f64; n_paths * m];
        if m > 0 {
            data.par_chunks_mut(m).enumerate().for_each(|(i, row)| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(seed, i as u64));
                let z: Vec<f64> = (0..m)
                    .map(|_| rng.sample::<f64, _>(StandardNormal))
                    .collect();
                for (ii, cell) in row.iter_mut().enumerate() {
                    let mut acc = 0.0;
                    for (jj, &zj) in z.iter().enumerate().take(ii + 1) {
                        acc += chol[ii][jj] * zj;
                    }
                    *cell = acc;
                }
            });
        }

        Array2::from_shape_vec((n_paths, m), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    /// Simule une trajectoire Rough Bergomi à partir d'une factorisation `L`
    /// pré-calculée et d'un RNG dédié.
    ///
    /// `chol` est la Cholesky inférieure de la covariance fBm sur les instants
    /// strictement positifs (taille `m = nb d'instants > 0`).
    fn simulate_one_path(
        &self,
        times: &[f64],
        chol: &[Vec<f64>],
        rng: &mut ChaCha8Rng,
    ) -> Vec<f64> {
        let n_steps = times.len();
        let mut row = vec![0.0f64; n_steps];

        // Indices des pas à dt>0 (parallèles aux lignes de `chol`).
        // On tire un vecteur Z (gaussiennes i.i.d.) de taille m, qui pilote à la
        // fois le fBm (B = L·Z) et — via les incréments browniens — la corrélation
        // du spot.
        let m = chol.len();
        let z: Vec<f64> = (0..m)
            .map(|_| rng.sample::<f64, _>(StandardNormal))
            .collect();

        // fBm aux instants strictement positifs : B = L·Z.
        let mut fbm = vec![0.0f64; m];
        for (i, fbm_i) in fbm.iter_mut().enumerate() {
            let mut acc = 0.0;
            // L est triangulaire inférieure : somme sur j ≤ i.
            for (j, &zj) in z.iter().enumerate().take(i + 1) {
                acc += chol[i][j] * zj;
            }
            *fbm_i = acc;
        }

        let rho_perp = (1.0 - self.rho * self.rho).max(0.0).sqrt();
        let log_v0 = self.v0.max(1e-300).ln();

        let two_h = two_h_of(self.h);
        let mut s = self.s0;
        row[0] = s; // convention : row[0] = s0 si times[0] = 0
        let mut prev_t = 0.0_f64;

        // `idx` parcourt les colonnes de `chol`/`fbm` (instants > 0).
        let mut idx = 0usize;

        for (k, &t) in times.iter().enumerate() {
            let dt = t - prev_t;
            if dt > 0.0 {
                let sqrt_dt = dt.sqrt();
                let z_idx = z[idx];
                let fbm_t = fbm[idx];

                // Log-variance à l'instant courant t (forme Rough Bergomi) :
                //   log v_t = log v_0 + ξ·B_t^H − ½·ξ²·t^{2H}
                // Le terme −½·ξ²·t^{2H} est la **correction de convexité d'Itô** :
                // comme Var[B_t^H] = t^{2H}, on a E[exp(ξ·B_t^H)] = exp(½·ξ²·t^{2H}),
                // donc le retrancher donne E[v_t] = v_0 **exactement** (forward
                // variance plat). Ce n'est pas l'intégrale du drift ξ·H·t^{H-½}·dt
                // (qui vaudrait ½·ξ·t^{2H}), mais bien le terme martingale en ξ².
                let log_v = log_v0 + self.xi * fbm_t - 0.5 * self.xi * self.xi * t.powf(two_h);
                let v = log_v.exp();
                let sqrt_v = v.sqrt();

                // Brownien du spot corrélé au fBm. On corrèle le spot à la
                // **dernière innovation** z_idx du fBm (le terme diagonal
                // L[idx][idx]·z_idx de B_t) mélangée à un brownien indépendant.
                // C'est une approximation : l'incrément complet ΔB^H recharge aussi
                // les innovations passées (mémoire longue), donc la corrélation
                // spot/vol réalisée est atténuée en magnitude par rapport à ρ.
                // Le **signe** (effet de levier) reste correct. Voir doc du struct.
                let z_indep: f64 = rng.sample(StandardNormal);
                let dw = (self.rho * z_idx + rho_perp * z_indep) * sqrt_dt;

                // Spot : log-Euler avec la variance instantanée v.
                s *= (self.r * dt - 0.5 * v * dt + sqrt_v * dw).exp();

                idx += 1;
            }
            row[k] = s;
            prev_t = t;
        }
        row
    }
}

/// `2H` — petit utilitaire pour éviter de recalculer dans la boucle chaude.
#[inline]
fn two_h_of(h: f64) -> f64 {
    2.0 * h
}

impl Simulator for RoughBergomiSimulator {
    fn simulate(
        &self,
        times: &[f64],
        n_paths: usize,
        seed: u64,
    ) -> Result<Array2<f64>, KontractError> {
        validate_grid(times)?;
        let n_steps = times.len();

        // Factorisation de Cholesky calculée **une seule fois** et partagée par
        // toutes les trajectoires (c'est le coût O(n²) amorti du Rough Bergomi).
        let chol = self.fbm_cholesky(times);

        let mut data = vec![0.0f64; n_paths * n_steps];
        data.par_chunks_mut(n_steps.max(1))
            .enumerate()
            .for_each(|(i, row)| {
                let mut rng = ChaCha8Rng::seed_from_u64(mix(seed, i as u64));
                let path = self.simulate_one_path(times, &chol, &mut rng);
                row.copy_from_slice(&path);
            });

        Array2::from_shape_vec((n_paths, n_steps), data)
            .map_err(|e| KontractError::InconsistentPath(e.to_string()))
    }

    fn asset_name(&self) -> &str {
        &self.asset
    }
}

/// Constructeur fonctionnel pratique pour [`RoughBergomiSimulator`].
#[allow(clippy::too_many_arguments)]
pub fn rough_bergomi_from_params(
    asset: &str,
    s0: f64,
    v0: f64,
    xi: f64,
    h: f64,
    rho: f64,
    r: f64,
) -> RoughBergomiSimulator {
    RoughBergomiSimulator::new(asset, s0, v0, xi, h, rho, r)
}

/// Factorisation de Cholesky inférieure `L` (telle que `A = L·Lᵀ`) d'une matrice
/// symétrique semi-définie positive `a` (stockée en lignes).
///
/// Implémentation dense classique en `O(n³)`. Pour rester robuste face aux
/// covariances fBm légèrement mal conditionnées (H petit, n grand), les pivots
/// négatifs par erreur d'arrondi sont planchés à 0 (la matrice est alors traitée
/// comme semi-définie positive plutôt que définie positive). Aucune dépendance
/// LAPACK n'est requise.
fn cholesky_lower(a: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let n = a.len();
    let mut l = vec![vec![0.0f64; n]; n];
    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[i][j];
            // Produit scalaire des lignes i et j sur [0, j) ; l'indexation par k
            // est nécessaire (les deux lignes de `l` sont empruntées en lecture).
            #[allow(clippy::needless_range_loop)]
            for k in 0..j {
                sum -= l[i][k] * l[j][k];
            }
            if i == j {
                // Pivot diagonal : plancher à 0 pour absorber le bruit d'arrondi
                // (covariance fBm SPD en théorie, semi-définie en pratique).
                l[i][j] = sum.max(0.0).sqrt();
            } else {
                let pivot = l[j][j];
                l[i][j] = if pivot > 0.0 { sum / pivot } else { 0.0 };
            }
        }
    }
    l
}

// ============================================================================
// Utilitaires communs aux simulateurs J12
// ============================================================================

/// Trouve l'index inférieur et la fraction d'interpolation dans une grille
/// croissante. Retourne `(index, fraction)` avec `fraction ∈ [0, 1]`.
///
/// Si `x` est hors grille, clamp au bord correspondant (fraction = 0 ou 1).
fn interp_index(grid: &[f64], x: f64) -> (usize, f64) {
    let n = grid.len();
    if n == 0 {
        return (0, 0.0);
    }
    if x <= grid[0] {
        return (0, 0.0);
    }
    if x >= grid[n - 1] {
        return (n - 1, 0.0);
    }
    // Recherche binaire de l'intervalle
    let mut lo = 0usize;
    let mut hi = n - 1;
    while hi - lo > 1 {
        let mid = (lo + hi) / 2;
        if grid[mid] <= x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let frac = (x - grid[lo]) / (grid[hi] - grid[lo]);
    (lo, frac.clamp(0.0, 1.0))
}
