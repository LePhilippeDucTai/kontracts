//! Tests du jalon J9c — batch pricing (portefeuille).
//!
//! Critère : pricer 100+ contrats rapidement via une simulation partagée ;
//! les prix doivent coïncider avec le pricing individuel.

use std::time::Instant;

use kontract::ast::{at, one, s, Contract};
use kontract::pricer::{price_batch_gbm, price_gbm, McConfig};
use kontract::simulator::Gbm;

fn call(strike: f64) -> Contract {
    ((s("AAPL") - strike).clip(0.0) * one("USD")).when(at(1.0))
}

fn cfg() -> McConfig {
    McConfig {
        n_paths: 50_000,
        seed: 2024,
        steps_per_year: 1,
        rate: 0.05,
    }
}

#[test]
fn batch_matches_individual_pricing() {
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.20);
    let contracts: Vec<Contract> = (80..=120).map(|k| call(k as f64)).collect();

    let batch = price_batch_gbm(&contracts, &model, &cfg()).unwrap();
    assert_eq!(batch.len(), contracts.len());

    // Portefeuille européen homogène : grille unifiée = grille individuelle,
    // même graine → prix identiques au bit près.
    for (c, b) in contracts.iter().zip(&batch) {
        let single = price_gbm(c, &model, &cfg()).unwrap();
        assert!((single.price - b.price).abs() < 1e-12);
    }
}

#[test]
fn batch_is_monotone_in_strike() {
    // Le prix d'un call décroît avec le strike.
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.20);
    let contracts: Vec<Contract> = (80..=120).map(|k| call(k as f64)).collect();
    let prices = price_batch_gbm(&contracts, &model, &cfg()).unwrap();
    for w in prices.windows(2) {
        assert!(w[0].price >= w[1].price - 1e-9);
    }
}

#[test]
fn empty_portfolio_is_empty() {
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.20);
    let res = price_batch_gbm(&[], &model, &cfg()).unwrap();
    assert!(res.is_empty());
}

#[test]
fn prices_one_hundred_contracts_quickly() {
    // 100 contrats, simulation partagée. Cible release : < 500 ms.
    // En debug (non optimisé) on relâche la borne ; l'évidence < 500 ms est
    // produite par `cargo test --release` (cf. PROGRESS).
    let model = Gbm::new("AAPL", 100.0, 0.05, 0.20);
    let contracts: Vec<Contract> = (1..=100).map(|i| call(60.0 + i as f64)).collect();

    let start = Instant::now();
    let prices = price_batch_gbm(&contracts, &model, &cfg()).unwrap();
    let elapsed = start.elapsed();

    assert_eq!(prices.len(), 100);
    let limit_ms = if cfg!(debug_assertions) { 5000 } else { 500 };
    assert!(
        elapsed.as_millis() < limit_ms,
        "batch de 100 contrats en {} ms (limite {} ms)",
        elapsed.as_millis(),
        limit_ms
    );
}
