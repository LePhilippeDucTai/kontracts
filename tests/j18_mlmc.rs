//! Jalon J18 — Multilevel Monte-Carlo (Giles 2008).
//!
//! Validation :
//!   - décomposition de variance `V_ℓ = Var(P_ℓ − P_{ℓ−1})` décroissante (Heston) ;
//!   - allocation optimale `N_ℓ ∝ √(V_ℓ/C_ℓ)` ;
//!   - convergence du coût en O(ε⁻²) (et non O(ε⁻²·⁵)) ;
//!   - prix MLMC ≈ MC standard (call EU sous GBM / Heston) ;
//!   - économies de coût mesurables vs MC standard à tolérance égale.
//!
//! Le simulateur GBM utilise un schéma log-normal **exact** : il n'a aucun biais
//! de discrétisation, donc `V_ℓ ≈ 0` pour ℓ≥1 (les niveaux fins/grossiers sont
//! identiques). Les tests de décroissance de variance utilisent donc **Heston**
//! (schéma d'Euler), le « cas demandant » du jalon.

use kontract::ast::{at, konst, one, scale, spot, when};
use kontract::{
    optimal_allocation, price_gbm, price_mlmc, price_mlmc_detailed, Contract, Gbm, HestonSimulator,
    McConfig, MlmcConfig,
};

// ── Références analytiques ───────────────────────────────────────────────────

fn norm_cdf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let a = x.abs() / std::f64::consts::SQRT_2;
    let t = 1.0 / (1.0 + 0.327_591_1 * a);
    let poly = ((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736)
        * t
        + 0.254_829_592;
    0.5 * (1.0 + sign * (1.0 - poly * t * (-a * a).exp()))
}

fn bs_call(s: f64, k: f64, t: f64, r: f64, sigma: f64) -> f64 {
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    let d2 = d1 - sigma * t.sqrt();
    s * norm_cdf(d1) - k * (-r * t).exp() * norm_cdf(d2)
}

/// Call européen `max(S_T − K, 0)` payé en T.
fn eu_call(k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot("S") - konst(k)).max(konst(0.0)), one("USD")),
    )
}

const S0: f64 = 100.0;
const K: f64 = 100.0;
const T: f64 = 1.0;
const R: f64 = 0.05;
const SIGMA: f64 = 0.2;

// ============================================================================
// Test 1 : prix MLMC ≈ Black-Scholes sur un call EU (GBM)
// ============================================================================

#[test]
fn mlmc_call_matches_black_scholes_gbm() {
    let call = eu_call(K, T);
    let gbm = Gbm::new("S", S0, R, SIGMA);

    let cfg = McConfig {
        n_paths: 0, // ignoré par MLMC
        seed: 7,
        steps_per_year: 0,
        rate: R,
        variance_reduction: None,
    };
    let mlmc_cfg = MlmcConfig {
        n_levels: 6,
        pilot_paths: 2_000,
        target_variance: 0.02 * 0.02, // ε = 0.02 (borne d'erreur statistique)
        cost_per_step: 1,
    };

    let res = price_mlmc(&call, &gbm, &cfg, &mlmc_cfg).unwrap();
    let bs = bs_call(S0, K, T, R, SIGMA);

    // GBM exact → MLMC sans biais : prix très proche de BS.
    assert!(
        (res.price - bs).abs() < 0.06,
        "MLMC={} vs BS={}",
        res.price,
        bs
    );
    // L'IC 95 % doit contenir le prix BS.
    assert!(
        res.ci95_low <= bs && bs <= res.ci95_high,
        "BS {bs} hors IC [{}, {}]",
        res.ci95_low,
        res.ci95_high
    );
}

// ============================================================================
// Test 2 : décomposition de variance V_ℓ décroissante (Heston / Euler)
// ============================================================================

#[test]
fn variance_decomposition_decays_with_level() {
    let call = eu_call(K, T);
    // Heston (schéma d'Euler) : biais de discrétisation → V_ℓ décroît avec ℓ.
    let heston = HestonSimulator::new("S", S0, 0.04, 1.5, 0.04, 0.3, -0.7, R);

    let cfg = McConfig {
        n_paths: 0,
        seed: 11,
        steps_per_year: 0,
        rate: R,
        variance_reduction: None,
    };
    let mlmc_cfg = MlmcConfig {
        n_levels: 5,
        pilot_paths: 8_000,
        target_variance: 0.05 * 0.05,
        cost_per_step: 1,
    };

    let det = price_mlmc_detailed(&call, &heston, &cfg, &mlmc_cfg).unwrap();
    let v = &det.level_variances;

    // V_0 (niveau grossier seul) >> V_ℓ pour ℓ≥1 (effet du couplage MLMC).
    assert!(v[1] < 0.5 * v[0], "V_1={} pas << V_0={}", v[1], v[0]);

    // Tendance globale décroissante : V_5 nettement < V_1.
    assert!(
        v[5] < 0.5 * v[1],
        "décroissance insuffisante : V_1={}, V_5={}",
        v[1],
        v[5]
    );

    // Décroissance approx. géométrique : la moyenne du ratio V_{ℓ+1}/V_ℓ < 1.
    let ratios: Vec<f64> = (1..5).map(|l| v[l + 1] / v[l]).collect();
    let mean_ratio = ratios.iter().sum::<f64>() / ratios.len() as f64;
    assert!(
        mean_ratio < 0.95,
        "pas de décroissance géométrique (ratio moyen {mean_ratio})"
    );
}

// ============================================================================
// Test 3 : convergence du coût en O(ε⁻²)
// ============================================================================

#[test]
fn cost_scales_as_epsilon_minus_two() {
    let call = eu_call(K, T);
    let heston = HestonSimulator::new("S", S0, 0.04, 1.5, 0.04, 0.3, -0.7, R);

    let cfg = McConfig {
        n_paths: 0,
        seed: 11,
        steps_per_year: 0,
        rate: R,
        variance_reduction: None,
    };

    let run = |tol: f64| -> f64 {
        let mlmc_cfg = MlmcConfig {
            n_levels: 5,
            pilot_paths: 1_000,
            target_variance: tol * tol,
            cost_per_step: 1,
        };
        price_mlmc_detailed(&call, &heston, &cfg, &mlmc_cfg)
            .unwrap()
            .total_cost as f64
    };

    let tol_a = 0.08;
    let tol_b = 0.04; // ε divisé par 2 → coût ×4 si O(ε⁻²)
    let cost_a = run(tol_a);
    let cost_b = run(tol_b);

    // O(ε⁻²) : diviser ε par 2 multiplie le coût par ≈4. On tolère [2.5, 6].
    let ratio = cost_b / cost_a;
    assert!(
        (2.5..=6.0).contains(&ratio),
        "scaling du coût hors O(ε⁻²) : ratio={ratio} (attendu ≈4)"
    );
}

// ============================================================================
// Test 4 : prix MLMC ≈ MC standard sous Heston (< 2 %)
// ============================================================================

#[test]
fn mlmc_heston_matches_single_level_mc() {
    let call = eu_call(K, T);
    let heston = HestonSimulator::new("S", S0, 0.04, 1.5, 0.04, 0.3, -0.7, R);

    // MLMC.
    let cfg = McConfig {
        n_paths: 0,
        seed: 11,
        steps_per_year: 0,
        rate: R,
        variance_reduction: None,
    };
    let mlmc_cfg = MlmcConfig {
        n_levels: 6,
        pilot_paths: 2_000,
        target_variance: 0.03 * 0.03,
        cost_per_step: 1,
    };
    let mlmc = price_mlmc(&call, &heston, &cfg, &mlmc_cfg).unwrap();

    // MC standard de référence : grille fine (64 pas/an), beaucoup de paths.
    let cfg_std = McConfig {
        n_paths: 400_000,
        seed: 99,
        steps_per_year: 64,
        rate: R,
        variance_reduction: None,
    };
    let std = price_gbm(&call, &heston, &cfg_std).unwrap();

    let rel = (mlmc.price - std.price).abs() / std.price;
    assert!(
        rel < 0.02,
        "MLMC={} vs MC std={} (écart relatif {:.3}%)",
        mlmc.price,
        std.price,
        rel * 100.0
    );
}

// ============================================================================
// Test 5 : allocation optimale N_ℓ ∝ √(V_ℓ/C_ℓ) + économies vs MC standard
// ============================================================================

#[test]
fn optimal_allocation_proportional_to_sqrt_variance() {
    // Variances décroissantes typiques (Euler), coûts ∝ 2^ℓ.
    let variances = [200.0, 50.0, 12.5, 3.125, 0.781];
    let costs = [1usize, 3, 6, 12, 24];
    let target_var = 0.01 * 0.01;

    let alloc = optimal_allocation(&variances, &costs, target_var);

    // Tous les niveaux reçoivent ≥ 1 trajectoire.
    assert!(alloc.iter().all(|&n| n >= 1));

    // N_ℓ doit décroître avec le niveau (V_ℓ et 1/√C_ℓ décroissent tous deux).
    for l in 0..alloc.len() - 1 {
        assert!(
            alloc[l] >= alloc[l + 1],
            "N_{} ({}) < N_{} ({}) : allocation non décroissante",
            l,
            alloc[l],
            l + 1,
            alloc[l + 1]
        );
    }

    // Vérification de la proportionnalité N_ℓ ∝ √(V_ℓ/C_ℓ) : le ratio
    // N_ℓ / √(V_ℓ/C_ℓ) doit être ~constant entre niveaux (à l'arrondi près).
    let key = |l: usize| (variances[l] / costs[l] as f64).sqrt();
    let c0 = alloc[0] as f64 / key(0);
    for (l, &n_l) in alloc.iter().enumerate().skip(1) {
        let cl = n_l as f64 / key(l);
        let rel = (cl - c0).abs() / c0;
        assert!(
            rel < 0.05 || n_l <= 2,
            "proportionnalité brisée au niveau {l} : {cl} vs {c0}"
        );
    }
}

// ============================================================================
// Test 6 : économies de coût mesurables vs MC standard (critère du jalon)
// ============================================================================

#[test]
fn mlmc_cheaper_than_standard_mc_same_tolerance() {
    let call = eu_call(K, T);
    let heston = HestonSimulator::new("S", S0, 0.04, 1.5, 0.04, 0.3, -0.7, R);

    let cfg = McConfig {
        n_paths: 0,
        seed: 11,
        steps_per_year: 0,
        rate: R,
        variance_reduction: None,
    };
    // Grille fine profonde (L=8 → 256 pas) : c'est là que MLMC bat le MC plat,
    // dont le coût croît linéairement avec la résolution (2^L pas / trajectoire).
    let tol = 0.06;
    let mlmc_cfg = MlmcConfig {
        n_levels: 8,
        pilot_paths: 800,
        target_variance: tol * tol,
        cost_per_step: 1,
    };
    let det = price_mlmc_detailed(&call, &heston, &cfg, &mlmc_cfg).unwrap();
    let mlmc_cost = det.total_cost as f64;

    // Coût d'un MC standard atteignant la même erreur standard à la grille la
    // plus fine (2^n_levels pas) : N = (σ/tol)², chaque path coûtant 2^L pas.
    let cfg_std = McConfig {
        n_paths: 30_000,
        seed: 99,
        steps_per_year: 1 << mlmc_cfg.n_levels,
        rate: R,
        variance_reduction: None,
    };
    let std = price_gbm(&call, &heston, &cfg_std).unwrap();
    let steps_fine = (1usize << mlmc_cfg.n_levels) as f64;
    let n_needed = (std.sample_std / tol).powi(2);
    let std_cost = n_needed * steps_fine;

    // Critère du jalon : économies mesurables (facteur > 2×).
    assert!(
        mlmc_cost < std_cost / 2.0,
        "pas d'économie : MLMC {mlmc_cost:.3e} vs MC std {std_cost:.3e}"
    );
}
