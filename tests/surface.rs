//! Tests du jalon J7b — surfaces de Greeks vs Black-Scholes.

use kontract::ast::{at, konst, one, scale, spot, when, Contract};
use kontract::greeks::BumpSizes;
use kontract::pricer::McConfig;
use kontract::surface::greek_surface;

fn erf(x: f64) -> f64 {
    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + 0.327_591_1 * x);
    let y = 1.0
        - (((((1.061_405_429 * t - 1.453_152_027) * t) + 1.421_413_741) * t - 0.284_496_736) * t
            + 0.254_829_592)
            * t
            * (-x * x).exp();
    sign * y
}

fn norm_cdf(x: f64) -> f64 {
    0.5 * (1.0 + erf(x / std::f64::consts::SQRT_2))
}

fn norm_pdf(x: f64) -> f64 {
    (-0.5 * x * x).exp() / (2.0 * std::f64::consts::PI).sqrt()
}

fn bs_delta(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> f64 {
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    norm_cdf(d1)
}

fn bs_vega(s: f64, k: f64, r: f64, sigma: f64, t: f64) -> f64 {
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * t) / (sigma * t.sqrt());
    s * norm_pdf(d1) * t.sqrt()
}

fn european_call(asset: &str, k: f64, t: f64) -> Contract {
    when(
        at(t),
        scale((spot(asset) - konst(k)).max(konst(0.0)), one("USD")),
    )
}

fn cfg() -> McConfig {
    McConfig {
        n_paths: 200_000,
        seed: 2024,
        steps_per_year: 1,
        rate: 0.05,
    }
}

#[test]
fn surface_dimensions_are_consistent() {
    let spots = vec![90.0, 100.0, 110.0];
    let vols = vec![0.15, 0.20, 0.25];
    let surf = greek_surface(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        &spots,
        &vols,
        &cfg(),
        &BumpSizes {
            spot: 1.0,
            ..BumpSizes::default()
        },
    )
    .unwrap();

    assert_eq!(surf.delta.dim(), (3, 3));
    assert_eq!(surf.gamma.dim(), (3, 3));
    assert_eq!(surf.vega.dim(), (3, 3));
    assert_eq!(surf.price.dim(), (3, 3));
}

#[test]
fn delta_and_vega_surfaces_match_black_scholes() {
    let spots = vec![90.0, 100.0, 110.0];
    let vols = vec![0.15, 0.20, 0.25];
    let (k, r, t) = (100.0, 0.05, 1.0);
    let surf = greek_surface(
        &european_call("AAPL", k, t),
        "AAPL",
        &spots,
        &vols,
        &cfg(),
        &BumpSizes {
            spot: 1.0,
            ..BumpSizes::default()
        },
    )
    .unwrap();

    for (i, &s) in spots.iter().enumerate() {
        for (j, &v) in vols.iter().enumerate() {
            let d_mc = surf.delta[[i, j]];
            let d_bs = bs_delta(s, k, r, v, t);
            assert!(
                (d_mc - d_bs).abs() / d_bs < 0.02,
                "delta({s},{v}) MC={d_mc} BS={d_bs}"
            );

            let v_mc = surf.vega[[i, j]];
            let v_bs = bs_vega(s, k, r, v, t);
            assert!(
                (v_mc - v_bs).abs() / v_bs < 0.03,
                "vega({s},{v}) MC={v_mc} BS={v_bs}"
            );
        }
    }
}

#[test]
fn delta_increases_with_spot() {
    // Monotonie : le delta d'un call croît avec le spot (à vol fixée).
    let spots = vec![80.0, 100.0, 120.0];
    let vols = vec![0.20];
    let surf = greek_surface(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        &spots,
        &vols,
        &cfg(),
        &BumpSizes::default(),
    )
    .unwrap();
    assert!(surf.delta[[0, 0]] < surf.delta[[1, 0]]);
    assert!(surf.delta[[1, 0]] < surf.delta[[2, 0]]);
}

#[test]
fn csv_and_pgm_exports_are_well_formed() {
    let spots = vec![95.0, 105.0];
    let vols = vec![0.18, 0.22];
    let surf = greek_surface(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        &spots,
        &vols,
        &cfg(),
        &BumpSizes::default(),
    )
    .unwrap();

    let csv = surf.to_csv(&surf.delta);
    assert!(csv.starts_with("spot\\vol,0.18,0.22"));
    assert_eq!(csv.lines().count(), 3); // entête + 2 lignes de spots

    let pgm = surf.to_pgm(&surf.delta);
    assert!(pgm.starts_with("P2\n2 2\n255"));
}

#[test]
fn empty_grid_is_rejected() {
    let res = greek_surface(
        &european_call("AAPL", 100.0, 1.0),
        "AAPL",
        &[],
        &[0.2],
        &cfg(),
        &BumpSizes::default(),
    );
    assert!(res.is_err());
}
