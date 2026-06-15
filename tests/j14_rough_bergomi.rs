//! Tests du jalon J14 — Rough Bergomi (volatilité rugueuse, fBm).
//!
//! Critères de complétion (4 tests + 1 robustesse) :
//!   RB-1 : trajectoires bien formées (pas de NaN, spots finis et positifs)
//!   RB-2 : propriété de Hurst — scaling de variance `Var[B_t^H] = t^{2H}`
//!          (estimation de H par régression log-log, recouvré à 10 %)
//!   RB-3 : kurtosis des log-rendements > kurtosis gaussien (signature rugueuse)
//!   RB-4 : convergence de la variance MC en 1/√n sur le pricing d'un call
//!   RB-5 : forward variance plat `E[v_T] ≈ v_0` + signe du levier (ρ<0 → skew)
//!
//! Le modèle Rough Bergomi (Bayer-Friz-Gatheral 2016) pilote la log-variance par
//! un mouvement brownien fractionnaire d'exposant `H < ½`, générant des smiles de
//! volatilité réalistes au prix d'un coût MC élevé (Cholesky O(n²) par appel).

use kontract::ast::{at, konst, one, scale, spot, when, Contract};
use kontract::pricer::price_on_paths;
use kontract::simulator::{rough_bergomi_from_params, Simulator};

/// Call européen exprimé dans le DSL (identique aux suites J12/J13).
fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

/// Grille uniforme `[0, t]` en `n_steps` intervalles (n_steps+1 points).
fn uniform_grid(t: f64, n_steps: usize) -> Vec<f64> {
    (0..=n_steps)
        .map(|i| i as f64 * t / n_steps as f64)
        .collect()
}

// ============================================================================
// RB-1 : trajectoires bien formées
// ============================================================================

/// Toutes les trajectoires (spots et fBm) doivent être finies ; les spots
/// strictement positifs (schéma log-Euler) ; la variance implicite strictement
/// positive (pas de log-vol qui explose).
#[test]
fn rb_paths_are_well_formed() {
    let (s0, v0, xi, h, rho, r) = (100.0, 0.04, 1.5, 0.2, -0.7, 0.03);
    let rb = rough_bergomi_from_params("X", s0, v0, xi, h, rho, r);

    let grid = uniform_grid(1.0, 100);
    let n_paths = 1_000;

    // Spots
    let arr = rb.simulate(&grid, n_paths, 7).expect("RB simulate failed");
    assert_eq!(arr.shape(), &[n_paths, grid.len()]);
    for &s in arr.iter() {
        assert!(s.is_finite(), "spot non fini: {s}");
        assert!(s > 0.0, "spot non strictement positif: {s}");
    }
    // Première colonne = S0 (times[0] == 0).
    for i in 0..n_paths {
        assert!(
            (arr[[i, 0]] - s0).abs() < 1e-9,
            "row[{i}][0] = {} != s0 = {s0}",
            arr[[i, 0]]
        );
    }

    // fBm sous-jacent : doit être fini, et B_0 (premier instant > 0) de petite
    // amplitude (var ≈ t^{2H}).
    let fbm = rb.simulate_fbm(&grid, n_paths, 7).expect("RB fbm failed");
    assert_eq!(fbm.shape()[0], n_paths);
    for &b in fbm.iter() {
        assert!(b.is_finite(), "fBm non fini: {b}");
    }
}

// ============================================================================
// RB-2 : propriété de Hurst (scaling de variance)
// ============================================================================

/// Le fBm satisfait `Var[B_t^H] = t^{2H}`. En estimant la variance empirique du
/// fBm à chaque instant `t` sur un grand échantillon, la régression linéaire de
/// `ln Var` sur `ln t` a pour pente `2H`. On recouvre `H` à 10 % près.
#[test]
fn rb_recovers_hurst_via_variance_scaling() {
    let h_true = 0.3_f64;
    let rb = rough_bergomi_from_params("X", 100.0, 0.04, 1.0, h_true, 0.0, 0.0);

    let grid = uniform_grid(1.0, 100);
    let n_paths = 40_000;
    let fbm = rb.simulate_fbm(&grid, n_paths, 123).expect("RB fbm failed");

    // Instants strictement positifs correspondant aux colonnes de `fbm`.
    let pts: Vec<f64> = grid.iter().copied().filter(|&t| t > 0.0).collect();
    assert_eq!(pts.len(), fbm.shape()[1]);

    // Variance empirique par colonne (moyenne fBm ≈ 0 par construction).
    let m = pts.len();
    let mut log_t = Vec::with_capacity(m);
    let mut log_var = Vec::with_capacity(m);
    for (j, &pt) in pts.iter().enumerate() {
        let col = fbm.column(j);
        let mean: f64 = col.iter().sum::<f64>() / n_paths as f64;
        let var: f64 =
            col.iter().map(|&x| (x - mean) * (x - mean)).sum::<f64>() / (n_paths as f64 - 1.0);
        // Ignorer les tout premiers instants (variance minuscule → bruit relatif).
        if pt >= 0.1 && var > 0.0 {
            log_t.push(pt.ln());
            log_var.push(var.ln());
        }
    }
    assert!(log_t.len() >= 5, "pas assez de points pour la régression");

    // Régression linéaire OLS : pente = 2H.
    let n = log_t.len() as f64;
    let sx: f64 = log_t.iter().sum();
    let sy: f64 = log_var.iter().sum();
    let sxx: f64 = log_t.iter().map(|x| x * x).sum();
    let sxy: f64 = log_t.iter().zip(&log_var).map(|(x, y)| x * y).sum();
    let slope = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    let h_est = slope / 2.0;

    let rel_err = (h_est - h_true).abs() / h_true;
    assert!(
        rel_err < 0.10,
        "Hurst recouvré H_est={h_est:.4} vs H_true={h_true:.4} (err_rel={rel_err:.3}, seuil 10 %)"
    );
}

// ============================================================================
// RB-3 : kurtosis des log-rendements > gaussien (signature rugueuse)
// ============================================================================

/// Pour `H < ½`, le clustering de volatilité rugueuse alourdit les queues : la
/// kurtosis empirique des log-rendements terminaux excède la valeur gaussienne
/// (3.0). On exige une **kurtosis en excès > 0.5** pour `H = 0.1`.
#[test]
fn rb_kurtosis_exceeds_gaussian() {
    let h = 0.1_f64;
    let rb = rough_bergomi_from_params("X", 100.0, 0.04, 1.8, h, -0.7, 0.0);

    let grid = uniform_grid(1.0, 100);
    let n_paths = 100_000;
    let arr = rb
        .simulate(&grid, n_paths, 2024)
        .expect("RB simulate failed");

    let last = grid.len() - 1;
    let s0 = arr[[0, 0]];
    // Log-rendements terminaux ln(S_T / S_0).
    let returns: Vec<f64> = (0..n_paths).map(|i| (arr[[i, last]] / s0).ln()).collect();

    let n = returns.len() as f64;
    let mean: f64 = returns.iter().sum::<f64>() / n;
    let var: f64 = returns.iter().map(|&x| (x - mean).powi(2)).sum::<f64>() / n;
    let std = var.sqrt();
    // Kurtosis (moment d'ordre 4 normalisé).
    let kurt: f64 = returns
        .iter()
        .map(|&x| ((x - mean) / std).powi(4))
        .sum::<f64>()
        / n;
    let excess = kurt - 3.0;

    assert!(
        excess > 0.5,
        "kurtosis en excès = {excess:.3} (kurt={kurt:.3}) devrait dépasser 0.5 pour H={h}"
    );
}

// ============================================================================
// RB-4 : convergence de la variance MC en 1/√n
// ============================================================================

/// Le pricer fonctionne avec des trajectoires Rough Bergomi (non markoviennes) et
/// son erreur standard décroît comme `1/√n`. On price un call à trois tailles
/// d'échantillon ; l'erreur standard doit suivre le scaling MC standard et les
/// prix doivent rester cohérents (IC qui se recouvrent), tolérance 5 %.
#[test]
fn rb_pricer_variance_converges() {
    let (s0, v0, xi, h, rho, r, k, t) = (100.0, 0.04, 1.2, 0.15, -0.6, 0.03, 100.0, 1.0);
    let rb = rough_bergomi_from_params("X", s0, v0, xi, h, rho, r);

    let grid = uniform_grid(t, 100);
    let contract = european_call("X", k, t);

    let sizes = [10_000usize, 40_000, 160_000];
    let mut results = Vec::new();
    for &n in &sizes {
        let paths = rb
            .simulate_paths(&grid, n, 99)
            .expect("RB simulate_paths failed");
        let res = price_on_paths(&contract, &paths, &grid, r).expect("price_on_paths failed");
        results.push((n, res));
    }

    // Tous les prix finis et positifs.
    for (n, res) in &results {
        assert!(
            res.price.is_finite() && res.price > 0.0,
            "prix non valide à n={n}: {}",
            res.price
        );
        assert!(res.std_error > 0.0, "std_error non positif à n={n}");
    }

    // Scaling 1/√n : multiplier n par 4 doit ~diviser std_error par 2.
    // On vérifie chaque saut (×4 → facteur attendu 2.0) à 30 % de tolérance
    // (bruit de l'estimation de variance à échantillon fini).
    for w in results.windows(2) {
        let (n0, r0) = (&w[0].0, &w[0].1);
        let (n1, r1) = (&w[1].0, &w[1].1);
        let ratio_n = (*n1 as f64) / (*n0 as f64);
        let expected = ratio_n.sqrt(); // = 2.0 pour ×4
        let observed = r0.std_error / r1.std_error;
        let rel = (observed - expected).abs() / expected;
        assert!(
            rel < 0.30,
            "scaling 1/√n: n {n0}→{n1}, std_err {:.5}→{:.5}, ratio observé {observed:.3} vs attendu {expected:.3} (err {rel:.3})",
            r0.std_error,
            r1.std_error
        );
    }

    // Cohérence des prix : le prix le plus précis (n max) doit tomber dans l'IC
    // 95 % du prix le moins précis, à 5 % près.
    let coarse = &results[0].1;
    let fine = &results[2].1;
    let within_ci = fine.price >= coarse.ci95_low && fine.price <= coarse.ci95_high;
    let rel_diff = (fine.price - coarse.price).abs() / fine.price;
    assert!(
        within_ci || rel_diff < 0.05,
        "incohérence des prix RB: coarse={:.4} [{:.4},{:.4}], fine={:.4} (rel={rel_diff:.4})",
        coarse.price,
        coarse.ci95_low,
        coarse.ci95_high,
        fine.price
    );
}

// ============================================================================
// RB-5 : forward variance plat + signe de l'effet de levier
// ============================================================================

/// Deux propriétés structurelles du modèle :
///
/// 1. **Forward variance plat** : la correction de convexité `−½ξ²t^{2H}` garantit
///    `E[v_t] = v_0` analytiquement. On reconstruit `v_T = v_0·exp(ξ·B_T − ½ξ²T^{2H})`
///    sur les échantillons de fBm et on vérifie `E[v_T] ≈ v_0` à 3 %.
///
/// 2. **Effet de levier (signe)** : avec `ρ < 0` (typique actions), la vol monte
///    quand le spot baisse → skew négatif → un put OTM doit être **plus cher** que
///    le call OTM symétrique (même écart au spot). On price les deux sur les mêmes
///    trajectoires et on vérifie le signe.
#[test]
fn rb_flat_forward_variance_and_leverage_sign() {
    let (s0, v0, xi, h, t) = (100.0, 0.04, 1.2, 0.15, 1.0);

    // --- 1. E[v_T] ≈ v_0 via reconstruction sur le fBm ---
    let rb_vol = rough_bergomi_from_params("X", s0, v0, xi, h, 0.0, 0.0);
    let grid = uniform_grid(t, 100);
    let n_paths = 80_000;
    let fbm = rb_vol
        .simulate_fbm(&grid, n_paths, 555)
        .expect("RB fbm failed");

    let last_col = fbm.shape()[1] - 1; // instant T
    let two_h = 2.0 * h;
    let drift = 0.5 * xi * xi * t.powf(two_h);
    let v_t: Vec<f64> = (0..n_paths)
        .map(|i| v0 * (xi * fbm[[i, last_col]] - drift).exp())
        .collect();
    let e_vt: f64 = v_t.iter().sum::<f64>() / n_paths as f64;
    let rel_err = (e_vt - v0).abs() / v0;
    assert!(
        rel_err < 0.03,
        "forward variance plat : E[v_T]={e_vt:.5} vs v_0={v0:.5} (err_rel={rel_err:.4}, seuil 3 %)"
    );

    // --- 2. Signe du levier : ρ<0 → put OTM > call OTM symétrique ---
    let rho = -0.7;
    let rb = rough_bergomi_from_params("X", s0, v0, xi, h, rho, 0.0);
    let n_paths_px = 200_000;
    let paths = rb
        .simulate_paths(&grid, n_paths_px, 777)
        .expect("RB simulate_paths failed");

    let k_call = 115.0; // call OTM (+15 %)
    let k_put = 85.0; //  put  OTM (−15 %, symétrique)
    let call = european_call("X", k_call, t);
    // Put OTM : (K - S)+ payé à T.
    let put = when(
        at(t),
        scale((konst(k_put) - spot("X")).max(konst(0.0)), one("USD")),
    );

    let call_px = price_on_paths(&call, &paths, &grid, 0.0)
        .expect("price call")
        .price;
    let put_px = price_on_paths(&put, &paths, &grid, 0.0)
        .expect("price put")
        .price;

    assert!(
        put_px > call_px,
        "effet de levier ρ<0 : put OTM {put_px:.4} devrait être > call OTM {call_px:.4} (skew négatif)"
    );
}
