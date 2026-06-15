//! Exemple : pricing d'un call européen et de ses Greeks (API Rust).
//!
//! `cargo run --example price_call`

use kontract::greeks::{greeks_gbm, BumpSizes};
use kontract::numerics::black_scholes_call;
use kontract::pricer::McConfig;
use kontract::products::european_call;
use kontract::{price_gbm, Gbm};

fn main() {
    let (s0, k, t, r, sigma) = (100.0, 100.0, 1.0, 0.05, 0.2);

    let call = european_call("AAPL", k, t, "USD");
    let model = Gbm::new("AAPL", s0, r, sigma);
    let cfg = McConfig {
        n_paths: 200_000,
        seed: 42,
        steps_per_year: 1,
        rate: r,
        variance_reduction: None,
    };

    let res = price_gbm(&call, &model, &cfg).expect("pricing");
    let bs = black_scholes_call(s0, k, t, r, sigma);
    println!("Call ATM");
    println!(
        "  MC   = {:.4}  (±{:.4}, IC95 [{:.4}, {:.4}])",
        res.price, res.std_error, res.ci95_low, res.ci95_high
    );
    println!("  BS   = {:.4}", bs);

    let g = greeks_gbm(&call, "AAPL", s0, sigma, &cfg, &BumpSizes::default()).expect("greeks");
    println!(
        "Greeks: delta={:.4} gamma={:.4} vega={:.4} rho={:.4}",
        g.delta, g.gamma, g.vega, g.rho
    );
}
