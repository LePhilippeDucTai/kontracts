//! Tests du jalon J15 — Réduction de variance (antithétiques + contrôle).
//!
//! Critères de complétion :
//!   VR-1 : Variables antithétiques — `sample_std_anti < 0.75 × sample_std_baseline`
//!           sur un call européen ATM.
//!   VR-2 : Les antithétiques préservent le prix (± 1 % du prix BS).
//!   VR-3 : Variable de contrôle — variance réduite sur call à barrière KO.
//!   VR-4 : La variable de contrôle est sans biais (prix ± 1 % du MC standard).
//!
//! Ces tests n'utilisent pas `price_gbm` directement pour VR-3/4 (barrier KO) afin
//! de vérifier la mécanique indépendamment du pricer de haut niveau.

use kontract::ast::{at, konst, one, scale, spot, until, when, Contract};
use kontract::pricer::{price_gbm, price_on_paths, McConfig};
use kontract::simulator::Gbm;
use kontract::variance_reduction::{
    apply_control_variate, black_scholes_call, price_antithetic_on_paths,
    price_control_variate_on_paths, simulate_antithetic_gbm, VarianceReductionConfig,
};

// ── Utilitaires ──────────────────────────────────────────────────────────────

/// Call européen vanille via le DSL.
fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

/// Call knock-out (barrière supérieure) via le DSL.
/// Le contrat est annulé dès que `spot >= barrier`.
fn up_and_out_call(asset: &str, k: f64, barrier: f64, t: f64) -> Contract {
    let call = when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    );
    // `until(cond, c)` : knock-out quand `spot >= barrier`.
    until(spot(asset).ge(konst(barrier)), call)
}

/// Black-Scholes standard (call).
fn bs_call(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    black_scholes_call(s, k, t, r, sigma)
}

/// Grille uniforme `[0, t]` avec `n_steps` intervalles (n_steps+1 points).
fn uniform_grid(t: f64, n_steps: usize) -> Vec<f64> {
    (0..=n_steps)
        .map(|i| i as f64 * t / n_steps as f64)
        .collect()
}

// ============================================================================
// VR-1 : Antithétiques → variance réduite (σ_anti < 0.75 × σ_baseline)
// ============================================================================

/// Sur un call européen ATM, les variables antithétiques doivent réduire l'écart-type
/// empirique d'au moins 25 % par rapport à la simulation directe de même taille
/// *effective* (même nombre de tirages aléatoires : n_half paires vs n_half paths).
#[test]
fn vr1_antithetic_reduces_variance_eu_call() {
    let (s0, k, t, r, sigma) = (100.0, 100.0, 1.0, 0.05, 0.20);
    let asset = "S";
    let gbm = Gbm::new(asset, s0, r, sigma);
    let contract = european_call(asset, k, t);

    let n_half = 20_000usize; // paires antithétiques
    let seed = 42u64;

    // -- Simulation baseline : n_half trajectoires directes --
    let grid = uniform_grid(t, 50);
    let paths_baseline = gbm
        .simulate_paths(&grid, n_half, seed)
        .expect("simulate_paths baseline");
    let res_baseline =
        price_on_paths(&contract, &paths_baseline, &grid, r).expect("price baseline");

    // -- Simulation antithétique : n_half paires --
    let (bases, antis) = simulate_antithetic_gbm(asset, s0, r, sigma, &grid, n_half, seed)
        .expect("simulate_antithetic_gbm");
    let res_anti =
        price_antithetic_on_paths(&contract, &bases, &antis, &grid, r).expect("price antithetic");

    assert!(
        res_anti.sample_std < 0.75 * res_baseline.sample_std,
        "VR-1 FAIL : std_anti={:.6} doit être < 0.75 × std_baseline={:.6} (ratio={:.3})",
        res_anti.sample_std,
        res_baseline.sample_std,
        res_anti.sample_std / res_baseline.sample_std
    );
}

// ============================================================================
// VR-2 : Antithétiques → prix sans biais (± 1 % du prix BS)
// ============================================================================

/// Le prix antithétique doit coïncider avec la formule de Black-Scholes à 1 % près.
#[test]
fn vr2_antithetic_price_matches_bs() {
    let (s0, k, t, r, sigma) = (100.0, 100.0, 1.0, 0.05, 0.20);
    let asset = "S";
    let gbm = Gbm::new(asset, s0, r, sigma);
    let contract = european_call(asset, k, t);

    let n_half = 50_000usize;
    let seed = 99u64;
    let grid = uniform_grid(t, 50);

    let (bases, antis) = simulate_antithetic_gbm(asset, s0, r, sigma, &grid, n_half, seed)
        .expect("simulate_antithetic_gbm");
    let res =
        price_antithetic_on_paths(&contract, &bases, &antis, &grid, r).expect("price antithetic");

    let bs = bs_call(s0, k, t, r, sigma);
    let rel_err = (res.price - bs).abs() / bs;
    assert!(
        rel_err < 0.01,
        "VR-2 FAIL : prix_anti={:.4} vs BS={:.4} (err_rel={:.4}, seuil 1 %)",
        res.price,
        bs,
        rel_err
    );

    // Vérification via `price_gbm` avec config VR intégrée.
    let cfg = McConfig {
        n_paths: n_half * 2,
        seed,
        steps_per_year: 50,
        rate: r,
        variance_reduction: Some(VarianceReductionConfig {
            use_antithetic: true,
            use_control_variate: false,
        }),
    };
    let res_gbm = price_gbm(&contract, &gbm, &cfg).expect("price_gbm with antithetic");
    let rel_err_gbm = (res_gbm.price - bs).abs() / bs;
    assert!(
        rel_err_gbm < 0.01,
        "VR-2b FAIL (price_gbm) : prix={:.4} vs BS={:.4} (err_rel={:.4})",
        res_gbm.price,
        bs,
        rel_err_gbm
    );
}

// ============================================================================
// VR-3 : Variable de contrôle → variance réduite sur call KO
// ============================================================================

/// Calcule le coefficient β optimal : `Cov(Y, X) / Var(X)`.
/// Avec ce β, la variance de l'estimateur corrigé est toujours ≤ à la variance brute.
fn optimal_beta(pvs_target: &[f64], pvs_ctrl: &[f64]) -> f64 {
    let n = pvs_target.len() as f64;
    let mean_y = pvs_target.iter().sum::<f64>() / n;
    let mean_x = pvs_ctrl.iter().sum::<f64>() / n;
    let cov: f64 = pvs_target
        .iter()
        .zip(pvs_ctrl.iter())
        .map(|(y, x)| (y - mean_y) * (x - mean_x))
        .sum::<f64>()
        / (n - 1.0);
    let var_x: f64 = pvs_ctrl.iter().map(|x| (x - mean_x).powi(2)).sum::<f64>() / (n - 1.0);
    if var_x < 1e-12 {
        1.0
    } else {
        cov / var_x
    }
}

/// Sur un call knock-out, la variable de contrôle (call européen vanille de même
/// strike et maturité) doit réduire la variance quand on utilise le β optimal.
/// Le β optimal (`Cov(Y,X)/Var(X)`) garantit que la variance corrigée ≤ variance brute.
#[test]
fn vr3_control_variate_reduces_variance_ko_call() {
    use kontract::pricer::present_value_pub;

    let (s0, k, barrier, t, r, sigma) = (100.0, 100.0, 130.0, 1.0, 0.05, 0.20);
    let asset = "S";
    let gbm = Gbm::new(asset, s0, r, sigma);

    // Variable de contrôle : call vanille de même strike (corrélation positive avec KO).
    let ctrl_call = european_call(asset, k, t); // même strike k=100

    // Call KO (cible).
    let ko_call = up_and_out_call(asset, k, barrier, t);

    let n_paths = 60_000usize;
    let seed = 77u64;

    // Grille fine pour le monitoring de la barrière.
    let grid = uniform_grid(t, 100);
    let paths = gbm
        .simulate_paths(&grid, n_paths, seed)
        .expect("simulate_paths");

    // Prix BS du call de contrôle.
    let bs_ctrl = bs_call(s0, k, t, r, sigma);

    // PVs individuels par trajectoire (pour calculer β optimal).
    let pvs_target: Vec<f64> = paths
        .iter()
        .map(|p| present_value_pub(&ko_call, p, &grid, r).unwrap())
        .collect();
    let pvs_ctrl: Vec<f64> = paths
        .iter()
        .map(|p| present_value_pub(&ctrl_call, p, &grid, r).unwrap())
        .collect();

    let beta = optimal_beta(&pvs_target, &pvs_ctrl);

    // -- Résultat MC brut (sans VdC) --
    let res_raw = price_on_paths(&ko_call, &paths, &grid, r).expect("price raw");

    // -- Résultat MC avec variable de contrôle (β optimal) --
    let res_cv =
        price_control_variate_on_paths(&ko_call, &ctrl_call, bs_ctrl, beta, &paths, &grid, r)
            .expect("price control variate");

    assert!(
        res_cv.sample_std < res_raw.sample_std,
        "VR-3 FAIL : std_cv={:.6} doit être < std_raw={:.6} (β={:.4})",
        res_cv.sample_std,
        res_raw.sample_std,
        beta
    );
}

// ============================================================================
// VR-4 : Variable de contrôle → sans biais (prix ± 1 % du MC standard)
// ============================================================================

/// L'estimateur par variable de contrôle est sans biais : son espérance doit
/// correspondre au prix MC brut (calculé sur un grand échantillon), à 1 % près.
#[test]
fn vr4_control_variate_unbiased() {
    let (s0, k, t, r, sigma) = (100.0, 100.0, 1.0, 0.05, 0.20);
    let asset = "S";
    let gbm = Gbm::new(asset, s0, r, sigma);

    let ctrl_call = european_call(asset, s0, t); // ATM
    let target = european_call(asset, k, t); // même call ATM → doit donner ~BS

    let n_paths = 100_000usize;
    let seed = 2024u64;
    let grid = uniform_grid(t, 50);
    let paths = gbm
        .simulate_paths(&grid, n_paths, seed)
        .expect("simulate_paths");

    let bs_ctrl = bs_call(s0, s0, t, r, sigma);

    // Prix MC brut (référence).
    let res_raw = price_on_paths(&target, &paths, &grid, r).expect("price raw");

    // Prix avec variable de contrôle.
    let res_cv =
        price_control_variate_on_paths(&target, &ctrl_call, bs_ctrl, 1.0, &paths, &grid, r)
            .expect("price control variate");

    // Le prix corrigé doit coller à la formule BS (sans biais).
    let bs = bs_call(s0, k, t, r, sigma);
    let rel_err_cv = (res_cv.price - bs).abs() / bs;
    let rel_err_raw = (res_raw.price - bs).abs() / bs;

    assert!(
        rel_err_cv < 0.01,
        "VR-4 FAIL : prix_cv={:.4} vs BS={:.4} (err_rel_cv={:.4}, err_rel_raw={:.4})",
        res_cv.price,
        bs,
        rel_err_cv,
        rel_err_raw
    );

    // Vérifier aussi via `price_gbm` avec config VR intégrée.
    let cfg = McConfig {
        n_paths,
        seed,
        steps_per_year: 50,
        rate: r,
        variance_reduction: Some(VarianceReductionConfig {
            use_antithetic: false,
            use_control_variate: true,
        }),
    };
    let res_gbm = price_gbm(&target, &gbm, &cfg).expect("price_gbm with cv");
    let rel_err_gbm = (res_gbm.price - bs).abs() / bs;
    assert!(
        rel_err_gbm < 0.01,
        "VR-4b FAIL (price_gbm) : prix={:.4} vs BS={:.4} (err_rel={:.4})",
        res_gbm.price,
        bs,
        rel_err_gbm
    );

    // Vérification de l'identité `apply_control_variate` (formule pure).
    let corrected = apply_control_variate(res_raw.price, res_raw.price, bs_ctrl, 1.0);
    let expected = res_raw.price - (res_raw.price - bs_ctrl);
    let diff = (corrected - expected).abs();
    assert!(
        diff < 1e-10,
        "apply_control_variate : {corrected:.10} != {expected:.10} (diff={diff:.2e})"
    );
}
