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

    for iter in 0..config.max_iterations {
        // Evaluate objective at current σ.
        let mc_config = McConfig {
            n_paths: config.n_paths,
            seed: 42,
            steps_per_year: 252,
            rate,
            variance_reduction: None,
        };

        let mut obj = 0.0;
        for &(spot, market_price) in market_prices {
            let gbm = Gbm::new("underlying", spot, rate, sigma);
            match crate::pricer::price_gbm(contract, &gbm, &mc_config) {
                Ok(result) => {
                    let diff = result.price - market_price;
                    obj += diff * diff;
                }
                Err(_) => continue,
            }
        }
        obj /= market_prices.len() as f64;

        // Check convergence on objective.
        if (obj_prev - obj).abs() < config.tol_obj {
            converged = true;
            return Ok(CalibrationResult {
                parameters: [sigma].to_vec(),
                objective: obj,
                iterations: iter + 1,
                converged,
            });
        }
        obj_prev = obj;

        // Trust-region step: adjust σ based on gradient approximation.
        let delta_sigma = 0.001;
        let mut obj_up = 0.0;
        for &(spot, market_price) in market_prices {
            let gbm_up = Gbm::new("underlying", spot, rate, sigma + delta_sigma);
            if let Ok(result) = crate::pricer::price_gbm(contract, &gbm_up, &mc_config) {
                let diff = result.price - market_price;
                obj_up += diff * diff;
            }
        }
        obj_up /= market_prices.len() as f64;

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
        parameters: [sigma].to_vec(),
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
    let mut params = [0.04, 2.0, 0.04, 0.3, -0.5].to_vec();

    let mut obj_prev = f64::INFINITY;
    let mut converged = false;

    for _iter in 0..config.max_iterations {
        // Evaluate objective at current parameters.
        let (v0, kappa, theta, sigma_v, rho) =
            (params[0], params[1], params[2], params[3], params[4]);

        let mc_config = McConfig {
            n_paths: config.n_paths,
            seed: 42,
            steps_per_year: 252,
            rate,
            variance_reduction: None,
        };

        let mut obj = 0.0;
        for &(spot, market_price) in market_prices {
            let heston =
                HestonSimulator::new("underlying", spot, v0, kappa, theta, sigma_v, rho, rate);
            match crate::pricer::price_gbm(contract, &heston, &mc_config) {
                Ok(result) => {
                    let diff = result.price - market_price;
                    obj += diff * diff;
                }
                Err(_) => continue,
            }
        }
        obj /= market_prices.len() as f64;

        // Check convergence.
        if (obj_prev - obj).abs() < config.tol_obj {
            converged = true;
            break;
        }
        obj_prev = obj;

        // Simple gradient descent with trust region.
        let delta = [0.001, 0.01, 0.001, 0.01, 0.01].to_vec();

        for i in 0..params.len() {
            let mut params_up = params.clone();
            params_up[i] += delta[i];

            let (v0, kappa, theta, sigma_v, rho) = (
                params_up[0],
                params_up[1],
                params_up[2],
                params_up[3],
                params_up[4],
            );

            let mut obj_up = 0.0;
            for &(spot, market_price) in market_prices {
                let heston =
                    HestonSimulator::new("underlying", spot, v0, kappa, theta, sigma_v, rho, rate);
                if let Ok(result) = crate::pricer::price_gbm(contract, &heston, &mc_config) {
                    let diff = result.price - market_price;
                    obj_up += diff * diff;
                }
            }
            obj_up /= market_prices.len() as f64;

            let grad = (obj_up - obj) / delta[i];
            let step = -(grad * config.trust_radius * 0.1).clamp(-0.02, 0.02);

            params[i] = (params[i] + step).clamp(0.001, 5.0); // Bound parameters
        }

        // Enforce constraints on Heston parameters.
        params[0] = params[0].max(0.001); // v0 > 0
        params[1] = params[1].max(0.01); // kappa > 0
        params[2] = params[2].max(0.001); // theta > 0
        params[3] = params[3].clamp(0.01, 2.0); // sigma_v in (0, 2)
        params[4] = params[4].clamp(-0.99, 0.99); // rho in (-1, 1)
    }

    Ok(CalibrationResult {
        parameters: params,
        objective: obj_prev,
        iterations: config.max_iterations,
        converged,
    })
}
