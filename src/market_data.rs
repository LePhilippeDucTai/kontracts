//! Market data reader and implied volatility surface (jalon J21).
//!
//! Ingests option prices (strike, maturity, bid/mid/ask) and computes implied vol surface.
//! Exports to JSON for downstream calibration (J22).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::KontractError;

/// Single option quote: strike, maturity, prices (bid/mid/ask).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OptionQuote {
    pub strike: f64,
    pub maturity: f64,  // Time to expiration in years
    pub mid_price: f64, // Mid price
    pub bid_price: Option<f64>,
    pub ask_price: Option<f64>,
}

/// Implied volatility surface: map (K, T) -> σ_implied
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilitySurface {
    pub spot: f64,
    pub rate: f64, // Risk-free rate
    pub dividend_yield: f64,
    /// Map (strike, maturity) -> implied_vol
    pub surface: BTreeMap<(OrderedFloat, OrderedFloat), f64>,
    pub strikes: Vec<f64>,
    pub maturities: Vec<f64>,
}

/// Wrapper for f64 to make it Ord (for BTreeMap keys).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OrderedFloat(f64);

impl Eq for OrderedFloat {}

impl PartialOrd for OrderedFloat {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrderedFloat {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .partial_cmp(&other.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    }
}

impl VolatilitySurface {
    /// Create a new volatility surface.
    pub fn new(spot: f64, rate: f64, dividend_yield: f64) -> Self {
        VolatilitySurface {
            spot,
            rate,
            dividend_yield,
            surface: BTreeMap::new(),
            strikes: vec![],
            maturities: vec![],
        }
    }

    /// Add an implied volatility point to the surface.
    pub fn add_point(&mut self, strike: f64, maturity: f64, implied_vol: f64) {
        self.surface
            .insert((OrderedFloat(strike), OrderedFloat(maturity)), implied_vol);

        // Maintain sorted unique strikes and maturities.
        if !self.strikes.contains(&strike) {
            self.strikes.push(strike);
            self.strikes
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
        if !self.maturities.contains(&maturity) {
            self.maturities.push(maturity);
            self.maturities
                .sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        }
    }

    /// Get implied vol at (strike, maturity) via bilinear interpolation.
    pub fn get_vol(&self, strike: f64, maturity: f64) -> Option<f64> {
        if self.surface.is_empty() {
            return None;
        }

        // Exact match.
        if let Some(&vol) = self
            .surface
            .get(&(OrderedFloat(strike), OrderedFloat(maturity)))
        {
            return Some(vol);
        }

        // Bilinear interpolation on nearest 4 points.
        let k_idx = self
            .strikes
            .binary_search_by(|x| x.partial_cmp(&strike).unwrap_or(std::cmp::Ordering::Equal));
        let t_idx = self.maturities.binary_search_by(|x| {
            x.partial_cmp(&maturity)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let (k_lo, k_hi) = match k_idx {
            Ok(i) => (i, i),
            Err(i) => {
                if i == 0 || i >= self.strikes.len() {
                    return None; // Out of bounds
                }
                (i - 1, i)
            }
        };

        let (t_lo, t_hi) = match t_idx {
            Ok(i) => (i, i),
            Err(i) => {
                if i == 0 || i >= self.maturities.len() {
                    return None; // Out of bounds
                }
                (i - 1, i)
            }
        };

        let k0 = self.strikes[k_lo];
        let k1 = self.strikes[k_hi];
        let t0 = self.maturities[t_lo];
        let t1 = self.maturities[t_hi];

        let v00 = self.surface.get(&(OrderedFloat(k0), OrderedFloat(t0)))?;
        let v10 = self.surface.get(&(OrderedFloat(k1), OrderedFloat(t0)))?;
        let v01 = self.surface.get(&(OrderedFloat(k0), OrderedFloat(t1)))?;
        let v11 = self.surface.get(&(OrderedFloat(k1), OrderedFloat(t1)))?;

        // Bilinear interpolation.
        let wk = if k1 != k0 {
            (strike - k0) / (k1 - k0)
        } else {
            0.0
        };
        let wt = if t1 != t0 {
            (maturity - t0) / (t1 - t0)
        } else {
            0.0
        };

        let v = (1.0 - wk) * (1.0 - wt) * v00
            + wk * (1.0 - wt) * v10
            + (1.0 - wk) * wt * v01
            + wk * wt * v11;

        Some(v)
    }
}

/// Compute implied volatility via Black-Scholes inversion (bisection method).
/// `call_price`: observed call price
/// `spot`: spot price
/// `strike`: strike
/// `maturity`: time to expiration (years)
/// `rate`: risk-free rate
/// `dividend_yield`: dividend yield
pub fn implied_volatility(
    call_price: f64,
    spot: f64,
    strike: f64,
    maturity: f64,
    rate: f64,
    dividend_yield: f64,
) -> Result<f64, KontractError> {
    // Bounds: vol in [0.001, 5.0]
    let mut vol_low = 0.001;
    let mut vol_high = 5.0;

    // Intrinsic value: lower bound on call price.
    let intrinsic =
        (spot * (-dividend_yield * maturity).exp() - strike * (-rate * maturity).exp()).max(0.0);
    if call_price < intrinsic - 1e-10 {
        return Err(KontractError::InconsistentPath(
            "call_price below intrinsic value".into(),
        ));
    }

    // Bisection: find vol such that bs_call(vol) ≈ call_price.
    for _ in 0..100 {
        let vol_mid = (vol_low + vol_high) / 2.0;
        let price_mid =
            black_scholes_call_simple(spot, strike, maturity, rate, dividend_yield, vol_mid);

        if (price_mid - call_price).abs() < 1e-6 {
            return Ok(vol_mid);
        }

        if price_mid < call_price {
            vol_low = vol_mid;
        } else {
            vol_high = vol_mid;
        }
    }

    Ok((vol_low + vol_high) / 2.0)
}

/// Simple Black-Scholes call price (for IV inversion).
fn black_scholes_call_simple(
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

    let nd1 = norm_cdf(d1);
    let nd2 = norm_cdf(d2);

    spot * (-dividend_yield * maturity).exp() * nd1 - strike * (-rate * maturity).exp() * nd2
}

/// Standard normal CDF (Abramowitz-Stegun approximation).
fn norm_cdf(x: f64) -> f64 {
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

/// Load option quotes from a simple CSV (strike, maturity, mid_price, [bid], [ask]).
pub fn load_csv(csv_text: &str) -> Result<Vec<OptionQuote>, KontractError> {
    let mut quotes = vec![];

    for line in csv_text.lines().skip(1) {
        // Skip header and empty lines.
        if line.trim().is_empty() {
            continue;
        }

        let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
        if parts.len() < 3 {
            continue;
        }

        let strike: f64 = parts[0]
            .parse()
            .map_err(|_| KontractError::InconsistentPath("Failed to parse strike".into()))?;
        let maturity: f64 = parts[1]
            .parse()
            .map_err(|_| KontractError::InconsistentPath("Failed to parse maturity".into()))?;
        let mid_price: f64 = parts[2]
            .parse()
            .map_err(|_| KontractError::InconsistentPath("Failed to parse mid_price".into()))?;

        let bid_price = if parts.len() > 3 {
            parts[3].parse().ok()
        } else {
            None
        };

        let ask_price = if parts.len() > 4 {
            parts[4].parse().ok()
        } else {
            None
        };

        quotes.push(OptionQuote {
            strike,
            maturity,
            mid_price,
            bid_price,
            ask_price,
        });
    }

    Ok(quotes)
}

/// Build a volatility surface from option quotes.
pub fn build_surface(
    quotes: Vec<OptionQuote>,
    spot: f64,
    rate: f64,
    dividend_yield: f64,
) -> Result<VolatilitySurface, KontractError> {
    let mut surface = VolatilitySurface::new(spot, rate, dividend_yield);

    for quote in quotes {
        let iv = implied_volatility(
            quote.mid_price,
            spot,
            quote.strike,
            quote.maturity,
            rate,
            dividend_yield,
        )?;
        surface.add_point(quote.strike, quote.maturity, iv);
    }

    Ok(surface)
}
