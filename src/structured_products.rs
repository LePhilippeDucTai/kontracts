//! Produits structurés (J28).
//!
//! Trois produits composites exprimés **entièrement** dans le DSL primitif.
//! Aucun cas spécial n'est ajouté au moteur de pricing ni à l'AST.
//!
//! Produits implémentés :
//!   - [`autocallable`]           : note à rachat anticipé conditionnel (first-touch) ;
//!   - [`reverse_convertible`]    : obligation à coupon élevé, capital à risque ;
//!   - [`capital_protected_note`] : capital garanti à 100 % + participation à la hausse.

use crate::ast::{
    and, anytime, at, give, konst, one, scale, spot, until, when, Condition, Contract,
};

/// Note autocallable : rachat anticipé si le sous-jacent franchit la barrière.
///
/// Sémantique :
/// - À la première date où `S_t ≥ barrier` : reçoit `notional + coupon` (first-touch via `anytime`).
/// - Si la barrière n'est jamais atteinte : reçoit `notional` à la maturité
///   (`until` laisse passer le flux final tant que la condition reste fausse).
pub fn autocallable(
    asset: &str,
    notional: f64,
    coupon: f64,
    barrier: f64,
    maturity: f64,
    ccy: &str,
) -> Contract {
    let barrier_hit = Condition::Ge(spot(asset), konst(barrier));
    and(
        anytime(
            barrier_hit.clone(),
            scale(konst(notional + coupon), one(ccy)),
        ),
        until(
            barrier_hit,
            when(at(maturity), scale(konst(notional), one(ccy))),
        ),
    )
}

/// Reverse convertible : coupon généreux, capital à risque si le sous-jacent chute.
///
/// À la maturité :
/// - Paie `notional + coupon` (certain).
/// - Soustrait le payoff d'un put short : `(notional / strike) · max(strike − S_T, 0)`.
///
/// Si `S_T ≥ strike` : flux net = `notional + coupon`.
/// Si `S_T < strike` : flux net = `coupon + notional · S_T / strike` (risque en capital).
pub fn reverse_convertible(
    asset: &str,
    notional: f64,
    coupon: f64,
    strike: f64,
    maturity: f64,
    ccy: &str,
) -> Contract {
    let embedded_put = (konst(strike) - spot(asset)).clip(0.0) * (notional / strike);
    and(
        when(at(maturity), scale(konst(notional + coupon), one(ccy))),
        give(when(at(maturity), scale(embedded_put, one(ccy)))),
    )
}

/// Capital Protected Note (CPN) : capital garanti + participation à la hausse.
///
/// À la maturité :
/// - Garantit `notional` (plancher, toujours versé).
/// - Ajoute `participation · (notional / s0) · max(S_T − s0, 0)` (hausse optionnelle).
///
/// PV ≥ `notional · e^{−r·T}` quelle que soit l'évolution du sous-jacent.
pub fn capital_protected_note(
    asset: &str,
    notional: f64,
    participation: f64,
    s0: f64,
    maturity: f64,
    ccy: &str,
) -> Contract {
    let upside = (spot(asset) - konst(s0)).clip(0.0) * (participation * notional / s0);
    and(
        when(at(maturity), scale(konst(notional), one(ccy))),
        when(at(maturity), scale(upside, one(ccy))),
    )
}
