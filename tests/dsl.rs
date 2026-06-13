//! Tests du jalon J8a — DSL ergonomique côté Rust.
//!
//! Critère : un trader compose des contrats lisibles en quelques lignes ;
//! l'AST produit doit être identique à la forme verbeuse.

use kontract::ast::{and, at, konst, one, s, scale, spot, when, Contract, EUR, USD};
use kontract::compiler::compile;

#[test]
fn fluent_european_call_equals_verbose() {
    // Forme fluide à la README.
    let fluent = ((s("AAPL") - 150.0).clip(0.0) * one(USD)).when(at(1.0));

    // Forme verbeuse via constructeurs explicites.
    let verbose = when(
        at(1.0),
        scale((spot("AAPL") - konst(150.0)).max(konst(0.0)), one("USD")),
    );

    assert_eq!(fluent, verbose);
}

#[test]
fn fluent_knock_out_call() {
    let call = ((s("AAPL") - 100.0).clip(0.0) * one(USD)).when(at(1.0));
    let ko = call.clone().until(s("AAPL").ge(konst(200.0)));

    // La structure se compile en un plan avec barrière.
    let plan = compile(&ko).unwrap();
    assert!(plan.needs_fine_grid);
    assert_eq!(plan.assets, vec!["AAPL".to_string()]);
}

#[test]
fn scalar_arithmetic_both_directions() {
    // observable ⊕ scalaire et scalaire ⊕ observable.
    let put_payoff = (100.0 - s("AAPL")).clip(0.0);
    let expected = (konst(100.0) - spot("AAPL")).max(konst(0.0));
    assert_eq!(put_payoff, expected);

    let scaled = 2.0 * one(USD);
    assert_eq!(scaled, scale(konst(2.0), one("USD")));
}

#[test]
fn currency_constants_work() {
    assert_eq!(one(USD), one("USD"));
    assert_eq!(one(EUR), one("EUR"));
}

#[test]
fn trader_builds_ten_contracts_fluently() {
    // Proxy du critère « 10 contrats en < 10 min » : 10 produits courants,
    // chacun en une ligne lisible, tous compilables.
    let contracts: Vec<Contract> = vec![
        one(USD),
        2.0 * one(USD),
        one(USD).give(),
        ((s("AAPL") - 100.0).clip(0.0) * one(USD)).when(at(1.0)), // call
        ((100.0 - s("AAPL")).clip(0.0) * one(USD)).when(at(1.0)), // put
        (s("AAPL") * one(USD)).when(at(1.0)),                     // forward-ish
        ((s("AAPL") - 100.0).clip(0.0) * one(USD))
            .when(at(1.0))
            .until(s("AAPL").ge(konst(150.0))), // up-and-out call
        ((s("AAPL") - 100.0).clip(0.0) * one(USD))
            .when(at(1.0))
            .anytime(s("AAPL").ge(konst(120.0))), // first-touch
        and(one(USD), one(EUR)),                                  // panier 2 devises
        ((s("AAPL") + s("MSFT")) / 2.0 * one(USD)).when(at(1.0)), // panier moyenné
    ];

    assert_eq!(contracts.len(), 10);
    for c in &contracts {
        // Chaque contrat est bien formé (compilation sans erreur).
        assert!(compile(c).is_ok());
        // … et survit au round-trip JSON (sérialisable).
        let json = c.to_json().unwrap();
        assert_eq!(*c, Contract::from_json(&json).unwrap());
    }
}
