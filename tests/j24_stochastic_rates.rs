//! Tests du jalon J24 — Taux courts stochastiques (Vasicek / Hull-White).
//!
//! Critère : « Swaption vs analytique ». On valide aussi la levée de
//! l'actualisation déterministe : une obligation zéro-coupon de l'AST, price
//! sous actualisation stochastique, retrouve `P(0,T)` analytique.

use kontract::pricer::McConfig;
use kontract::products::zero_coupon_bond;
use kontract::rates::{price_under_short_rate, swaption_price_mc, ShortRateModel};
use kontract::{HullWhite, Swaption, Vasicek};

fn mc(n_paths: usize, seed: u64) -> McConfig {
    McConfig {
        n_paths,
        seed,
        steps_per_year: 100,
        rate: 0.0,
        variance_reduction: None,
    }
}

/// Moments empiriques du taux court de Vasicek à l'horizon `T` vs théorie :
/// `E[r_T] = r₀e^{−aT} + b(1−e^{−aT})`, `Var[r_T] = σ²/2a·(1−e^{−2aT})`.
#[test]
fn test_vasicek_short_rate_moments() {
    let model = Vasicek::new(0.03, 0.5, 0.05, 0.02);
    let t = 3.0;
    let grid: Vec<f64> = (0..=300).map(|k| t * k as f64 / 300.0).collect();
    let rates = model
        .simulate_short_rate(&grid, 200_000, 7)
        .expect("simulate");

    let last: Vec<f64> = rates.column(grid.len() - 1).to_vec();
    let n = last.len() as f64;
    let mean = last.iter().sum::<f64>() / n;
    let var = last.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n;

    let e = (-model.a * t).exp();
    let mean_th = model.r0 * e + model.b * (1.0 - e);
    let var_th = model.sigma * model.sigma * (1.0 - (-2.0 * model.a * t).exp()) / (2.0 * model.a);

    assert!(
        (mean - mean_th).abs() < 1e-3,
        "E[r_T] empirique {mean:.5} vs théorie {mean_th:.5}"
    );
    assert!(
        (var - var_th).abs() / var_th < 0.05,
        "Var[r_T] empirique {var:.6} vs théorie {var_th:.6}"
    );
}

/// **Levée de l'actualisation déterministe** : un zéro-coupon `when(at(T), one)`
/// de l'AST, price sous actualisation stochastique Vasicek, retrouve `P(0,T)`
/// analytique — sans aucune modification du contrat.
#[test]
fn test_zero_coupon_stochastic_discount_vs_analytic() {
    let model = Vasicek::new(0.03, 0.6, 0.05, 0.015);
    let maturities = [0.5, 1.0, 2.0, 5.0];

    maturities.iter().for_each(|&t| {
        let zcb = zero_coupon_bond("USD", t);
        let res = price_under_short_rate(&zcb, &model, &mc(200_000, 11)).expect("price");
        let analytic = model.discount_bond0(t);
        assert!(
            (res.price - analytic).abs() < 2e-3,
            "P(0,{t}) MC {:.5} vs analytique {:.5} (CI [{:.5}, {:.5}])",
            res.price,
            analytic,
            res.ci95_low,
            res.ci95_high
        );
    });
}

/// **Critère du jalon** : prix Monte-Carlo d'une swaption payeuse vs formule
/// analytique de Jamshidian (Vasicek). Actualisation par le compte de
/// capitalisation réalisé.
#[test]
fn test_swaption_payer_mc_vs_analytic() {
    let model = Vasicek::new(0.04, 0.5, 0.05, 0.012);
    // Swaption 1Y sur swap 2Y, paiements semestriels, fixe 5 %.
    let swaption = Swaption::level(1.0, 0.5, 4, 0.05, true);

    let analytic = model.swaption_analytic(&swaption).expect("analytic");
    let mc_res = swaption_price_mc(&model, &swaption, &mc(300_000, 21), 120).expect("mc");

    assert!(analytic > 0.0, "swaption analytique doit être positive");
    let rel = (mc_res.price - analytic).abs() / analytic;
    assert!(
        rel < 0.03,
        "payer swaption MC {:.6} vs analytique {:.6} (rel {:.4}, CI [{:.6}, {:.6}])",
        mc_res.price,
        analytic,
        rel,
        mc_res.ci95_low,
        mc_res.ci95_high
    );
}

/// Receveur aussi : MC vs analytique (Jamshidian → calls sur zéro-coupons).
#[test]
fn test_swaption_receiver_mc_vs_analytic() {
    let model = Vasicek::new(0.04, 0.5, 0.05, 0.012);
    let swaption = Swaption::level(1.0, 0.5, 4, 0.05, false);

    let analytic = model.swaption_analytic(&swaption).expect("analytic");
    let mc_res = swaption_price_mc(&model, &swaption, &mc(300_000, 22), 120).expect("mc");

    let rel = (mc_res.price - analytic).abs() / analytic;
    assert!(
        rel < 0.03,
        "receiver swaption MC {:.6} vs analytique {:.6} (rel {:.4})",
        mc_res.price,
        analytic,
        rel
    );
}

/// Parité payeur/receveur : `Payer − Receiver = P(0,T₀) − Σ cᵢ P(0,Tᵢ)`
/// (valeur du swap forward payeur), vérifiée sur les prix **analytiques**.
#[test]
fn test_swaption_payer_receiver_parity() {
    let model = Vasicek::new(0.035, 0.4, 0.045, 0.01);
    let tenor = 0.5;
    let n = 6; // swap 3Y
    let k = 0.045;
    let expiry = 2.0;

    let payer = Swaption::level(expiry, tenor, n, k, true);
    let receiver = Swaption::level(expiry, tenor, n, k, false);

    let p = model.swaption_analytic(&payer).expect("payer");
    let r = model.swaption_analytic(&receiver).expect("receiver");

    // Valeur du swap forward payeur : P(0,T₀) − Σ cᵢ P(0,Tᵢ).
    let fixed_pv: f64 = (1..=n)
        .map(|i| {
            let ti = expiry + i as f64 * tenor;
            let ci = k * tenor + if i == n { 1.0 } else { 0.0 };
            ci * model.discount_bond0(ti)
        })
        .sum();
    let forward_swap = model.discount_bond0(expiry) - fixed_pv;

    assert!(
        (p - r - forward_swap).abs() < 1e-9,
        "parité : Payer − Receiver = {:.8}, swap forward = {:.8}",
        p - r,
        forward_swap
    );
}

/// Hull-White sur courbe plate : reproduit exactement `P(0,T) = e^{−r₀T}`
/// (analytiquement) et le retrouve par actualisation stochastique MC.
#[test]
fn test_hull_white_flat_curve_reproduction() {
    let r0 = 0.03;
    let model = HullWhite::new(r0, 0.5, 0.01);

    // Analytique : P(0,T) = e^{−r₀T} exactement.
    [0.5, 1.0, 3.0].iter().for_each(|&t| {
        let analytic = model.discount_bond0(t);
        let flat = (-r0 * t).exp();
        assert!(
            (analytic - flat).abs() < 1e-10,
            "HW courbe plate P(0,{t}) {analytic:.8} ≠ e^(−r₀T) {flat:.8}"
        );
    });

    // MC : actualisation stochastique d'un ZCB ≈ e^{−r₀T}.
    let zcb = zero_coupon_bond("USD", 2.0);
    let res = price_under_short_rate(&zcb, &model, &mc(200_000, 31)).expect("price");
    let flat = (-r0 * 2.0_f64).exp();
    assert!(
        (res.price - flat).abs() < 2e-3,
        "HW P(0,2) MC {:.5} vs courbe plate {:.5}",
        res.price,
        flat
    );
}

/// Cohérence : à l'ATM (`K` = taux de swap forward), payeur ≈ receveur.
#[test]
fn test_atm_swaption_symmetry() {
    let model = Vasicek::new(0.04, 0.5, 0.05, 0.012);
    let tenor = 0.5;
    let n = 4;
    let expiry = 1.0;

    // Taux de swap forward ATM : (P(0,T₀) − P(0,Tn)) / Σ τ P(0,Tᵢ).
    let annuity: f64 = (1..=n)
        .map(|i| tenor * model.discount_bond0(expiry + i as f64 * tenor))
        .sum();
    let tn = expiry + n as f64 * tenor;
    let fwd_rate = (model.discount_bond0(expiry) - model.discount_bond0(tn)) / annuity;

    let payer = model
        .swaption_analytic(&Swaption::level(expiry, tenor, n, fwd_rate, true))
        .expect("payer");
    let receiver = model
        .swaption_analytic(&Swaption::level(expiry, tenor, n, fwd_rate, false))
        .expect("receiver");

    assert!(
        (payer - receiver).abs() < 1e-6,
        "ATM : payer {payer:.8} ≈ receiver {receiver:.8}"
    );
}
