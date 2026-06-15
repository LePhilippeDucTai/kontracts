//! Tests du jalon J23 — Backtesting & validation de modèle.
//!
//! Critère : « prix historiques vs modèle, stability » → « validation PnL réel ».

use kontract::backtest::{
    backtest_delta_hedge, bs_delta, delta_hedge_error, historical_model_prices, model_vs_market,
    stability,
};
use kontract::numerics::{black_scholes_call, norm_cdf};
use kontract::pricer::McConfig;
use kontract::products::european_call;
use kontract::Gbm;

/// Le delta analytique d'un call ATM est ≈ N(d1) et celui d'un put = delta_call − 1
/// (parité). On vérifie les valeurs de référence + la dégénérescence à l'échéance.
#[test]
fn test_bs_delta_reference_and_parity() {
    let (s, k, tau, r, sigma) = (100.0, 100.0, 1.0, 0.05, 0.2);
    let d_call = bs_delta(s, k, tau, r, sigma, true);
    let d_put = bs_delta(s, k, tau, r, sigma, false);

    // Référence analytique : N(d1).
    let d1 = ((s / k).ln() + (r + 0.5 * sigma * sigma) * tau) / (sigma * tau.sqrt());
    assert!((d_call - norm_cdf(d1)).abs() < 1e-12);

    // Parité call/put : Δ_call − Δ_put = 1.
    assert!((d_call - d_put - 1.0).abs() < 1e-12);

    // Bornes : Δ_call ∈ (0,1), Δ_put ∈ (−1,0).
    assert!(d_call > 0.0 && d_call < 1.0);
    assert!(d_put > -1.0 && d_put < 0.0);

    // À l'échéance : indicatrice ITM.
    assert_eq!(bs_delta(120.0, 100.0, 0.0, r, sigma, true), 1.0);
    assert_eq!(bs_delta(80.0, 100.0, 0.0, r, sigma, true), 0.0);
    assert_eq!(bs_delta(80.0, 100.0, 0.0, r, sigma, false), -1.0);
}

/// Cas dégénéré utile : une trajectoire **plate** `S_t = S_0` constante. La
/// couverture en delta ne fait que faire fructifier le cash ; l'erreur de
/// réplication doit être petite (de l'ordre du discount résiduel sur l'écart
/// prime/payoff), et déterministe.
#[test]
fn test_delta_hedge_flat_path() {
    let n = 51;
    let maturity = 1.0;
    let dt = maturity / (n - 1) as f64;
    let spots = vec![100.0; n]; // trajectoire plate à l'ATM

    let err = delta_hedge_error(&spots, dt, 100.0, 0.05, 0.2, true).expect("hedge ok");
    // Pas de mouvement → le PnL reste borné par la prime initiale, et fini.
    assert!(err.is_finite());
    assert!(err.abs() < 15.0, "erreur plate {err} hors de portée");
}

/// **Validation par PnL réalisé** (cœur du jalon) : on couvre en delta le long de
/// trajectoires tirées par le simulateur GBM du moteur, au même σ. Le PnL de
/// réplication moyen doit valoir ≈ 0 (couverture cohérente avec le modèle qui
/// génère la réalité), à l'erreur Monte-Carlo près.
#[test]
fn test_delta_hedge_mean_pnl_near_zero() {
    let model = Gbm::new("underlying", 100.0, 0.03, 0.25);
    let report =
        backtest_delta_hedge(&model, 100.0, 1.0, true, 100, 20_000, 7).expect("backtest ok");

    // Le PnL moyen est négligeable devant la prime.
    let rel = report.mean_pnl.abs() / report.premium;
    assert!(
        rel < 0.03,
        "PnL moyen {:.4} trop grand vs prime {:.4} (rel {:.4})",
        report.mean_pnl,
        report.premium,
        rel
    );
    assert!(report.std_pnl > 0.0);
    assert_eq!(report.n_paths, 20_000);
    assert_eq!(report.n_rebalances, 100);
}

/// La couverture discrète laisse une erreur de variance ∝ `dt` : doubler (et plus)
/// la fréquence de rééquilibrage doit **réduire** l'écart-type du PnL.
#[test]
fn test_delta_hedge_error_shrinks_with_frequency() {
    let model = Gbm::new("underlying", 100.0, 0.05, 0.2);

    let coarse = backtest_delta_hedge(&model, 100.0, 1.0, true, 12, 8_000, 11).expect("coarse");
    let fine = backtest_delta_hedge(&model, 100.0, 1.0, true, 200, 8_000, 11).expect("fine");

    assert!(
        fine.std_pnl < coarse.std_pnl,
        "rééquilibrer plus souvent doit réduire l'écart-type : fine {:.4} vs coarse {:.4}",
        fine.std_pnl,
        coarse.std_pnl
    );
}

/// `model_vs_market` : sur des séries identiques l'erreur est nulle ; sur un
/// décalage constant le biais et la RMSE valent ce décalage.
#[test]
fn test_model_vs_market_stats() {
    let market = vec![10.0, 12.0, 8.0, 15.0];

    let exact = model_vs_market(&market, &market).expect("exact");
    assert!(exact.bias.abs() < 1e-12);
    assert!(exact.rmse < 1e-12);
    assert!(exact.max_abs_error < 1e-12);
    assert!(exact.mean_relative_error < 1e-12);

    // Modèle systématiquement +0.5 au-dessus du marché.
    let model: Vec<f64> = market.iter().map(|q| q + 0.5).collect();
    let report = model_vs_market(&model, &market).expect("report");
    assert!((report.bias - 0.5).abs() < 1e-12);
    assert!((report.rmse - 0.5).abs() < 1e-12);
    assert!((report.mean_abs_error - 0.5).abs() < 1e-12);
    assert!((report.max_abs_error - 0.5).abs() < 1e-12);

    // Longueurs incompatibles → erreur.
    assert!(model_vs_market(&[1.0, 2.0], &[1.0]).is_err());
}

/// `stability` : série constante → variations nulles ; tendance linéaire → dérive
/// constante et volatilité des variations nulle.
#[test]
fn test_stability_metrics() {
    let flat = vec![5.0; 6];
    let s = stability(&flat).expect("flat");
    assert!(s.value_std < 1e-12);
    assert!(s.change_std < 1e-12);
    assert!(s.max_abs_change < 1e-12);
    assert!((s.value_mean - 5.0).abs() < 1e-12);

    // Rampe linéaire +2 par pas : dérive 2, pas de volatilité de variation.
    let ramp: Vec<f64> = (0..5).map(|i| (i as f64) * 2.0).collect();
    let r = stability(&ramp).expect("ramp");
    assert!((r.change_mean - 2.0).abs() < 1e-12);
    assert!(r.change_std < 1e-12);
    assert!((r.max_abs_change - 2.0).abs() < 1e-12);

    // Série trop courte → erreur.
    assert!(stability(&[1.0]).is_err());
}

/// Bout-à-bout « prix historiques vs modèle » : on rejoue le **moteur MC** sur une
/// séquence d'observations (spot constant, maturité résiduelle décroissante) et on
/// compare à la référence Black-Scholes analytique. L'écart doit rester dans la
/// tolérance MC (< 1 %), et la série de prix décroît à mesure que la maturité fond.
#[test]
fn test_historical_replay_vs_black_scholes() {
    let spot = 100.0;
    let strike = 100.0;
    let rate = 0.05;
    let sigma = 0.2;
    let maturities = [1.0, 0.75, 0.5, 0.25, 0.05];

    let cfg = McConfig {
        n_paths: 50_000,
        seed: 123,
        steps_per_year: 1,
        rate,
        variance_reduction: None,
    };

    let model_prices = historical_model_prices(
        maturities.len(),
        |i| {
            let t = maturities[i];
            let contract = european_call("underlying", strike, t, "USD");
            (contract, Gbm::new("underlying", spot, rate, sigma))
        },
        &cfg,
    )
    .expect("replay ok");

    let bs_prices: Vec<f64> = maturities
        .iter()
        .map(|&t| black_scholes_call(spot, strike, t, rate, sigma))
        .collect();

    let report = model_vs_market(&model_prices, &bs_prices).expect("report");
    assert!(
        report.mean_relative_error < 0.01,
        "moteur vs BS : erreur relative {:.4} > 1 %",
        report.mean_relative_error
    );

    // La valeur temps décroît avec la maturité résiduelle (call ATM).
    assert!(
        model_prices.windows(2).all(|w| w[1] < w[0] + 0.05),
        "le prix du call ATM doit décroître quand la maturité fond : {model_prices:?}"
    );
}
