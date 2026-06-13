//! Tests du jalon J1 — AST et sérialisation JSON.
//!
//! Critère de complétion : round-trip JSON sur 10 contrats couvrant l'ensemble
//! des combinateurs primitifs.

use kontract::ast::{
    and, anytime, at, give, konst, one, or, scale, spot, until, when, Condition, Contract,
};

/// Vérifie qu'un contrat survit à un aller-retour JSON sans perte.
fn assert_round_trip(c: &Contract) {
    let json = c.to_json().expect("sérialisation");
    let back = Contract::from_json(&json).expect("désérialisation");
    assert_eq!(c, &back, "round-trip échoué pour {json}");
}

/// Un call européen exprimé dans le DSL : `max(S - K, 0)` payé en USD à T.
fn european_call(asset: &str, strike: f64, maturity: f64) -> Contract {
    let payoff = (spot(asset) - konst(strike)).max(konst(0.0));
    when(at(maturity), scale(payoff, one("USD")))
}

#[test]
fn round_trip_zero() {
    assert_round_trip(&Contract::Zero);
}

#[test]
fn round_trip_one() {
    assert_round_trip(&one("USD"));
    assert_round_trip(&one("EUR"));
}

#[test]
fn round_trip_each_primitive() {
    // Chaque combinateur primitif au moins une fois.
    assert_round_trip(&give(one("USD")));
    assert_round_trip(&and(one("USD"), give(one("EUR"))));
    assert_round_trip(&or(one("USD"), one("EUR")));
    assert_round_trip(&scale(konst(100.0), one("USD")));
    assert_round_trip(&when(at(1.0), one("USD")));
    assert_round_trip(&anytime(spot("AAPL").ge(konst(200.0)), one("USD")));
    assert_round_trip(&until(spot("AAPL").ge(konst(200.0)), one("USD")));
}

#[test]
fn round_trip_observable_arithmetic() {
    // Couvre Add / Sub / Mul / Div / Neg / Max / Min.
    let obs = (((spot("AAPL") + konst(1.0)) * konst(2.0) - spot("MSFT")) / konst(3.0))
        .max(konst(0.0))
        .min(konst(1000.0));
    assert_round_trip(&scale(obs, one("USD")));
    assert_round_trip(&scale(-spot("AAPL"), one("USD")));
}

#[test]
fn round_trip_condition_combinators() {
    let cond = !spot("AAPL")
        .ge(konst(200.0))
        .and(spot("MSFT").lt(konst(100.0)))
        .or(Condition::Bool(false));
    assert_round_trip(&when(cond, one("USD")));
}

#[test]
fn round_trip_ten_contracts() {
    // La suite exigée par le critère : 10 contrats distincts et composés.
    let contracts = vec![
        Contract::Zero,
        one("USD"),
        give(one("EUR")),
        and(one("USD"), give(one("EUR"))),
        or(one("USD"), one("EUR")),
        scale(konst(100.0), one("USD")),
        scale(spot("AAPL"), one("USD")),
        when(at(1.0), one("USD")),
        european_call("AAPL", 150.0, 1.0),
        until(
            spot("AAPL").ge(konst(200.0)),
            european_call("AAPL", 150.0, 1.0),
        ),
    ];
    assert_eq!(contracts.len(), 10);
    for c in &contracts {
        assert_round_trip(c);
    }
}

#[test]
fn json_is_stable_and_readable() {
    // Le JSON doit refléter la structure (tag externe par défaut de serde).
    let c = scale(konst(2.0), one("USD"));
    let json = c.to_json().unwrap();
    assert!(json.contains("Scale"));
    assert!(json.contains("One"));
    assert!(json.contains("USD"));
}

#[test]
fn from_json_rejects_garbage() {
    assert!(Contract::from_json("{ not valid json").is_err());
    assert!(Contract::from_json("\"Unknown\"").is_err());
}
