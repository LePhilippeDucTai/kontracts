//! Réduction de variance Monte-Carlo (jalon J15).
//!
//! Deux techniques classiques :
//!
//! 1. **Variables antithétiques** : pour chaque trajectoire `Z`, on simule aussi
//!    `−Z`. La moyenne des deux estimateurs corrélés négativement réduit la variance
//!    (typiquement par un facteur 2 pour les payoffs quasi-linéaires).
//!
//! 2. **Variable de contrôle** : on utilise un call européen ATM dont le prix est
//!    connu analytiquement (Black-Scholes). L'estimateur corrigé est :
//!    `P_cible − β · (P_call_MC − P_call_BS)`.
//!    Pour `β = 1` (covariance unitaire approchée), l'estimateur reste sans biais
//!    et sa variance est réduite si la corrélation est forte (≥ 0.7 en pratique).
//!
//! Ces deux techniques sont **orthogonales** et peuvent être combinées.

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rand_distr::StandardNormal;
use rayon::prelude::*;

use crate::ast::Contract;
pub use crate::numerics::black_scholes_call;
use crate::observable::Path;
use crate::pricer::{price_on_paths, summarize_pvs, PriceResult};
use crate::KontractError;

// ============================================================================
// Configuration
// ============================================================================

/// Configuration des techniques de réduction de variance.
///
/// Ajoutée comme champ optionnel dans [`McConfig`] ; si `None`, le comportement
/// est identique aux jalons J1–J14 (rétrocompatibilité totale).
#[derive(Debug, Clone, Copy, Default)]
pub struct VarianceReductionConfig {
    /// Active la méthode des variables antithétiques.
    pub use_antithetic: bool,
    /// Active la méthode de la variable de contrôle (call européen ATM).
    pub use_control_variate: bool,
}

// Black-Scholes analytique is now in numerics module (imported above).

// ============================================================================
// Variables antithétiques
// ============================================================================

/// Génère une paire de trajectoires antithétiques pour un GBM scalaire.
///
/// Pour chaque demi-chemin d'index `i`, on tire des innovations `Z` puis on
/// construit la trajectoire antithétique avec `−Z`. La moyenne des deux PVs
/// est l'estimateur antithétique (variance réduite).
///
/// # Retourne
/// `(price_base, price_antithetic)` : prix moyen des trajectoires directes et
/// prix moyen des trajectoires antithétiques. L'estimateur final est
/// `(price_base + price_antithetic) / 2`.
pub fn price_with_antithetic(
    contract: &Contract,
    paths_base: &[Path],
    paths_anti: &[Path],
    grid: &[f64],
    rate: f64,
) -> Result<(f64, f64), KontractError> {
    let res_base = price_on_paths(contract, paths_base, grid, rate)?;
    let res_anti = price_on_paths(contract, paths_anti, grid, rate)?;
    Ok((res_base.price, res_anti.price))
}

/// Construit les trajectoires antithétiques d'un GBM à partir d'innovations
/// négées.
///
/// Utilisé en interne par [`price_gbm_antithetic`] ; exposé pour les tests.
pub fn simulate_antithetic_gbm(
    asset: &str,
    s0: f64,
    mu: f64,
    sigma: f64,
    times: &[f64],
    n_half: usize,
    seed: u64,
) -> Result<(Vec<Path>, Vec<Path>), KontractError> {
    use crate::simulator::mix;

    let n_steps = times.len();
    if n_steps == 0 {
        return Err(KontractError::InconsistentPath("grille vide".into()));
    }

    let results: Result<Vec<(Path, Path)>, KontractError> = (0..n_half)
        .into_par_iter()
        .map(|i| {
            let mut rng = ChaCha8Rng::seed_from_u64(mix(seed, i as u64));

            let mut spots_base = vec![0.0f64; n_steps];
            let mut spots_anti = vec![0.0f64; n_steps];
            let mut s_b = s0;
            let mut s_a = s0;
            let mut prev_t = 0.0_f64;

            for (k, &t) in times.iter().enumerate() {
                let dt = t - prev_t;
                if dt > 0.0 {
                    let z: f64 = rng.sample(StandardNormal);
                    let drift = (mu - 0.5 * sigma * sigma) * dt;
                    let diff_scale = sigma * dt.sqrt();
                    s_b *= (drift + diff_scale * z).exp();
                    s_a *= (drift - diff_scale * z).exp(); // antithétique : −z
                }
                spots_base[k] = s_b;
                spots_anti[k] = s_a;
                prev_t = t;
            }

            let path_b = Path::new(times.to_vec())
                .with_asset(asset.to_string(), spots_base)
                .map_err(|e| KontractError::InconsistentPath(e.to_string()))?;
            let path_a = Path::new(times.to_vec())
                .with_asset(asset.to_string(), spots_anti)
                .map_err(|e| KontractError::InconsistentPath(e.to_string()))?;

            Ok((path_b, path_a))
        })
        .collect();

    let pairs = results?;
    let (bases, antis): (Vec<Path>, Vec<Path>) = pairs.into_iter().unzip();
    Ok((bases, antis))
}

// ============================================================================
// Variable de contrôle
// ============================================================================

/// Applique la correction par variable de contrôle.
///
/// L'estimateur corrigé est :
/// ```text
/// P_cv = P_cible − β · (P_call_MC − P_call_BS)
/// ```
/// Avec `β = 1`, l'estimateur reste sans biais et la variance diminue en
/// proportion du carré de la corrélation entre le payoff cible et le call.
///
/// # Paramètres
/// - `price_target`  : prix MC brut du contrat cible
/// - `price_call_mc` : prix MC du call de contrôle (sur les mêmes trajectoires)
/// - `price_call_bs` : prix analytique BS du même call
/// - `beta`          : coefficient de contrôle (1.0 recommandé)
#[inline]
pub fn apply_control_variate(
    price_target: f64,
    price_call_mc: f64,
    price_call_bs: f64,
    beta: f64,
) -> f64 {
    price_target - beta * (price_call_mc - price_call_bs)
}

// ============================================================================
// Pricer avec VR intégré (appelé depuis pricer.rs)
// ============================================================================

/// Prix d'un contrat sur des trajectoires déjà simulées (base + antithétiques),
/// en combinant les deux estimateurs en un seul `PriceResult` avec diagnostics.
///
/// L'estimateur antithétique par paire est :
/// `pv_anti[i] = (pv_base[i] + pv_anti[i]) / 2`
/// ce qui divise la variance par ≥ 2 pour les payoffs convexes.
pub fn price_antithetic_on_paths(
    contract: &Contract,
    paths_base: &[Path],
    paths_anti: &[Path],
    grid: &[f64],
    rate: f64,
) -> Result<PriceResult, KontractError> {
    use crate::pricer::present_value_pub;

    let pvs_base: Result<Vec<f64>, _> = paths_base
        .par_iter()
        .map(|p| present_value_pub(contract, p, grid, rate))
        .collect();
    let pvs_anti: Result<Vec<f64>, _> = paths_anti
        .par_iter()
        .map(|p| present_value_pub(contract, p, grid, rate))
        .collect();

    let pvs_base = pvs_base?;
    let pvs_anti = pvs_anti?;

    // Estimateur antithétique : moyenne des deux PV par paire.
    let pvs_combined: Vec<f64> = pvs_base
        .iter()
        .zip(pvs_anti.iter())
        .map(|(b, a)| 0.5 * (b + a))
        .collect();

    Ok(summarize_pvs(&pvs_combined))
}

/// Prix d'un contrat avec variable de contrôle (call ATM GBM), sur des
/// trajectoires déjà simulées.
///
/// La correction est : `P_cv_i = pv_target_i − β·(pv_call_mc_i − BS_call)`
/// ce qui soustrait le bruit corrélé entre le contrat cible et le call de contrôle.
pub fn price_control_variate_on_paths(
    contract: &Contract,
    control_call: &Contract,
    bs_price: f64,
    beta: f64,
    paths: &[Path],
    grid: &[f64],
    rate: f64,
) -> Result<PriceResult, KontractError> {
    use crate::pricer::present_value_pub;

    let pvs_target: Result<Vec<f64>, _> = paths
        .par_iter()
        .map(|p| present_value_pub(contract, p, grid, rate))
        .collect();
    let pvs_call: Result<Vec<f64>, _> = paths
        .par_iter()
        .map(|p| present_value_pub(control_call, p, grid, rate))
        .collect();

    let pvs_target = pvs_target?;
    let pvs_call = pvs_call?;

    let pvs_cv: Vec<f64> = pvs_target
        .iter()
        .zip(pvs_call.iter())
        .map(|(pt, pc)| apply_control_variate(*pt, *pc, bs_price, beta))
        .collect();

    Ok(summarize_pvs(&pvs_cv))
}
