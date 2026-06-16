//! Calibration de modèles aux prix de marché.
//!
//! Deux familles d'optimiseurs :
//!   - **J21-fast** : trust-region léger (mono-paramètre GBM, descente Heston),
//!     cible < 1 sec pour un fit rapide.
//!   - **J22** : **CMA-ES** global (cf. [`crate::optimizer`]) pour Heston / SABR /
//!     Merton, robuste au bruit Monte-Carlo et aux minima locaux. La calibration
//!     se fait en **common random numbers** (graine MC fixe) → objectif lisse et
//!     reproductible, et en **espace normalisé** `[0,1]ⁿ` (chaque paramètre
//!     ramené à l'échelle de ses bornes) pour que le pas global unique de
//!     CMA-ES traite équitablement des paramètres d'échelles très différentes
//!     (`κ ≈ 2`, `ρ ≈ −0.5`, `v₀ ≈ 0.04`).

use crate::optimizer::{cmaes_minimize, Bounds, CmaesConfig};
use crate::{
    pricer::McConfig, Contract, Gbm, HestonSimulator, KontractError, MertonJumpSimulator,
    SABRSimulator,
};

/// Configuration for fast calibration.
#[derive(Debug, Clone)]
pub struct FastCalibrationConfig {
    /// Number of MC paths for objective evaluation.
    pub n_paths: usize,
    /// Trust-region radius initial value.
    pub trust_radius: f64,
    /// Maximum iterations for optimization.
    pub max_iterations: usize,
    /// Convergence tolerance on parameter change.
    pub tol_param: f64,
    /// Convergence tolerance on objective.
    pub tol_obj: f64,
}

impl Default for FastCalibrationConfig {
    fn default() -> Self {
        FastCalibrationConfig {
            n_paths: 1000,
            trust_radius: 0.1,
            max_iterations: 50,
            tol_param: 1e-4,
            tol_obj: 1e-6,
        }
    }
}

/// Result of calibration: fitted parameters and diagnostics.
#[derive(Debug, Clone)]
pub struct CalibrationResult {
    pub parameters: Vec<f64>,
    pub objective: f64,
    pub iterations: usize,
    pub converged: bool,
}

/// Nom du sous-jacent porté par le contrat (premier actif rencontré).
///
/// La calibration **doit** simuler ce sous-jacent : utiliser un nom codé en dur
/// (ancien « underlying ») produit des trajectoires sans rapport avec les
/// `Spot(name)` du contrat → prix constant (souvent 0) → gradient nul → l'optimiseur
/// reste bloqué sur son point de départ.
fn contract_asset(contract: &Contract) -> String {
    crate::compile(contract)
        .ok()
        .and_then(|plan| plan.assets.into_iter().next())
        .unwrap_or_else(|| "underlying".to_string())
}

/// Prix Monte-Carlo GBM du contrat pour chaque quote `(spot, _)`, à σ donné.
///
/// Renvoie un prix par quote (ordre préservé) ; `NaN` si une évaluation échoue.
fn gbm_prices(
    contract: &Contract,
    asset: &str,
    market_prices: &[(f64, f64)],
    sigma: f64,
    rate: f64,
    mc_config: &McConfig,
) -> Vec<f64> {
    market_prices
        .iter()
        .map(|&(spot, _)| {
            let gbm = Gbm::new(asset, spot, rate, sigma);
            crate::pricer::price_gbm(contract, &gbm, mc_config)
                .map(|r| r.price)
                .unwrap_or(f64::NAN)
        })
        .collect()
}

/// MSE entre un vecteur de prix modèle et les prix de marché.
fn mse(prices: &[f64], market_prices: &[(f64, f64)]) -> f64 {
    let n = market_prices.len().max(1) as f64;
    prices
        .iter()
        .zip(market_prices.iter())
        .map(|(&p, &(_, mkt))| (p - mkt).powi(2))
        .sum::<f64>()
        / n
}

/// Calcule la MSE entre les prix Monte-Carlo Heston et les prix de marché.
fn eval_obj_heston(
    contract: &Contract,
    market_prices: &[(f64, f64)],
    params: &[f64],
    rate: f64,
    mc_config: &McConfig,
) -> f64 {
    let (v0, kappa, theta, sigma_v, rho) = (params[0], params[1], params[2], params[3], params[4]);
    let sum_sq: f64 = market_prices
        .iter()
        .filter_map(|&(spot, market_price)| {
            let heston =
                HestonSimulator::new("underlying", spot, v0, kappa, theta, sigma_v, rho, rate);
            crate::pricer::price_gbm(contract, &heston, mc_config)
                .ok()
                .map(|result| {
                    let diff = result.price - market_price;
                    diff * diff
                })
        })
        .sum();
    sum_sq / market_prices.len() as f64
}

/// Applique les contraintes sur les paramètres Heston.
fn clamp_heston_params(params: Vec<f64>) -> Vec<f64> {
    params
        .into_iter()
        .enumerate()
        .map(|(i, p)| match i {
            0 => p.max(0.001),         // v0 > 0
            1 => p.max(0.01),          // kappa > 0
            2 => p.max(0.001),         // theta > 0
            3 => p.clamp(0.01, 2.0),   // sigma_v in (0, 2)
            4 => p.clamp(-0.99, 0.99), // rho in (-1, 1)
            _ => p,
        })
        .collect()
}

/// Fit GBM volatility from market prices (single parameter: σ).
/// `contract`: payoff to calibrate (e.g., European call)
/// `times`: time grid for simulation
/// `market_prices`: observed prices for different spot values
/// `rate`: risk-free rate
/// Returns fitted σ.
pub fn fit_gbm_volatility(
    contract: &Contract,
    _times: &[f64],
    market_prices: &[(f64, f64)], // (spot, price) pairs
    rate: f64,
    config: &FastCalibrationConfig,
) -> Result<CalibrationResult, KontractError> {
    let mc_config = McConfig {
        n_paths: config.n_paths,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };
    let asset = contract_asset(contract);
    let delta = 1e-3; // pas de différence finie pour le véga

    let mut sigma = 0.20_f64; // point de départ
    let mut converged = false;
    let mut iters = config.max_iterations;

    // **Gauss-Newton amorti** pour la moindre-carré `min_σ Σ (price_i(σ) − mkt_i)²`.
    //
    // À chaque pas : véga_i = ∂price_i/∂σ par différence finie en **common random
    // numbers** (même graine/chemins pour `price` et `price(σ+δ)`), puis
    // `Δσ = − Σ véga_i·r_i / Σ véga_i²` (r_i = price_i − mkt_i). Ce pas est
    // **auto-amortissant** : il tend vers 0 près de l'optimum (contrairement à
    // l'ancien pas à magnitude fixe ±0.05 qui oscillait autour de la solution).
    for iter in 0..config.max_iterations {
        let prices = gbm_prices(contract, &asset, market_prices, sigma, rate, &mc_config);
        let prices_up = gbm_prices(
            contract,
            &asset,
            market_prices,
            sigma + delta,
            rate,
            &mc_config,
        );

        // Jacobien (véga) et résidus, agrégés pour le pas de Gauss-Newton.
        let (num, den) = prices
            .iter()
            .zip(prices_up.iter())
            .zip(market_prices.iter())
            .fold(
                (0.0_f64, 0.0_f64),
                |(num, den), ((&p, &p_up), &(_, mkt))| {
                    let vega = (p_up - p) / delta;
                    let resid = p - mkt;
                    (num + vega * resid, den + vega * vega)
                },
            );

        // Véga négligeable (insensibilité totale) → impossible d'avancer.
        if den < 1e-12 {
            converged = true;
            iters = iter + 1;
            break;
        }

        // Pas de Gauss-Newton, borné par le rayon de confiance pour la stabilité.
        let raw_step = -num / den;
        let step = raw_step.clamp(-config.trust_radius, config.trust_radius);
        let sigma_next = (sigma + step).clamp(0.01, 3.0);

        if (sigma_next - sigma).abs() < config.tol_param {
            sigma = sigma_next;
            converged = true;
            iters = iter + 1;
            break;
        }
        sigma = sigma_next;
    }

    let final_prices = gbm_prices(contract, &asset, market_prices, sigma, rate, &mc_config);
    Ok(CalibrationResult {
        parameters: vec![sigma],
        objective: mse(&final_prices, market_prices),
        iterations: iters,
        converged,
    })
}

/// Fit Heston parameters via trust-region optimization.
/// `contract`: payoff to calibrate
/// `_times`: time grid (unused, included for API consistency)
/// `market_prices`: observed prices (spot, price) pairs
/// `rate`: risk-free rate
/// Initial parameters: [v0, kappa, theta, sigma_v, rho]
/// Returns fitted [v0, kappa, theta, sigma_v, rho].
pub fn fit_heston_parameters(
    contract: &Contract,
    _times: &[f64],
    market_prices: &[(f64, f64)],
    rate: f64,
    config: &FastCalibrationConfig,
) -> Result<CalibrationResult, KontractError> {
    // Initial guess: reasonable Heston parameters.
    let mut params = vec![0.04, 2.0, 0.04, 0.3, -0.5];

    let mut obj_prev = f64::INFINITY;
    let mut converged = false;

    // Boucle de convergence trust-region : conservée (cf. CLAUDE.md exceptions — algorithme itératif avec état multi-paramètres)
    for _iter in 0..config.max_iterations {
        let mc_config = McConfig {
            n_paths: config.n_paths,
            seed: 42,
            steps_per_year: 252,
            rate,
            variance_reduction: None,
        };

        let obj = eval_obj_heston(contract, market_prices, &params, rate, &mc_config);

        // Check convergence.
        if (obj_prev - obj).abs() < config.tol_obj {
            converged = true;
            break;
        }
        obj_prev = obj;

        // Simple gradient descent with trust region.
        let delta = [0.001, 0.01, 0.001, 0.01, 0.01];

        let new_params: Vec<f64> = params
            .iter()
            .enumerate()
            .map(|(i, &p)| {
                let mut params_up = params.clone();
                params_up[i] += delta[i];
                let obj_up = eval_obj_heston(contract, market_prices, &params_up, rate, &mc_config);
                let grad = (obj_up - obj) / delta[i];
                let step = -(grad * config.trust_radius * 0.1).clamp(-0.02, 0.02);
                (p + step).clamp(0.001, 5.0) // Bound parameters
            })
            .collect();

        params = clamp_heston_params(new_params);
    }

    Ok(CalibrationResult {
        parameters: params,
        objective: obj_prev,
        iterations: config.max_iterations,
        converged,
    })
}

// ============================================================================
// J22 — Calibration globale par CMA-ES (Heston / SABR / Merton)
// ============================================================================

/// Construit la configuration MC en common random numbers (graine fixe).
fn crn_mc_config(n_paths: usize, rate: f64) -> McConfig {
    McConfig {
        n_paths,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    }
}

/// Lance CMA-ES sur un objectif `objective`, en **espace normalisé** délimité par
/// `[lower, upper]`, depuis le point initial `x0`.
///
/// CMA-ES travaille sur `u ∈ [0,1]ⁿ` (chaque coordonnée `u_i` mappée vers
/// `lower_i + u_i·(upper_i − lower_i)`) ; le résultat est dé-normalisé vers
/// l'espace des paramètres physiques.
fn run_cmaes_calibration<O>(
    x0: Vec<f64>,
    lower: Vec<f64>,
    upper: Vec<f64>,
    max_generations: usize,
    objective: O,
) -> CalibrationResult
where
    O: Fn(&[f64]) -> f64 + Sync,
{
    let n = x0.len();
    let width: Vec<f64> = upper
        .iter()
        .zip(lower.iter())
        .map(|(&hi, &lo)| (hi - lo).max(1e-12))
        .collect();

    // Objectif en espace normalisé : dé-normalise puis appelle l'objectif physique.
    let obj_u = |u: &[f64]| {
        let param: Vec<f64> = u
            .iter()
            .zip(lower.iter())
            .zip(width.iter())
            .map(|((&ui, &lo), &w)| lo + ui * w)
            .collect();
        objective(&param)
    };

    let u0: Vec<f64> = x0
        .iter()
        .zip(lower.iter())
        .zip(width.iter())
        .map(|((&x, &lo), &w)| ((x - lo) / w).clamp(0.0, 1.0))
        .collect();

    // Population élargie (vs le défaut minimal λ = 4 + ⌊3·ln n⌋ ≈ 7) : la
    // calibration multi-paramètres (SABR/Heston) avec bornes actives est sujette
    // au piégeage en coin (un paramètre collé à sa borne fausse l'adaptation de
    // covariance). Une population plus grande rend la recherche globale robuste.
    let cfg = CmaesConfig {
        population_size: Some((8 * n).max(16)),
        sigma0: 0.4,
        max_generations,
        seed: 42,
        ..Default::default()
    };
    let norm_bounds = Bounds::new(vec![0.0; n], vec![1.0; n]);
    let res = cmaes_minimize(obj_u, &u0, &norm_bounds, &cfg);

    let parameters: Vec<f64> = res
        .best_params
        .iter()
        .zip(lower.iter())
        .zip(width.iter())
        .map(|((&ui, &lo), &w)| lo + ui * w)
        .collect();

    CalibrationResult {
        parameters,
        objective: res.best_objective,
        iterations: res.generations,
        converged: res.converged,
    }
}

/// MSE de reprise de prix SABR (β fixé).
#[allow(clippy::too_many_arguments)] // paramètres SABR explicites (α, β, ν, ρ) + contexte
fn eval_obj_sabr(
    contract: &Contract,
    market_prices: &[(f64, f64)],
    alpha: f64,
    beta: f64,
    nu: f64,
    rho: f64,
    rate: f64,
    mc_config: &McConfig,
) -> f64 {
    let sum_sq: f64 = market_prices
        .iter()
        .filter_map(|&(spot, market_price)| {
            let sabr = SABRSimulator::new("underlying", spot, alpha, beta, nu, rho, rate);
            crate::pricer::price_gbm(contract, &sabr, mc_config)
                .ok()
                .map(|result| (result.price - market_price).powi(2))
        })
        .sum();
    sum_sq / market_prices.len() as f64
}

/// MSE de reprise de prix Merton (saut-diffusion).
#[allow(clippy::too_many_arguments)] // paramètres Merton explicites (σ, λ, μ_j, σ_j) + contexte
fn eval_obj_merton(
    contract: &Contract,
    market_prices: &[(f64, f64)],
    sigma: f64,
    lambda: f64,
    mu_j: f64,
    sigma_j: f64,
    rate: f64,
    mc_config: &McConfig,
) -> f64 {
    let sum_sq: f64 = market_prices
        .iter()
        .filter_map(|&(spot, market_price)| {
            let merton =
                MertonJumpSimulator::new("underlying", spot, rate, sigma, lambda, mu_j, sigma_j);
            crate::pricer::price_gbm(contract, &merton, mc_config)
                .ok()
                .map(|result| (result.price - market_price).powi(2))
        })
        .sum();
    sum_sq / market_prices.len() as f64
}

/// Calibre les 5 paramètres de **Heston** `[v0, κ, θ, σ_v, ρ]` par CMA-ES.
///
/// Bornes admissibles : `v0,θ ∈ [1e-3, 1]`, `κ ∈ [0.1, 10]`, `σ_v ∈ [0.01, 2]`,
/// `ρ ∈ [−0.99, 0.99]`. Critère minimisé : MSE de reprise des prix de marché.
///
/// Note (identifiabilité) : depuis peu de quotes, les 5 paramètres ne sont pas
/// tous individuellement identifiables — la **reprise de prix** (objectif) est la
/// quantité robuste, le round-trip paramétrique exact requiert une surface riche.
pub fn calibrate_heston_cmaes(
    contract: &Contract,
    market_prices: &[(f64, f64)],
    rate: f64,
    config: &FastCalibrationConfig,
) -> Result<CalibrationResult, KontractError> {
    let mc_config = crn_mc_config(config.n_paths, rate);
    let x0 = vec![0.04, 2.0, 0.04, 0.3, -0.5];
    let lower = vec![0.001, 0.1, 0.001, 0.01, -0.99];
    let upper = vec![1.0, 10.0, 1.0, 2.0, 0.99];

    let objective = |p: &[f64]| eval_obj_heston(contract, market_prices, p, rate, &mc_config);
    Ok(run_cmaes_calibration(
        x0,
        lower,
        upper,
        config.max_iterations,
        objective,
    ))
}

/// Calibre **SABR** `[α, ν, ρ]` à `β` **fixé** (pratique de marché) par CMA-ES.
///
/// Le vecteur retourné est `[α, β, ν, ρ]` (β recopié) pour reconstruire
/// directement un [`SABRSimulator`]. Bornes : `α ∈ [1e-3, 2]`, `ν ∈ [1e-3, 3]`,
/// `ρ ∈ [−0.99, 0.99]`.
pub fn calibrate_sabr_cmaes(
    contract: &Contract,
    market_prices: &[(f64, f64)],
    rate: f64,
    beta: f64,
    config: &FastCalibrationConfig,
) -> Result<CalibrationResult, KontractError> {
    let mc_config = crn_mc_config(config.n_paths, rate);
    let x0 = vec![0.2, 0.4, -0.3];
    let lower = vec![0.001, 0.001, -0.99];
    let upper = vec![2.0, 3.0, 0.99];

    let objective = |p: &[f64]| {
        eval_obj_sabr(
            contract,
            market_prices,
            p[0],
            beta,
            p[1],
            p[2],
            rate,
            &mc_config,
        )
    };
    let mut result = run_cmaes_calibration(x0, lower, upper, config.max_iterations, objective);
    // Réinsère β (fixé) pour obtenir [α, β, ν, ρ].
    result.parameters = vec![
        result.parameters[0],
        beta,
        result.parameters[1],
        result.parameters[2],
    ];
    Ok(result)
}

/// Calibre les 4 paramètres de **Merton** `[σ, λ, μ_j, σ_j]` par CMA-ES.
///
/// Bornes : `σ ∈ [0.01, 1]`, `λ ∈ [0, 5]`, `μ_j ∈ [−0.5, 0.5]`,
/// `σ_j ∈ [0.01, 1]`.
pub fn calibrate_merton_cmaes(
    contract: &Contract,
    market_prices: &[(f64, f64)],
    rate: f64,
    config: &FastCalibrationConfig,
) -> Result<CalibrationResult, KontractError> {
    let mc_config = crn_mc_config(config.n_paths, rate);
    let x0 = vec![0.2, 1.0, -0.1, 0.2];
    let lower = vec![0.01, 0.0, -0.5, 0.01];
    let upper = vec![1.0, 5.0, 0.5, 1.0];

    let objective = |p: &[f64]| {
        eval_obj_merton(
            contract,
            market_prices,
            p[0],
            p[1],
            p[2],
            p[3],
            rate,
            &mc_config,
        )
    };
    Ok(run_cmaes_calibration(
        x0,
        lower,
        upper,
        config.max_iterations,
        objective,
    ))
}
