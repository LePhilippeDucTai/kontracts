use kontract::{build_surface, implied_volatility, load_csv, OptionQuote, VolatilitySurface};

#[test]
fn test_implied_volatility_atm() {
    // European call at-the-money with known parameters.
    let spot = 100.0;
    let strike = 100.0;
    let maturity = 1.0;
    let rate = 0.05;
    let dividend_yield = 0.0;
    let vol_true = 0.20;

    // Compute call price under true vol.
    let call_price =
        black_scholes_call_test(spot, strike, maturity, rate, dividend_yield, vol_true);

    // Invert to recover vol.
    let vol_recovered =
        implied_volatility(call_price, spot, strike, maturity, rate, dividend_yield)
            .expect("IV computation failed");

    // Should recover the original vol within tolerance.
    assert!(
        (vol_recovered - vol_true).abs() < 1e-2,
        "IV recovery failed: expected {}, got {}",
        vol_true,
        vol_recovered
    );
}

#[test]
fn test_implied_volatility_itm() {
    // In-the-money call.
    let spot = 110.0;
    let strike = 100.0;
    let maturity = 0.5;
    let rate = 0.05;
    let dividend_yield = 0.0;
    let vol_true = 0.25;

    let call_price =
        black_scholes_call_test(spot, strike, maturity, rate, dividend_yield, vol_true);
    let vol_recovered =
        implied_volatility(call_price, spot, strike, maturity, rate, dividend_yield)
            .expect("IV computation failed");

    assert!((vol_recovered - vol_true).abs() < 1e-2);
}

#[test]
fn test_implied_volatility_otm() {
    // Out-of-the-money call.
    let spot = 90.0;
    let strike = 100.0;
    let maturity = 0.5;
    let rate = 0.05;
    let dividend_yield = 0.0;
    let vol_true = 0.30;

    let call_price =
        black_scholes_call_test(spot, strike, maturity, rate, dividend_yield, vol_true);
    let vol_recovered =
        implied_volatility(call_price, spot, strike, maturity, rate, dividend_yield)
            .expect("IV computation failed");

    assert!((vol_recovered - vol_true).abs() < 1e-2);
}

#[test]
fn test_volatility_surface_creation() {
    let mut surface = VolatilitySurface::new(100.0, 0.05, 0.0);
    assert!(surface.surface.is_empty());

    surface.add_point(100.0, 1.0, 0.20);
    surface.add_point(110.0, 1.0, 0.18);
    surface.add_point(100.0, 0.5, 0.22);

    assert_eq!(surface.surface.len(), 3);
    assert_eq!(surface.strikes.len(), 2);
    assert_eq!(surface.maturities.len(), 2);
}

#[test]
fn test_volatility_surface_exact_match() {
    let mut surface = VolatilitySurface::new(100.0, 0.05, 0.0);
    surface.add_point(100.0, 1.0, 0.20);
    surface.add_point(110.0, 1.0, 0.18);
    surface.add_point(100.0, 0.5, 0.22);

    // Exact matches.
    assert_eq!(surface.get_vol(100.0, 1.0), Some(0.20));
    assert_eq!(surface.get_vol(110.0, 1.0), Some(0.18));
    assert_eq!(surface.get_vol(100.0, 0.5), Some(0.22));
}

#[test]
fn test_volatility_surface_interpolation() {
    let mut surface = VolatilitySurface::new(100.0, 0.05, 0.0);
    surface.add_point(100.0, 1.0, 0.20);
    surface.add_point(110.0, 1.0, 0.18);
    surface.add_point(100.0, 0.5, 0.22);
    surface.add_point(110.0, 0.5, 0.24);

    // Interpolated point: (105, 0.75) should be between the 4 corners.
    let vol_interp = surface.get_vol(105.0, 0.75);
    assert!(vol_interp.is_some());
    let vol = vol_interp.unwrap();

    // Should be roughly the average of the 4 corners (bilinear).
    let expected = (0.20 + 0.18 + 0.22 + 0.24) / 4.0;
    assert!(
        (vol - expected).abs() < 0.01,
        "Interpolation expected ~{}, got {}",
        expected,
        vol
    );
}

#[test]
fn test_volatility_surface_out_of_bounds() {
    let mut surface = VolatilitySurface::new(100.0, 0.05, 0.0);
    surface.add_point(100.0, 1.0, 0.20);

    // Out of bounds.
    assert_eq!(surface.get_vol(200.0, 1.0), None);
    assert_eq!(surface.get_vol(100.0, 2.0), None);
}

#[test]
fn test_csv_loading() {
    let csv = "strike,maturity,mid_price\n\
               100.0,1.0,10.5\n\
               110.0,1.0,3.2\n\
               100.0,0.5,5.3";

    let quotes = load_csv(csv).expect("CSV loading failed");
    assert_eq!(quotes.len(), 3);

    assert_eq!(quotes[0].strike, 100.0);
    assert_eq!(quotes[0].maturity, 1.0);
    assert_eq!(quotes[0].mid_price, 10.5);
}

#[test]
fn test_csv_loading_with_bid_ask() {
    let csv = "strike,maturity,mid_price,bid,ask\n\
               100.0,1.0,10.5,10.2,10.8\n\
               110.0,1.0,3.2,3.0,3.4";

    let quotes = load_csv(csv).expect("CSV loading failed");
    assert_eq!(quotes.len(), 2);

    assert_eq!(quotes[0].bid_price, Some(10.2));
    assert_eq!(quotes[0].ask_price, Some(10.8));
}

#[test]
fn test_round_trip_surface() {
    // Generate a synthetic surface with known parameters.
    let spot = 100.0;
    let rate = 0.05;
    let dividend_yield = 0.01;

    // Create synthetic option quotes with known vols.
    let mut quotes = vec![];
    for &strike in &[90.0, 100.0, 110.0] {
        for &maturity in &[0.25, 0.5, 1.0] {
            // Vary vol slightly by strike/maturity (smile + term structure).
            let smile_factor = if strike < 100.0 {
                1.1
            } else if strike > 100.0 {
                1.2
            } else {
                1.0
            };
            let term_factor = if maturity < 0.5 { 1.05 } else { 1.0 };
            let vol_true = 0.20 * smile_factor * term_factor;

            let call_price =
                black_scholes_call_test(spot, strike, maturity, rate, dividend_yield, vol_true);

            quotes.push(OptionQuote {
                strike,
                maturity,
                mid_price: call_price,
                bid_price: Some(call_price * 0.99),
                ask_price: Some(call_price * 1.01),
            });
        }
    }

    // Build surface.
    let surface =
        build_surface(quotes.clone(), spot, rate, dividend_yield).expect("Surface building failed");

    // Verify we can recover the vols (at exact points).
    for quote in &quotes {
        let vol_recovered = surface.get_vol(quote.strike, quote.maturity);
        assert!(
            vol_recovered.is_some(),
            "Failed to recover vol at strike={}, maturity={}",
            quote.strike,
            quote.maturity
        );

        let vol = vol_recovered.unwrap();
        let vol_implied = implied_volatility(
            quote.mid_price,
            spot,
            quote.strike,
            quote.maturity,
            rate,
            dividend_yield,
        )
        .expect("IV computation failed");

        assert!(
            (vol - vol_implied).abs() < 1e-4,
            "Surface vol mismatch: expected {}, got {}",
            vol_implied,
            vol
        );
    }
}

#[test]
fn test_surface_preserves_skew() {
    // Build a surface with a volatility smile.
    let mut surface = VolatilitySurface::new(100.0, 0.05, 0.0);

    // Add a smile: ATM lower vol, OTM higher vol.
    surface.add_point(90.0, 1.0, 0.25);
    surface.add_point(100.0, 1.0, 0.20);
    surface.add_point(110.0, 1.0, 0.25);

    // Verify: wings > center.
    let vol_wing = surface.get_vol(90.0, 1.0).unwrap();
    let vol_atm = surface.get_vol(100.0, 1.0).unwrap();

    assert!(
        vol_wing > vol_atm,
        "Smile structure lost: wing {} should be > ATM {}",
        vol_wing,
        vol_atm
    );
}

#[test]
fn test_surface_term_structure() {
    // Build a surface with term structure: longer maturities have lower vol.
    let mut surface = VolatilitySurface::new(100.0, 0.05, 0.0);

    surface.add_point(100.0, 0.25, 0.30);
    surface.add_point(100.0, 0.5, 0.25);
    surface.add_point(100.0, 1.0, 0.20);

    let vol_short = surface.get_vol(100.0, 0.25).unwrap();
    let vol_long = surface.get_vol(100.0, 1.0).unwrap();

    assert!(
        vol_short > vol_long,
        "Term structure lost: short {} should be > long {}",
        vol_short,
        vol_long
    );
}

// Helper: compute BS call price.
fn black_scholes_call_test(
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    dividend_yield: f64,
    vol: f64,
) -> f64 {
    if vol <= 0.0 {
        return (spot * (-dividend_yield * maturity).exp() - strike * (-rate * maturity).exp())
            .max(0.0);
    }

    let d1 = ((spot / strike).ln() + (rate - dividend_yield + 0.5 * vol * vol) * maturity)
        / (vol * maturity.sqrt());
    let d2 = d1 - vol * maturity.sqrt();

    let nd1 = norm_cdf_test(d1);
    let nd2 = norm_cdf_test(d2);

    spot * (-dividend_yield * maturity).exp() * nd1 - strike * (-rate * maturity).exp() * nd2
}

fn norm_cdf_test(x: f64) -> f64 {
    const A1: f64 = 0.254829592;
    const A2: f64 = -0.284496736;
    const A3: f64 = 1.421413741;
    const A4: f64 = -1.453152027;
    const A5: f64 = 1.061405429;
    const P: f64 = 0.3275911;

    let sign = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let t = 1.0 / (1.0 + P * x);
    let y = 1.0 - (((((A5 * t + A4) * t + A3) * t + A2) * t + A1) * t * (-x * x).exp());

    (1.0 + sign * y) / 2.0
}
