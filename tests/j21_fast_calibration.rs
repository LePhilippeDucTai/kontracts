use kontract::{
    pricer::McConfig,
    products::{european_call, european_put},
    {fit_gbm_volatility, fit_heston_parameters, CalibrationResult, FastCalibrationConfig},
    {Gbm, HestonSimulator},
};

#[test]
fn test_fit_gbm_volatility_atm() {
    // Calibrate GBM volatility from synthetic ATM call prices.
    let true_vol = 0.25;
    let spot = 100.0;
    let strike = 100.0;
    let maturity = 1.0;
    let rate = 0.05;

    // Generate synthetic market prices under true_vol.
    let gbm = Gbm::new("underlying", spot, rate, true_vol);
    let contract = european_call("underlying", strike, maturity, "USD");
    let mc_config = McConfig {
        n_paths: 10000,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };

    let market_price = kontract::price_gbm(&contract, &gbm, &mc_config)
        .expect("Pricing failed")
        .price;

    // Fit volatility from this price.
    let market_prices = vec![(spot, market_price)];
    let config = FastCalibrationConfig {
        n_paths: 5000,
        ..Default::default()
    };

    let result = fit_gbm_volatility(&contract, &[maturity], &market_prices, rate, &config)
        .expect("Calibration failed");

    assert!(
        (result.parameters[0] - true_vol).abs() < 0.05,
        "Fit vol {:.4} should be close to true {:.4}",
        result.parameters[0],
        true_vol
    );
    assert!(
        result.objective < 0.1,
        "Objective {:.6} should be small",
        result.objective
    );
}

#[test]
fn test_fit_gbm_volatility_multiple_spots() {
    // Fit on multiple spot prices (moneyness variation).
    let true_vol = 0.20;
    let rate = 0.05;
    let strike = 100.0;
    let maturity = 0.5;

    let contract = european_call("underlying", strike, maturity, "USD");
    let mc_config = McConfig {
        n_paths: 10000,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };

    // Generate market prices at different spots.
    let mut market_prices = vec![];
    for spot in &[90.0, 100.0, 110.0] {
        let gbm = Gbm::new("underlying", *spot, rate, true_vol);
        let price = kontract::price_gbm(&contract, &gbm, &mc_config)
            .expect("Pricing failed")
            .price;
        market_prices.push((*spot, price));
    }

    let config = FastCalibrationConfig {
        n_paths: 5000,
        ..Default::default()
    };

    let result = fit_gbm_volatility(&contract, &[maturity], &market_prices, rate, &config)
        .expect("Calibration failed");

    assert!(
        (result.parameters[0] - true_vol).abs() < 0.05,
        "Fit vol {:.4} should recover true {:.4}",
        result.parameters[0],
        true_vol
    );
}

#[test]
fn test_fit_gbm_put() {
    // Fit GBM vol from put prices.
    let true_vol = 0.30;
    let spot = 100.0;
    let strike = 100.0;
    let maturity = 1.0;
    let rate = 0.05;

    let gbm = Gbm::new("underlying", spot, rate, true_vol);
    let contract = european_put("underlying", strike, maturity, "USD");
    let mc_config = McConfig {
        n_paths: 10000,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };

    let market_price = kontract::price_gbm(&contract, &gbm, &mc_config)
        .expect("Pricing failed")
        .price;

    let market_prices = vec![(spot, market_price)];
    let config = FastCalibrationConfig {
        n_paths: 5000,
        ..Default::default()
    };

    let result = fit_gbm_volatility(&contract, &[maturity], &market_prices, rate, &config)
        .expect("Calibration failed");

    assert!(
        (result.parameters[0] - true_vol).abs() < 0.05,
        "Put vol {:.4} should recover true {:.4}",
        result.parameters[0],
        true_vol
    );
}

#[test]
fn test_calibration_config_default() {
    let config = FastCalibrationConfig::default();
    assert_eq!(config.n_paths, 1000);
    assert_eq!(config.max_iterations, 50);
    assert!(config.tol_param > 0.0);
    assert!(config.tol_obj > 0.0);
}

#[test]
fn test_fit_gbm_convergence_fast() {
    // Quick check: does fit complete in reasonable time for small dataset?
    let true_vol = 0.25;
    let spot = 100.0;
    let strike = 100.0;
    let maturity = 0.25;
    let rate = 0.05;

    let gbm = Gbm::new("underlying", spot, rate, true_vol);
    let contract = european_call("underlying", strike, maturity, "USD");
    let mc_config = McConfig {
        n_paths: 5000,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };

    let market_price = kontract::price_gbm(&contract, &gbm, &mc_config)
        .expect("Pricing failed")
        .price;

    let market_prices = vec![(spot, market_price)];
    let config = FastCalibrationConfig {
        n_paths: 2000,
        max_iterations: 30,
        ..Default::default()
    };

    let result = fit_gbm_volatility(&contract, &[maturity], &market_prices, rate, &config)
        .expect("Calibration failed");

    // Should converge (or at least not blow up).
    assert!(result.iterations <= config.max_iterations);
    assert!(result.objective.is_finite(), "Objective should be finite");
}

#[test]
fn test_fit_heston_basic() {
    // Basic Heston fit: check it runs and returns reasonable params.
    let spot = 100.0;
    let strike = 100.0;
    let maturity = 1.0;
    let rate = 0.05;

    // True Heston parameters.
    let v0 = 0.04;
    let kappa = 2.0;
    let theta = 0.04;
    let sigma_v = 0.3;
    let rho = -0.5;

    let heston = HestonSimulator::new("underlying", spot, v0, kappa, theta, sigma_v, rho, rate);
    let contract = european_call("underlying", strike, maturity, "USD");
    let mc_config = McConfig {
        n_paths: 5000,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };

    let market_price = kontract::price_gbm(&contract, &heston, &mc_config)
        .expect("Pricing failed")
        .price;

    let market_prices = vec![(spot, market_price)];
    let config = FastCalibrationConfig {
        n_paths: 3000,
        max_iterations: 30,
        ..Default::default()
    };

    let result = fit_heston_parameters(&contract, &[maturity], &market_prices, rate, &config)
        .expect("Calibration failed");

    // Check: all 5 parameters returned.
    assert_eq!(result.parameters.len(), 5);

    // Check: parameters are in reasonable bounds.
    assert!(
        result.parameters[0] > 0.0 && result.parameters[0] < 1.0,
        "v0 out of bounds"
    );
    assert!(
        result.parameters[1] > 0.0 && result.parameters[1] < 5.0,
        "kappa out of bounds"
    );
    assert!(
        result.parameters[2] > 0.0 && result.parameters[2] < 1.0,
        "theta out of bounds"
    );
    assert!(
        result.parameters[3] > 0.0 && result.parameters[3] < 2.0,
        "sigma_v out of bounds"
    );
    assert!(
        result.parameters[4] > -1.0 && result.parameters[4] < 1.0,
        "rho out of bounds"
    );

    // Objective should be finite.
    assert!(result.objective.is_finite(), "Objective should be finite");
}

#[test]
fn test_fit_gbm_vol_bounds() {
    // Verify fitted vol stays in bounds [0.01, 3.0].
    let spot = 100.0;
    let strike = 80.0; // Deep ITM
    let maturity = 0.1;
    let rate = 0.05;

    let contract = european_call("underlying", strike, maturity, "USD");
    let mc_config = McConfig {
        n_paths: 5000,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };

    // Generate a price with extreme vol.
    let extreme_vol = 0.80;
    let gbm = Gbm::new("underlying", spot, rate, extreme_vol);
    let market_price = kontract::price_gbm(&contract, &gbm, &mc_config)
        .expect("Pricing failed")
        .price;

    let market_prices = vec![(spot, market_price)];
    let config = FastCalibrationConfig {
        n_paths: 3000,
        ..Default::default()
    };

    let result = fit_gbm_volatility(&contract, &[maturity], &market_prices, rate, &config)
        .expect("Calibration failed");

    // Fitted vol should be in bounds.
    assert!(
        result.parameters[0] >= 0.01 && result.parameters[0] <= 3.0,
        "Fitted vol {:.4} out of bounds",
        result.parameters[0]
    );
}

#[test]
fn test_calibration_result_fields() {
    // Verify CalibrationResult structure.
    let result = CalibrationResult {
        parameters: vec![0.25],
        objective: 0.001,
        iterations: 15,
        converged: true,
    };

    assert_eq!(result.parameters.len(), 1);
    assert!(result.objective > 0.0);
    assert!(result.iterations > 0);
    assert!(result.converged);
}

#[test]
fn test_fit_gbm_low_vol() {
    // Fit very low volatility (sensitive test).
    let true_vol = 0.05;
    let spot = 100.0;
    let strike = 100.0;
    let maturity = 1.0;
    let rate = 0.05;

    let gbm = Gbm::new("underlying", spot, rate, true_vol);
    let contract = european_call("underlying", strike, maturity, "USD");
    let mc_config = McConfig {
        n_paths: 10000,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };

    let market_price = kontract::price_gbm(&contract, &gbm, &mc_config)
        .expect("Pricing failed")
        .price;

    let market_prices = vec![(spot, market_price)];
    let config = FastCalibrationConfig {
        n_paths: 5000,
        ..Default::default()
    };

    let result = fit_gbm_volatility(&contract, &[maturity], &market_prices, rate, &config)
        .expect("Calibration failed");

    assert!(
        (result.parameters[0] - true_vol).abs() < 0.06,
        "Low vol {:.4} should recover true {:.4}",
        result.parameters[0],
        true_vol
    );
}

#[test]
fn test_fit_heston_single_quote() {
    // Single option quote (minimal data).
    let spot = 100.0;
    let strike = 100.0;
    let maturity = 0.5;
    let rate = 0.05;

    let heston = HestonSimulator::new("underlying", spot, 0.05, 1.5, 0.05, 0.2, -0.3, rate);
    let contract = european_call("underlying", strike, maturity, "USD");
    let mc_config = McConfig {
        n_paths: 5000,
        seed: 42,
        steps_per_year: 252,
        rate,
        variance_reduction: None,
    };

    let market_price = kontract::price_gbm(&contract, &heston, &mc_config)
        .expect("Pricing failed")
        .price;

    let market_prices = vec![(spot, market_price)];
    let config = FastCalibrationConfig {
        n_paths: 2000,
        max_iterations: 20,
        ..Default::default()
    };

    let result = fit_heston_parameters(&contract, &[maturity], &market_prices, rate, &config)
        .expect("Calibration failed");

    assert_eq!(result.parameters.len(), 5);
    assert!(result.objective.is_finite());
}
