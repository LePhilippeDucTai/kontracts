//! Tests du jalon J28 — Produits structurés.
//!
//! Critères de complétion (ROADMAP.md) :
//!   - 3 produits : autocallable, reverse convertible, capital-protected note ;
//!   - PV > 0 pour chaque produit (bornes inférieures vérifiées) ;
//!   - bornes de payoff respectées (plafond et plancher économiques) ;
//!   - round-trip JSON des trois expressions DSL.

use kontract::ast::Contract;
use kontract::price_gbm;
use kontract::pricer::McConfig;
use kontract::simulator::Gbm;
use kontract::structured_products::{autocallable, capital_protected_note, reverse_convertible};

const S0: f64 = 100.0;
const R: f64 = 0.05;
const SIGMA: f64 = 0.20;

fn model() -> Gbm {
    Gbm::new("AAPL", S0, R, SIGMA)
}

fn mc(n: usize, steps: usize, seed: u64) -> McConfig {
    McConfig {
        n_paths: n,
        seed,
        steps_per_year: steps,
        rate: R,
        variance_reduction: None,
    }
}

/// PV de l'autocallable strictement positive (notional + coupon > 0).
#[test]
fn autocallable_pv_positive() {
    let contract = autocallable("AAPL", 100.0, 10.0, 150.0, 1.0, "USD");
    let res = price_gbm(&contract, &model(), &mc(50_000, 52, 1)).unwrap();
    assert!(
        res.price > 0.0,
        "PV autocallable = {} (attendu > 0)",
        res.price
    );
}

/// Barrière inatteignable (1e9) → produit ≡ obligation zéro-coupon au notional.
/// `anytime` retourne 0, `until` laisse passer le flux à maturité.
#[test]
fn autocallable_barrier_far_approximates_bond() {
    let notional = 100.0;
    let t = 1.0;
    let contract = autocallable("AAPL", notional, 0.0, 1e9, t, "USD");
    let res = price_gbm(&contract, &model(), &mc(50_000, 52, 2)).unwrap();
    let expected = notional * (-R * t).exp();
    let rel = (res.price - expected).abs() / expected;
    assert!(
        rel < 0.01,
        "autocallable(barrier→∞) MC={:.5} vs ZC={:.5} (rel={:.4})",
        res.price,
        expected,
        rel
    );
}

/// Bornes du reverse convertible :
///   PV > 0 (coupon > put implicite) et PV < discounted(notional+coupon).
#[test]
fn reverse_convertible_payoff_bounds() {
    let notional = 100.0;
    let coupon = 15.0;
    let t = 1.0;
    let contract = reverse_convertible("AAPL", notional, coupon, S0, t, "USD");
    let res = price_gbm(&contract, &model(), &mc(100_000, 1, 3)).unwrap();
    let cap = (notional + coupon) * (-R * t).exp();
    assert!(
        res.price > 0.0,
        "PV reverse convertible = {:.5} (attendu > 0)",
        res.price
    );
    assert!(
        res.price < cap,
        "PV reverse convertible = {:.5} doit être < cap = {:.5}",
        res.price,
        cap
    );
}

/// Capital-protected note avec participation = 0 ≡ obligation zéro-coupon.
#[test]
fn capital_protected_note_floor() {
    let notional = 100.0;
    let t = 1.0;
    let contract = capital_protected_note("AAPL", notional, 0.0, S0, t, "USD");
    let res = price_gbm(&contract, &model(), &mc(50_000, 1, 4)).unwrap();
    let expected = notional * (-R * t).exp();
    let rel = (res.price - expected).abs() / expected;
    assert!(
        rel < 0.01,
        "CPN(participation=0) MC={:.5} vs ZC={:.5} (rel={:.4})",
        res.price,
        expected,
        rel
    );
}

/// CPN avec participation > 0 a une PV strictement supérieure au plancher.
///
/// PV_analytique = notional·e^{−rT} + participation·(notional/s0)·BS_call(s0,s0,T,r,σ)
#[test]
fn capital_protected_note_upside_exceeds_floor() {
    let notional = 100.0;
    let t = 1.0;
    let participation = 0.80;
    let contract = capital_protected_note("AAPL", notional, participation, S0, t, "USD");
    let res = price_gbm(&contract, &model(), &mc(100_000, 1, 5)).unwrap();
    let floor = notional * (-R * t).exp();
    assert!(
        res.price > floor,
        "CPN(participation={participation}) MC={:.5} devrait dépasser le plancher {:.5}",
        res.price,
        floor
    );
}

/// Round-trip JSON : les trois expressions sont sérialisables et restaurables.
#[test]
fn structured_products_json_round_trip() {
    let c1 = autocallable("AAPL", 100.0, 10.0, 120.0, 1.0, "USD");
    let c2 = reverse_convertible("AAPL", 100.0, 15.0, 100.0, 1.0, "USD");
    let c3 = capital_protected_note("AAPL", 100.0, 0.8, 100.0, 1.0, "USD");

    for c in [c1, c2, c3] {
        let json = c.to_json().unwrap();
        let restored = Contract::from_json(&json).unwrap();
        assert_eq!(c, restored);
    }
}
