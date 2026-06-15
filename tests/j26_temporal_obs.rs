//! Tests du jalon J26 — Observables temporels (Average, RunningMax, RunningMin).
//!
//! Critère : Asian call et lookback call valorisés via le moteur existant,
//! avec validation économique (bornes, monotonie, AM-GM) et round-trip JSON.
//! Le compilateur doit détecter `needs_fine_grid = true` pour ces observables.

use kontract::ast::{at, average, average_over, konst, one, running_max, running_min, scale, spot};
use kontract::compiler::compile;
use kontract::observable::Path;
use kontract::pricer::McConfig;
use kontract::{price_gbm, Contract, Gbm};

fn mc(n_paths: usize, seed: u64) -> McConfig {
    McConfig {
        n_paths,
        seed,
        steps_per_year: 252,
        rate: 0.05,
        variance_reduction: None,
    }
}

/// La moyenne d'une constante vaut la constante, quel que soit le pas.
#[test]
fn test_average_const() {
    let path = Path::new(vec![0.0, 0.25, 0.5, 0.75, 1.0])
        .with_asset("X", vec![100.0, 105.0, 98.0, 102.0, 110.0])
        .unwrap();
    let obs = average(konst(7.0));
    (0..5).for_each(|t| {
        let v = obs.eval(&path, t).unwrap();
        assert!((v - 7.0).abs() < 1e-14, "step {t}: {v}");
    });
}

/// RunningMax sur un chemin strictement croissant = valeur au pas courant.
#[test]
fn test_running_max_ascending() {
    let path = Path::new(vec![0.0, 0.25, 0.5, 0.75, 1.0])
        .with_asset("S", vec![1.0, 2.0, 3.0, 4.0, 5.0])
        .unwrap();
    let obs = running_max(spot("S"));
    (0..5).for_each(|t| {
        let v = obs.eval(&path, t).unwrap();
        assert!((v - (t + 1) as f64).abs() < 1e-14, "step {t}: {v}");
    });
}

/// RunningMin sur un chemin strictement décroissant = valeur au pas courant.
#[test]
fn test_running_min_descending() {
    let path = Path::new(vec![0.0, 0.25, 0.5, 0.75, 1.0])
        .with_asset("S", vec![5.0, 4.0, 3.0, 2.0, 1.0])
        .unwrap();
    let obs = running_min(spot("S"));
    (0..5).for_each(|t| {
        let v = obs.eval(&path, t).unwrap();
        assert!((v - (5 - t) as f64).abs() < 1e-14, "step {t}: {v}");
    });
}

/// `average_over` avec fenêtre explicite moyenne uniquement les pas dans la fenêtre.
#[test]
fn test_average_windowed() {
    // Grille : t = 0, 0.25, 0.5, 0.75, 1.0 ; valeurs : 1, 2, 3, 4, 5
    let path = Path::new(vec![0.0, 0.25, 0.5, 0.75, 1.0])
        .with_asset("S", vec![1.0, 2.0, 3.0, 4.0, 5.0])
        .unwrap();
    // Moyenne sur [0.25, 0.75] → valeurs 2, 3, 4 → moyenne = 3
    let obs = average_over(spot("S"), 0.25, 0.75);
    let v = obs.eval(&path, 4).unwrap();
    assert!((v - 3.0).abs() < 1e-12, "average_over [0.25,0.75] = {v}");
}

/// Inégalité AM-GM sur path concret : moyenne arithmétique ≥ géométrique.
#[test]
fn test_am_ge_gm_on_path() {
    let values = vec![90.0_f64, 95.0, 100.0, 105.0, 110.0];
    let path = Path::new(vec![0.0, 0.25, 0.5, 0.75, 1.0])
        .with_asset("S", values.clone())
        .unwrap();
    let arith = average(spot("S")).eval(&path, 4).unwrap();
    let geom = (values.iter().map(|x| x.ln()).sum::<f64>() / values.len() as f64).exp();
    assert!(arith >= geom - 1e-10, "AM {arith:.6} < GM {geom:.6}");
}

/// Asian call < vanilla call (même K, T, σ) — l'averaging réduit la variance
/// effective, donc le prix par Jensen.
#[test]
fn test_asian_lt_vanilla_mc() {
    let (s0, k, t_mat, r, sigma) = (100.0, 100.0, 1.0, 0.05, 0.20);
    let model = Gbm::new("S", s0, r, sigma);

    let asian = scale((average(spot("S")) - k).clip(0.0), one("USD")).when(at(t_mat));
    let vanilla = scale((spot("S") - k).clip(0.0), one("USD")).when(at(t_mat));

    let cfg = mc(100_000, 42);
    let asian_price = price_gbm(&asian, &model, &cfg).unwrap().price;
    let vanilla_price = price_gbm(&vanilla, &model, &cfg).unwrap().price;

    assert!(asian_price > 0.0, "Asian price must be positive");
    assert!(
        asian_price < vanilla_price,
        "Asian {asian_price:.4} doit être < vanilla {vanilla_price:.4}"
    );
}

/// Lookback call > vanilla call (même K, T, σ) : max(S_t) ≥ S_T p.s.
#[test]
fn test_lookback_gt_vanilla_mc() {
    let (s0, k, t_mat, r, sigma) = (100.0, 90.0, 1.0, 0.05, 0.20);
    let model = Gbm::new("S", s0, r, sigma);

    let lookback = scale((running_max(spot("S")) - k).clip(0.0), one("USD")).when(at(t_mat));
    let vanilla = scale((spot("S") - k).clip(0.0), one("USD")).when(at(t_mat));

    let cfg = mc(100_000, 43);
    let lb_price = price_gbm(&lookback, &model, &cfg).unwrap().price;
    let van_price = price_gbm(&vanilla, &model, &cfg).unwrap().price;

    assert!(
        lb_price > van_price,
        "Lookback {lb_price:.4} doit être > vanilla {van_price:.4}"
    );
}

/// Round-trip JSON + le compilateur détecte `needs_fine_grid = true`.
#[test]
fn test_round_trip_json_and_compiler_flags() {
    let asian = scale((average(spot("S")) - konst(100.0)).clip(0.0), one("USD")).when(at(1.0));
    let lb = scale(
        (running_max(spot("S")) - konst(100.0)).clip(0.0),
        one("USD"),
    )
    .when(at(1.0));
    let lb_min = scale(
        (konst(100.0) - running_min(spot("S"))).clip(0.0),
        one("USD"),
    )
    .when(at(1.0));
    let vanilla = scale((spot("S") - konst(100.0)).clip(0.0), one("USD")).when(at(1.0));

    // Round-trip serde
    for c in [&asian, &lb, &lb_min] {
        let json = c.to_json().unwrap();
        let c2: Contract = Contract::from_json(&json).unwrap();
        assert_eq!(c, &c2, "round-trip JSON échoué");
    }

    // Compilateur : grille fine pour observables temporels, pas pour vanilla
    assert!(
        compile(&asian).unwrap().needs_fine_grid,
        "Asian → grille fine"
    );
    assert!(
        compile(&lb).unwrap().needs_fine_grid,
        "Lookback max → grille fine"
    );
    assert!(
        compile(&lb_min).unwrap().needs_fine_grid,
        "Lookback min → grille fine"
    );
    assert!(
        !compile(&vanilla).unwrap().needs_fine_grid,
        "Vanilla → pas de grille fine"
    );
}
