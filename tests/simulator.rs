//! Tests du jalon J3 — simulateur GBM.
//!
//! Critère : les moments empiriques des trajectoires collent aux moments
//! théoriques du GBM (tolérance ~3 erreurs-types).

use kontract::simulator::Gbm;
use kontract::KontractError;

const S0: f64 = 100.0;
const MU: f64 = 0.05;
const SIGMA: f64 = 0.20;
const T: f64 = 1.0;
const N: usize = 200_000;

fn terminal_values(seed: u64) -> Vec<f64> {
    let gbm = Gbm::new("AAPL", S0, MU, SIGMA);
    let arr = gbm.simulate(&[0.0, T], N, seed).expect("simulation");
    arr.column(1).to_vec()
}

fn mean(xs: &[f64]) -> f64 {
    xs.iter().sum::<f64>() / xs.len() as f64
}

fn variance(xs: &[f64], m: f64) -> f64 {
    xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (xs.len() as f64 - 1.0)
}

#[test]
fn initial_column_is_s0_exactly() {
    let gbm = Gbm::new("AAPL", S0, MU, SIGMA);
    let arr = gbm.simulate(&[0.0, 0.5, T], 1000, 7).unwrap();
    for v in arr.column(0) {
        assert_eq!(*v, S0);
    }
}

#[test]
fn terminal_mean_matches_theory() {
    let st = terminal_values(42);
    let emp_mean = mean(&st);

    // E[S_T] = S0 · exp(μT)
    let theo_mean = S0 * (MU * T).exp();
    // Var[S_T] = S0² · exp(2μT) · (exp(σ²T) − 1)
    let theo_var = S0 * S0 * (2.0 * MU * T).exp() * ((SIGMA * SIGMA * T).exp() - 1.0);
    let se = (theo_var / N as f64).sqrt();

    assert!(
        (emp_mean - theo_mean).abs() < 3.0 * se,
        "mean empirique {emp_mean} vs théorique {theo_mean} (3·SE = {})",
        3.0 * se
    );
}

#[test]
fn log_return_moments_match_theory() {
    let st = terminal_values(123);
    let logs: Vec<f64> = st.iter().map(|s| (s / S0).ln()).collect();

    let emp_mean = mean(&logs);
    let emp_var = variance(&logs, emp_mean);

    // log(S_T/S0) ~ N( (μ − ½σ²)T , σ²T )
    let theo_mean = (MU - 0.5 * SIGMA * SIGMA) * T;
    let theo_var = SIGMA * SIGMA * T;

    let se_mean = (theo_var / N as f64).sqrt();
    assert!(
        (emp_mean - theo_mean).abs() < 3.0 * se_mean,
        "mean log {emp_mean} vs {theo_mean}"
    );

    // Variance : tolérance relative 2 % (largement > 3σ pour N = 2·10⁵).
    assert!(
        (emp_var - theo_var).abs() / theo_var < 0.02,
        "var log {emp_var} vs {theo_var}"
    );
}

#[test]
fn same_seed_is_reproducible() {
    let a = terminal_values(2024);
    let b = terminal_values(2024);
    assert_eq!(a, b);
}

#[test]
fn different_seed_differs() {
    let a = terminal_values(1);
    let b = terminal_values(2);
    assert_ne!(a, b);
}

#[test]
fn simulate_paths_round_trips_into_observable_eval() {
    use kontract::ast::{konst, spot};

    let gbm = Gbm::new("AAPL", S0, MU, SIGMA);
    let paths = gbm.simulate_paths(&[0.0, T], 5, 99).unwrap();
    assert_eq!(paths.len(), 5);

    // Le payoff d'un call s'évalue bien sur les paths produits.
    let call = (spot("AAPL") - konst(100.0)).max(konst(0.0));
    for p in &paths {
        let payoff = call.eval(p, 1).unwrap();
        assert!(payoff >= 0.0);
        // S_0 doit être lu exactement au pas 0.
        assert_eq!(spot("AAPL").eval(p, 0).unwrap(), S0);
    }
}

#[test]
fn empty_grid_is_rejected() {
    let gbm = Gbm::new("AAPL", S0, MU, SIGMA);
    assert!(matches!(
        gbm.simulate(&[], 10, 1),
        Err(KontractError::InconsistentPath(_))
    ));
}

#[test]
fn decreasing_grid_is_rejected() {
    let gbm = Gbm::new("AAPL", S0, MU, SIGMA);
    assert!(matches!(
        gbm.simulate(&[0.0, 1.0, 0.5], 10, 1),
        Err(KontractError::InconsistentPath(_))
    ));
}
