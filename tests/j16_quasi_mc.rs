use kontract::*;

#[test]
fn sobol_call_pricing_basic() {
    // Test 1: Basic Sobol GBM pricing of EU call
    let call = products::european_call("AAPL", 110.0, 1.0, "USD");
    let sobol_gbm = SobolGbm::new("AAPL", 100.0, 0.05, 0.2);

    let cfg = McConfig {
        n_paths: 10_000,
        seed: 42,
        steps_per_year: 50,
        rate: 0.05,
        variance_reduction: None,
    };

    let result = price_gbm(&call, &sobol_gbm, &cfg).expect("pricing failed");

    // Sobol call should be a valid finite number (not NaN or infinite)
    assert!(result.price.is_finite(), "Sobol call price is not finite");
    // Price should be reasonable for OTM call (K=110, S=100): 0 to 10
    assert!(
        result.price >= 0.0 && result.price < 30.0,
        "Sobol call price out of range: {:.4}",
        result.price
    );
}

#[test]
fn sobol_vs_gbm_same_seed() {
    // Test 2: Sobol GBM vs standard GBM at same parameters
    let put = products::european_put("AAPL", 100.0, 1.0, "USD");

    let sobol_gbm = SobolGbm::new("AAPL", 100.0, 0.05, 0.2);
    let std_gbm = Gbm::new("AAPL", 100.0, 0.05, 0.2);

    let cfg = McConfig {
        n_paths: 50_000,
        seed: 42,
        steps_per_year: 50,
        rate: 0.05,
        variance_reduction: None,
    };

    let sobol_result = price_gbm(&put, &sobol_gbm, &cfg).expect("sobol pricing failed");
    let std_result = price_gbm(&put, &std_gbm, &cfg).expect("std pricing failed");

    // Both should produce valid positive prices (they use different RNG, so may differ)
    assert!(
        sobol_result.price > 0.0 && sobol_result.price < 50.0,
        "Sobol price invalid: {:.4}",
        sobol_result.price
    );
    assert!(
        std_result.price > 0.0 && std_result.price < 50.0,
        "Std price invalid: {:.4}",
        std_result.price
    );
}

#[test]
fn sobol_convergence_trend() {
    // Test 3: Verify Sobol produces valid paths across path counts
    let call = products::european_call("TEST", 100.0, 1.0, "USD");
    let sobol_gbm = SobolGbm::new("TEST", 100.0, 0.05, 0.2);

    let base_cfg = McConfig {
        n_paths: 1000,
        seed: 42,
        steps_per_year: 50,
        rate: 0.05,
        variance_reduction: None,
    };

    // Run at multiple path counts
    for n_paths in &[1000, 5000, 10_000] {
        let cfg = McConfig {
            n_paths: *n_paths,
            ..base_cfg.clone()
        };
        let result = price_gbm(&call, &sobol_gbm, &cfg).expect("pricing failed");

        // All should produce positive finite prices
        assert!(result.price.is_finite() && result.price > 0.0);
        assert!(result.std_error > 0.0);
        assert!(result.std_error < result.price);
    }
}

#[test]
fn sobol_matches_black_scholes() {
    // Régression : le prix Sobol doit coller à Black-Scholes analytique.
    // (Garde-fou contre le biais d'inversion normale d'Acklam corrigé : l'ancienne
    //  version sous-dispersait les tirages → prix ~60–70 % sous BS.)
    let call = products::european_call("X", 100.0, 1.0, "USD");
    let sobol_gbm = SobolGbm::new("X", 100.0, 0.05, 0.2);

    let cfg = McConfig {
        n_paths: 32_768,
        seed: 42,
        steps_per_year: 50,
        rate: 0.05,
        variance_reduction: None,
    };

    let result = price_gbm(&call, &sobol_gbm, &cfg).expect("pricing failed");
    let bs = numerics::black_scholes_call(100.0, 100.0, 1.0, 0.05, 0.2);

    let rel_err = (result.price - bs).abs() / bs;
    assert!(
        rel_err < 0.02,
        "Sobol call {:.4} doit coller à BS {:.4} (err rel {:.4} ≥ 0.02)",
        result.price,
        bs,
        rel_err
    );
}

#[test]
fn sobol_forward_is_martingale() {
    // Le forward prépayé actualisé doit valoir S0 (propriété de martingale).
    // Test direct de la justesse de la distribution simulée (E[S_T·e^{-rT}] = S0).
    use kontract::ast::{at, one, scale, spot, when};
    let s0 = 100.0;
    let fwd = when(at(1.0), scale(spot("X"), one("USD")));
    let sobol_gbm = SobolGbm::new("X", s0, 0.05, 0.2);

    let cfg = McConfig {
        n_paths: 32_768,
        seed: 42,
        steps_per_year: 1,
        rate: 0.05,
        variance_reduction: None,
    };

    let result = price_gbm(&fwd, &sobol_gbm, &cfg).expect("pricing failed");
    let rel_err = (result.price - s0).abs() / s0;
    assert!(
        rel_err < 0.01,
        "Forward Sobol {:.4} doit valoir S0={} (err rel {:.4} ≥ 0.01)",
        result.price,
        s0,
        rel_err
    );
}

#[test]
fn sobol_barrier_knockout() {
    // Test 4: Sobol on a knock-out barrier call
    let ko_call = products::up_and_out_call("IBM", 110.0, 120.0, 1.0, "USD");
    let sobol_gbm = SobolGbm::new("IBM", 100.0, 0.05, 0.25);

    let cfg = McConfig {
        n_paths: 20_000,
        seed: 42,
        steps_per_year: 100,
        rate: 0.05,
        variance_reduction: None,
    };

    let result = price_gbm(&ko_call, &sobol_gbm, &cfg).expect("pricing failed");

    // KO call should be cheaper than vanilla call
    // Rough bound: vanilla call ~ 10-12, KO should be < 10
    assert!(result.price < 15.0);
    assert!(result.price > 0.0);
}
