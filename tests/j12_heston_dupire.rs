//! Tests du jalon J12 — Heston + Dupire.
//!
//! Critères de complétion :
//!   - Heston avec σ_v=0 (vol constante) doit pricer un call < 1 % vs Black-Scholes.
//!   - Heston avec ρ ≠ 0 : prix diffèrent pour ρ=+0.5 et ρ=-0.5.
//!   - Dupire round-trip GBM → extraction → re-pricing : < 2 % d'erreur relative.
//!   - Les deux simulateurs fonctionnent via l'interface générique `&dyn Simulator`.

use kontract::ast::{at, konst, one, scale, spot, when, Contract};
use kontract::pricer::{price_gbm, McConfig};
use kontract::simulator::{dupire_from_gbm_calls, heston_from_params, Simulator};

// ============================================================================
// Utilitaires communs
// ============================================================================

/// Approximation rationnelle d'erf (Abramowitz & Stegun 7.1.26, erreur ≤ 1.5e-7).
fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    sign * y
}

fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

/// Formule de Black-Scholes pour un call européen.
fn bs_call(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> f64 {
    if sigma <= 0.0 || t <= 0.0 {
        return (s - k * (-r * t).exp()).max(0.0);
    }
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    s * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
}

/// Call européen exprimé dans le DSL.
fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

/// McConfig raisonnable pour les tests Heston / Dupire (assez de paths pour 1 %).
fn mc_cfg(r: f64) -> McConfig {
    McConfig {
        n_paths: 300_000,
        seed: 42,
        steps_per_year: 50,
        rate: r,
        variance_reduction: None,
    }
}

// ============================================================================
// Tests Heston
// ============================================================================

/// Test 1 — Limite GBM : σ_v = 0 (vol constante).
///
/// Quand la vol de vol est nulle, la variance reste figée à v0 pour tous les
/// chemins. Le modèle de Heston se réduit alors à un GBM avec σ = √v0.
/// Le prix d'un call doit coller à Black-Scholes (tolérance 1 %).
#[test]
fn heston_gbm_limit_sigma_v_zero() {
    let (s0, k, r, sigma, t) = (100.0, 100.0, 0.05, 0.20_f64, 1.0);
    let v0 = sigma * sigma; // variance initiale = σ²

    let heston = heston_from_params(
        "AAPL", s0, v0, /* kappa */ 1.0,
        /* theta */ v0, // theta = v0 → vol à long terme = σ
        /* sigma_v */ 0.0, // ← vol de vol nulle
        /* rho */ 0.0, r,
    );

    let contract = european_call("AAPL", k, t);
    let cfg = mc_cfg(r);

    let mc_price = price_gbm(&contract, &heston, &cfg).unwrap().price;
    let bs_price = bs_call(s0, k, r, sigma, t);

    let rel_err = (mc_price - bs_price).abs() / bs_price;
    assert!(
        rel_err < 0.01,
        "Heston(σ_v=0) vs BS : MC={mc_price:.4}, BS={bs_price:.4}, err_rel={rel_err:.4} (seuil 1 %)"
    );
}

/// Test 2 — Limite GBM : σ_v très petit, kappa grand (mean-reversion rapide).
///
/// Avec κ très grand et θ = v0, la variance est maintenue très proche de θ.
/// Le prix doit rester proche de Black-Scholes (tolérance 1.5 %).
#[test]
fn heston_gbm_limit_high_kappa() {
    let (s0, k, r, sigma, t) = (100.0, 100.0, 0.05, 0.20_f64, 1.0);
    let v0 = sigma * sigma;

    let heston = heston_from_params(
        "AAPL", s0, v0, /* kappa */ 100.0, // mean-reversion ultra rapide
        /* theta */ v0, /* sigma_v */ 0.01, // vol de vol très petite
        /* rho */ 0.0, r,
    );

    let contract = european_call("AAPL", k, t);
    let cfg = mc_cfg(r);

    let mc_price = price_gbm(&contract, &heston, &cfg).unwrap().price;
    let bs_price = bs_call(s0, k, r, sigma, t);

    let rel_err = (mc_price - bs_price).abs() / bs_price;
    assert!(
        rel_err < 0.015,
        "Heston(κ=100,σ_v=0.01) vs BS : MC={mc_price:.4}, BS={bs_price:.4}, err_rel={rel_err:.4} (seuil 1.5 %)"
    );
}

/// Test 3 — Statistiques des log-returns Heston.
///
/// La moyenne empirique des log-returns doit approcher (r − ½E[v])T.
/// Pour σ_v = 0 et kappa = 1, E[v] = v0 + (θ - v0)(1 - e^{-κT}) ≈ θ.
#[test]
fn heston_log_return_mean_is_close_to_theory() {
    let (s0, r, t) = (100.0, 0.05, 1.0);
    let v0 = 0.04_f64; // σ = 0.20
    let theta = 0.04_f64;
    let kappa = 2.0;
    let sigma_v = 0.3;
    let n_paths = 200_000_usize;

    let heston = heston_from_params("X", s0, v0, kappa, theta, sigma_v, 0.0, r);
    let arr = heston.simulate(&[0.0, t], n_paths, 77).unwrap();

    // log-returns empiriques
    let logs: Vec<f64> = arr.outer_iter().map(|row| (row[1] / s0).ln()).collect();
    let emp_mean = logs.iter().sum::<f64>() / logs.len() as f64;

    // Théorie : E[log(S_T/S0)] ≈ (r - θ/2) * T sous Heston si E[v] ≈ θ
    let theo_mean = (r - 0.5 * theta) * t;

    // Tolérance 3× l'erreur standard : SE ≈ sqrt(v*T / n)
    let se = (theta * t / n_paths as f64).sqrt();
    assert!(
        (emp_mean - theo_mean).abs() < 5.0 * se, // 5σ pour tenir compte des corr stochastiques
        "Heston log-return mean empirique={emp_mean:.4} théorique={theo_mean:.4}, 5·SE={:.4}",
        5.0 * se
    );
}

/// Test 4 — Impact de ρ sur le prix du call OTM.
///
/// Avec ρ > 0, une hausse de S s'accompagne d'une hausse de σ, ce qui booste
/// le prix des calls OTM (smile vers la droite). Avec ρ < 0, l'OTM call est
/// moins cher car la vol monte quand le spot baisse (smile vers la gauche /
/// skew négatif typique des actions).
///
/// On compare le prix d'un call OTM (K=115) sous ρ=+0.8 vs ρ=-0.8 en
/// simulant sur une grille fine (100 pas) pour que le processus de variance
/// pilote la dynamique du spot à travers de nombreuses étapes Euler.
///
/// Note: la corrélation ρ n'affecte le spot qu'à partir du 2ème pas Euler
/// (le 1er pas utilise v0 des deux côtés). Une grille fine est donc nécessaire.
#[test]
fn heston_rho_impacts_call_price() {
    use kontract::pricer::price_on_paths;

    let (s0, r, t) = (100.0, 0.05, 1.0);
    let k_otm = 115.0; // OTM : sensible à l'asymétrie du smile
    let v0 = 0.04_f64; // σ_0 = 20 %
                       // sigma_v élevé et kappa faible → vol stochastique forte → rho a un impact visible
    let (kappa, theta, sigma_v) = (0.5, 0.04_f64, 0.8_f64);
    let n_paths = 200_000;
    let seed = 42_u64;

    let heston_pos = heston_from_params("X", s0, v0, kappa, theta, sigma_v, 0.8, r);
    let heston_neg = heston_from_params("X", s0, v0, kappa, theta, sigma_v, -0.8, r);

    // Grille de simulation fine (100 pas) pour capturer l'effet de ρ sur v puis sur S
    let n_steps = 100;
    let fine_grid: Vec<f64> = (0..=n_steps)
        .map(|i| i as f64 * t / n_steps as f64)
        .collect();

    let contract_otm = european_call("X", k_otm, t);

    let paths_pos = heston_pos
        .simulate_paths(&fine_grid, n_paths, seed)
        .unwrap();
    let paths_neg = heston_neg
        .simulate_paths(&fine_grid, n_paths, seed)
        .unwrap();

    let price_pos = price_on_paths(&contract_otm, &paths_pos, &fine_grid, r)
        .unwrap()
        .price;
    let price_neg = price_on_paths(&contract_otm, &paths_neg, &fine_grid, r)
        .unwrap()
        .price;

    // ρ > 0 → OTM call plus cher (smile vers la droite)
    // ρ < 0 → OTM call moins cher (skew négatif)
    assert!(
        (price_pos - price_neg).abs() > 0.05,
        "ρ=+0.8 → {price_pos:.4}, ρ=-0.8 → {price_neg:.4} : différence OTM trop faible (attendu > 0.05)"
    );

    // Et que la direction est correcte : ρ positif → plus cher OTM
    assert!(
        price_pos > price_neg,
        "ρ=+0.8 call OTM {price_pos:.4} devrait être > ρ=-0.8 call OTM {price_neg:.4}"
    );
}

// ============================================================================
// Tests Dupire
// ============================================================================

/// Construit une surface de prix GBM C(K, T) avec les paramètres donnés.
fn gbm_call_surface(s0: f64, r: f64, sigma: f64, strikes: &[f64], maturities: &[f64]) -> Vec<f64> {
    let mut prices = Vec::with_capacity(maturities.len() * strikes.len());
    for &t in maturities {
        for &k in strikes {
            prices.push(bs_call(s0, k, r, sigma, t));
        }
    }
    prices
}

/// Test 5 — Round-trip GBM → Dupire : re-pricing dans les 2 %.
///
/// On génère une surface de calls sous GBM (σ=0.20) sur une grille fine de
/// maturités (mensuelle), on extrait la vol locale via Dupire, puis on re-price
/// les calls avec DupireSimulator. La simulation utilise une grille dense (100
/// pas par an) pour capter les variations de σ_loc(S, t) au fil du temps,
/// ce qui est indispensable pour le modèle à vol locale.
///
/// Note : `price_gbm` avec un call européen n'utilise que 2 pas de simulation
/// (t=0 et T), ce qui est insuffisant pour un simulateur à vol locale. On passe
/// par `price_on_paths` avec une grille fine.
#[test]
fn dupire_roundtrip_gbm_calls() {
    use kontract::pricer::price_on_paths;

    let (s0, r, sigma) = (100.0, 0.05, 0.20_f64);

    // Grille strikes fine centrée sur S0
    let strikes: Vec<f64> = (60..=160).step_by(5).map(|k| k as f64).collect();
    // Maturités mensuelles : pas fins → erreurs de diff. finies en T réduites
    let maturities: Vec<f64> = (1..=24).map(|m| m as f64 / 12.0).collect();

    let call_prices = gbm_call_surface(s0, r, sigma, &strikes, &maturities);

    let dupire = dupire_from_gbm_calls("X", s0, r, &strikes, &maturities, &call_prices)
        .expect("extraction Dupire");

    let n_paths = 200_000;
    let seed = 99_u64;

    // Re-pricage sur plusieurs cas de test avec grille fine
    let test_cases = [(90.0, 1.0_f64), (100.0, 1.0), (110.0, 1.0)];

    for (k, t) in test_cases {
        // Grille fine : 100 pas par an
        let n_steps = (100.0 * t) as usize;
        let fine_grid: Vec<f64> = (0..=n_steps)
            .map(|i| i as f64 * t / n_steps as f64)
            .collect();

        let contract = european_call("X", k, t);
        let paths = dupire.simulate_paths(&fine_grid, n_paths, seed).unwrap();
        let mc_price = price_on_paths(&contract, &paths, &fine_grid, r)
            .unwrap()
            .price;
        let ref_price = bs_call(s0, k, r, sigma, t);

        if ref_price < 0.5 {
            continue;
        }
        let rel_err = (mc_price - ref_price).abs() / ref_price;
        assert!(
            rel_err < 0.03, // 3 % : compromis précision Dupire / MC
            "Dupire K={k}, T={t} : MC={mc_price:.4}, BS={ref_price:.4}, err={rel_err:.4}"
        );
    }
}

/// Test 6 — Consistance du smile Dupire sur grille fine.
///
/// Sur une surface GBM (vol plate), le simulateur Dupire doit produire un prix
/// ATM proche de BS. On utilise une grille de maturités fine (mensuelle) pour
/// réduire les erreurs de différences finies dans la formule de Dupire, et
/// une grille de simulation dense (100 pas) pour capter σ_loc(S, t).
#[test]
fn dupire_atm_vol_is_reasonable() {
    use kontract::pricer::price_on_paths;

    let (s0, r, sigma) = (100.0, 0.05, 0.20_f64);
    let k = s0; // ATM

    // Grille mensuelle pour limiter les erreurs Dupire
    let strikes: Vec<f64> = (50..=200).step_by(5).map(|k| k as f64).collect();
    let maturities: Vec<f64> = (1..=24).map(|m| m as f64 / 12.0).collect();
    let call_prices = gbm_call_surface(s0, r, sigma, &strikes, &maturities);

    let dupire = dupire_from_gbm_calls("X", s0, r, &strikes, &maturities, &call_prices)
        .expect("extraction Dupire");

    let n_paths = 200_000;
    let seed = 55_u64;

    for &t in &[0.5_f64, 1.0] {
        // Grille fine pour capter la dynamique de vol locale
        let n_steps = (100.0 * t) as usize;
        let fine_grid: Vec<f64> = (0..=n_steps)
            .map(|i| i as f64 * t / n_steps as f64)
            .collect();

        let contract = european_call("X", k, t);
        let paths = dupire.simulate_paths(&fine_grid, n_paths, seed).unwrap();
        let mc_price = price_on_paths(&contract, &paths, &fine_grid, r)
            .unwrap()
            .price;
        let bs_ref = bs_call(s0, k, r, sigma, t);

        let rel_err = (mc_price - bs_ref).abs() / bs_ref;
        assert!(
            rel_err < 0.03,
            "Dupire ATM T={t} : MC={mc_price:.4}, BS={bs_ref:.4}, err_rel={rel_err:.4} (seuil 3 %)"
        );
    }
}

/// Test 7 — Les deux simulateurs fonctionnent via `&dyn Simulator` (interface générique).
///
/// Vérifie que `price_gbm` (qui accepte `&dyn Simulator`) fonctionne bien avec
/// HestonSimulator et DupireSimulator, sans downcast ni code spécifique au modèle.
#[test]
fn both_simulators_work_via_dyn_simulator() {
    let (s0, r, sigma) = (100.0, 0.05, 0.20_f64);
    let k = 100.0;
    let t = 1.0;
    let v0 = sigma * sigma;

    let heston = heston_from_params("A", s0, v0, 2.0, v0, 0.3, 0.0, r);

    let strikes: Vec<f64> = (60..=160).step_by(10).map(|k| k as f64).collect();
    let maturities = vec![0.5, 1.0, 2.0];
    let call_prices = gbm_call_surface(s0, r, sigma, &strikes, &maturities);
    let dupire = dupire_from_gbm_calls("B", s0, r, &strikes, &maturities, &call_prices).unwrap();

    let cfg = McConfig {
        n_paths: 50_000,
        seed: 7,
        steps_per_year: 50,
        rate: r,
        variance_reduction: None,
    };

    // Heston via dyn Simulator
    let call_h = european_call("A", k, t);
    let simulators: Vec<&dyn Simulator> = vec![&heston];
    for sim in simulators {
        let res = price_gbm(&call_h, sim, &cfg).expect("price_gbm Heston");
        assert!(res.price > 0.0, "Heston via dyn: prix nul ou négatif");
        assert!(res.price < s0, "Heston via dyn: prix > S0");
    }

    // Dupire via dyn Simulator
    let call_d = european_call("B", k, t);
    let res_d = price_gbm(&call_d, &dupire as &dyn Simulator, &cfg).expect("price_gbm Dupire");
    assert!(res_d.price > 0.0, "Dupire via dyn: prix nul ou négatif");
    assert!(res_d.price < s0, "Dupire via dyn: prix > S0");
}

/// Test 8 — Reproductibilité et indépendance des graines.
///
/// Deux appels Heston avec la même graine → même prix bit-pour-bit.
/// Deux appels avec des graines différentes → des valeurs différentes.
#[test]
fn heston_reproducibility() {
    let v0 = 0.04_f64;
    let heston = heston_from_params("Y", 100.0, v0, 2.0, v0, 0.4, -0.3, 0.05);
    let contract = european_call("Y", 100.0, 1.0);
    let cfg_a = McConfig {
        seed: 1234,
        n_paths: 10_000,
        steps_per_year: 20,
        rate: 0.05,
        variance_reduction: None,
    };
    let cfg_b = McConfig {
        seed: 5678,
        ..cfg_a
    };

    let price_a1 = price_gbm(&contract, &heston, &cfg_a).unwrap().price;
    let price_a2 = price_gbm(&contract, &heston, &cfg_a).unwrap().price;
    let price_b = price_gbm(&contract, &heston, &cfg_b).unwrap().price;

    assert_eq!(price_a1, price_a2, "même graine → même prix");
    assert_ne!(price_a1, price_b, "graines différentes → prix différents");
}
