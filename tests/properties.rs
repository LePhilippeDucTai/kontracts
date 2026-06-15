//! Tests de **propriété** (proptest) — invariants robustes du moteur.
//!
//! Complètent les tests par cas : on génère des paramètres aléatoires et on
//! vérifie des invariants qui doivent tenir pour *tous* les inputs (round-trip
//! de sérialisation, monotonies de prix, positivité, absence de NaN).

use kontract::pricer::McConfig;
use kontract::products::{european_call, european_put};
use kontract::{price_gbm, Contract, Gbm};
use proptest::prelude::*;

fn cfg(seed: u64) -> McConfig {
    // Petit nombre de trajectoires + graine fixe (CRN) : monotonies exactes,
    // exécution rapide sous proptest.
    McConfig {
        n_paths: 4000,
        seed,
        steps_per_year: 1,
        rate: 0.03,
        variance_reduction: None,
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(24))]

    /// Round-trip JSON de l'AST : `from_json(to_json(c)) == c` pour un call/put
    /// quelconque (l'AST est pur et sérialisable — invariant n°1 du projet).
    #[test]
    fn prop_ast_json_roundtrip(
        k in 1.0f64..500.0,
        t in 0.05f64..5.0,
        is_call in any::<bool>(),
    ) {
        let c: Contract = if is_call {
            european_call("S", k, t, "USD")
        } else {
            european_put("S", k, t, "USD")
        };
        let json = c.to_json().unwrap();
        let back = Contract::from_json(&json).unwrap();
        prop_assert_eq!(c, back);
    }

    /// Le prix d'un call est **croissant** en spot (couverture par CRN : mêmes
    /// trajectoires, payoff par trajectoire croissant en S₀).
    #[test]
    fn prop_call_increasing_in_spot(
        s_lo in 20.0f64..120.0,
        bump in 1.0f64..50.0,
        vol in 0.05f64..0.8,
    ) {
        let call = european_call("S", 100.0, 1.0, "USD");
        let lo = price_gbm(&call, &Gbm::new("S", s_lo, 0.03, vol), &cfg(7)).unwrap().price;
        let hi = price_gbm(&call, &Gbm::new("S", s_lo + bump, 0.03, vol), &cfg(7)).unwrap().price;
        prop_assert!(hi >= lo - 1e-9, "call non croissant en spot: {hi} < {lo}");
        prop_assert!(lo.is_finite() && hi.is_finite());
        prop_assert!(lo >= -1e-9, "prix négatif: {lo}");
    }

    /// Le prix d'un call est **décroissant** en strike (même CRN).
    #[test]
    fn prop_call_decreasing_in_strike(
        k_lo in 50.0f64..150.0,
        bump in 1.0f64..50.0,
        vol in 0.05f64..0.8,
    ) {
        let model = Gbm::new("S", 100.0, 0.03, vol);
        let lo_k = price_gbm(&european_call("S", k_lo, 1.0, "USD"), &model, &cfg(9)).unwrap().price;
        let hi_k = price_gbm(&european_call("S", k_lo + bump, 1.0, "USD"), &model, &cfg(9)).unwrap().price;
        prop_assert!(hi_k <= lo_k + 1e-9, "call non décroissant en strike: {hi_k} > {lo_k}");
        prop_assert!(hi_k >= -1e-9);
    }

    /// Garman-Kohlhagen : parité put-call exacte pour des paramètres quelconques.
    #[test]
    fn prop_gk_put_call_parity(
        x0 in 0.5f64..2.0,
        k in 0.5f64..2.0,
        t in 0.1f64..3.0,
        r_d in -0.01f64..0.1,
        r_f in -0.01f64..0.1,
        sigma in 0.02f64..0.6,
    ) {
        let c = kontract::garman_kohlhagen_call(x0, k, t, r_d, r_f, sigma);
        let p = kontract::garman_kohlhagen_put(x0, k, t, r_d, r_f, sigma);
        let parity = x0 * (-r_f * t).exp() - k * (-r_d * t).exp();
        prop_assert!(((c - p) - parity).abs() < 1e-9);
        prop_assert!(c >= -1e-12 && p >= -1e-12);
    }
}
