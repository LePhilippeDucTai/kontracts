//! Fast calibration via trust-region optimization (jalon J21-fast).
//!
//! Lightweight parameter fitting for GBM, Heston, etc.
//! Target: < 1 sec for 100+ market quotes.

use crate::{pricer::McConfig, Contract, Gbm, HestonSimulator, KontractError};

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

/// Calcule la MSE entre les prix Monte-Carlo GBM et les prix de marché.
fn eval_obj_gbm(
    contract: &Contract,
    market_prices: &[(f64, f64)],
    sigma: f64,
    rate: f64,
    mc_config: &McConfig,
) -> f64 {
    let sum_sq: f64 = market_prices
        .iter()
        .filter_map(|&(spot, market_price)| {
            let gbm = Gbm::new("underlying", spot, rate, sigma);
            crate::pricer::price_gbm(contract, &gbm, mc_config)
                .ok()
                .map(|result| {
                    let diff = result.price - market_price;
                    diff * diff
                })
        })
        .sum();
    sum_sq / market_prices.len() as f64
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
    let mut sigma = 0.20; // Initial guess
    let mut obj_prev = f64::INFINITY;
    let mut converged = false;

    // Boucle de convergence trust-region : conservée (cf. CLAUDE.md exceptions — algorithme itératif avec early-return)
    for iter in 0..config.max_iterations {
        let mc_config = McConfig {
            n_paths: config.n_paths,
            seed: 42,
            steps_per_year: 252,
            rate,
            variance_reduction: None,
        };

        let obj = eval_obj_gbm(contract, market_prices, sigma, rate, &mc_config);

        // Check convergence on objective.
        if (obj_prev - obj).abs() < config.tol_obj {
            converged = true;
            return Ok(CalibrationResult {
                parameters: vec![sigma],
                objective: obj,
                iterations: iter + 1,
                converged,
            });
        }
        obj_prev = obj;

        // Trust-region step: adjust σ based on gradient approximation.
        let delta_sigma = 0.001;
        let obj_up = eval_obj_gbm(
            contract,
            market_prices,
            sigma + delta_sigma,
            rate,
            &mc_config,
        );

        let grad = (obj_up - obj) / delta_sigma;
        if grad.abs() < 1e-10 {
            // Gradient too small, stop.
            converged = true;
            break;
        }

        // Newton step with trust region.
        let step = -(grad * config.trust_radius).clamp(-0.05, 0.05);
        sigma = (sigma + step).clamp(0.01, 3.0); // Bound σ in [0.01, 3.0]
    }

    Ok(CalibrationResult {
        parameters: vec![sigma],
        objective: obj_prev,
        iterations: config.max_iterations,
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
