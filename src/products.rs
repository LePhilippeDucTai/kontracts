//! Catalogue de produits exprimés dans le DSL (jalon J9).
//!
//! **Aucun de ces produits n'est connu du moteur** : ce sont de simples
//! expressions des combinateurs primitifs. Ce module est purement un confort
//! d'écriture pour les utilisateurs et la suite de validation.
//!
//! Limites assumées (cf. ROADMAP/PROGRESS) :
//!   - l'**asian** (moyenne arithmétique le long du path) nécessite un observable
//!     d'agrégation temporelle, absent de l'algèbre actuelle → reporté ;
//!   - la **swaption** nécessite des taux stochastiques (jambe variable) →
//!     reportée au jalon J24.

use crate::ast::{at, konst, one, scale, spot, when, Condition, Contract};

/// Obligation zéro-coupon : reçoit 1 unité de `ccy` à `t`.
pub fn zero_coupon_bond(ccy: &str, t: f64) -> Contract {
    when(at(t), one(ccy))
}

/// Call européen : `max(S − K, 0)` payé en `ccy` à `t`.
pub fn european_call(asset: &str, k: f64, t: f64, ccy: &str) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one(ccy)),
    )
}

/// Put européen : `max(K − S, 0)` payé en `ccy` à `t`.
pub fn european_put(asset: &str, k: f64, t: f64, ccy: &str) -> Contract {
    when(
        at(t),
        scale((konst(k) - spot(asset)).max(konst(0.0)), one(ccy)),
    )
}

/// Forward : reçoit `S_T − K` à `t` (flux pouvant être négatif).
pub fn forward(asset: &str, k: f64, t: f64, ccy: &str) -> Contract {
    when(at(t), scale(spot(asset) - konst(k), one(ccy)))
}

/// Straddle : call + put de même strike (pari sur la volatilité).
pub fn straddle(asset: &str, k: f64, t: f64, ccy: &str) -> Contract {
    Contract::And(
        Box::new(european_call(asset, k, t, ccy)),
        Box::new(european_put(asset, k, t, ccy)),
    )
}

/// Bull call spread : long call `k_low`, short call `k_high` (`k_low < k_high`).
pub fn bull_call_spread(asset: &str, k_low: f64, k_high: f64, t: f64, ccy: &str) -> Contract {
    Contract::And(
        Box::new(european_call(asset, k_low, t, ccy)),
        Box::new(Contract::Give(Box::new(european_call(
            asset, k_high, t, ccy,
        )))),
    )
}

/// Digital cash-or-nothing call : paie `payout` à `t` si `S_T ≥ K`.
pub fn cash_or_nothing_call(asset: &str, k: f64, payout: f64, t: f64, ccy: &str) -> Contract {
    when(
        at(t),
        when(
            Condition::Ge(spot(asset), konst(k)),
            scale(konst(payout), one(ccy)),
        ),
    )
}

/// Up-and-out call : call européen annulé si `S` touche/dépasse `barrier`.
pub fn up_and_out_call(asset: &str, k: f64, barrier: f64, t: f64, ccy: &str) -> Contract {
    Contract::Until(
        Condition::Ge(spot(asset), konst(barrier)),
        Box::new(european_call(asset, k, t, ccy)),
    )
}

/// Down-and-out call : call européen annulé si `S` touche/descend sous `barrier`.
pub fn down_and_out_call(asset: &str, k: f64, barrier: f64, t: f64, ccy: &str) -> Contract {
    Contract::Until(
        Condition::Le(spot(asset), konst(barrier)),
        Box::new(european_call(asset, k, t, ccy)),
    )
}
