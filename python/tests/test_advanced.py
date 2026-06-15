"""Tests des bindings étendus : modèles, produits, taux, FX, calibration."""

import math

import kontract as k


def bs_call(s, kk, r, sigma, t):
    from math import erf, exp, log, sqrt

    d1 = (log(s / kk) + (r + 0.5 * sigma * sigma) * t) / (sigma * sqrt(t))
    d2 = d1 - sigma * sqrt(t)
    cdf = lambda x: 0.5 * (1.0 + erf(x / sqrt(2.0)))
    return s * cdf(d1) - kk * exp(-r * t) * cdf(d2)


def test_products_catalogue():
    call = k.european_call("AAPL", 100.0, 1.0, k.USD)
    model = k.GBM(s0=100.0, sigma=0.2, r=0.05, asset="AAPL")
    res = call.price(model, n_paths=200_000, seed=1, steps_per_year=1)
    assert abs(res.price - bs_call(100.0, 100.0, 0.05, 0.2, 1.0)) < 0.05


def test_dividend_yield_lowers_call():
    call = k.european_call("S", 100.0, 1.0, k.USD)
    no_div = k.GBM(s0=100.0, sigma=0.2, r=0.05, q=0.0, asset="S")
    with_div = k.GBM(s0=100.0, sigma=0.2, r=0.05, q=0.05, asset="S")
    p0 = call.price(no_div, n_paths=200_000, seed=3, steps_per_year=1).price
    p1 = call.price(with_div, n_paths=200_000, seed=3, steps_per_year=1).price
    assert p1 < p0  # dividend yield reduces the forward → cheaper call


def test_heston_model_prices():
    call = k.european_call("S", 100.0, 1.0, k.USD)
    model = k.heston(
        spot=100.0, v0=0.04, kappa=2.0, theta=0.04, sigma_v=0.3, rho=-0.5, r=0.05, asset="S"
    )
    res = call.price(model, n_paths=100_000, seed=7, steps_per_year=100)
    # Heston ATM with these params is in the BS(0.2) neighbourhood.
    assert abs(res.price - bs_call(100.0, 100.0, 0.05, 0.2, 1.0)) < 1.5


def test_variance_reduction_shrinks_error():
    call = k.european_call("S", 100.0, 1.0, k.USD)
    model = k.GBM(s0=100.0, sigma=0.2, r=0.05, asset="S")
    plain = call.price(model, n_paths=40_000, seed=11, steps_per_year=1)
    anti = call.price(model, n_paths=40_000, seed=11, steps_per_year=1, antithetic=True)
    assert anti.std_error < plain.std_error


def test_american_put_above_european():
    payoff = (k.const_(100.0) - k.S("S")).clip(0.0) * k.one(k.USD)
    eu = (payoff) @ k.at(1.0)
    model = k.GBM(s0=90.0, sigma=0.3, r=0.08, asset="S")
    eu_price = eu.price(model, n_paths=80_000, seed=5, steps_per_year=50).price
    dates = [i / 50.0 for i in range(1, 51)]
    us_price = payoff.price_american(model, dates, n_paths=80_000, seed=5).price
    assert us_price >= eu_price - 0.05


def test_garman_kohlhagen_parity():
    c = k.garman_kohlhagen_call(1.2, 1.25, 1.0, 0.04, 0.01, 0.1)
    p = k.garman_kohlhagen_put(1.2, 1.25, 1.0, 0.04, 0.01, 0.1)
    parity = 1.2 * math.exp(-0.01) - 1.25 * math.exp(-0.04)
    assert abs((c - p) - parity) < 1e-9
    assert abs(k.fx_forward(1.2, 1.0, 0.04, 0.01) - 1.2 * math.exp(0.03)) < 1e-12


def test_quanto_monotonic_in_rho():
    neg = k.quanto_call(100.0, 100.0, 1.0, 0.04, 0.02, 0.0, 0.25, 0.15, -0.5)
    pos = k.quanto_call(100.0, 100.0, 1.0, 0.04, 0.02, 0.0, 0.25, 0.15, 0.5)
    assert neg > pos


def test_zero_coupon_under_stochastic_rates():
    zcb = k.zero_coupon_bond(k.USD, 2.0)
    model = k.vasicek(r0=0.03, a=0.6, b=0.05, sigma=0.015)
    res = zcb.price_under_rates(model, n_paths=150_000, seed=11, steps_per_year=100)
    analytic = model.discount_bond0(2.0)
    assert abs(res.price - analytic) < 3e-3


def test_swaption_mc_vs_analytic():
    model = k.vasicek(r0=0.04, a=0.5, b=0.05, sigma=0.012)
    sw = k.Swaption.level(1.0, 0.5, 4, 0.05, True)
    mc = k.swaption_mc(model, sw, n_paths=200_000, seed=21, steps=120).price
    analytic = k.vasicek_swaption_analytic(0.04, 0.5, 0.05, 0.012, sw)
    assert abs(mc - analytic) / analytic < 0.04


def test_implied_vol_roundtrip():
    price = bs_call(100.0, 100.0, 0.05, 0.25, 1.0)
    iv = k.implied_volatility(price, 100.0, 100.0, 1.0, 0.05, 0.0)
    assert abs(iv - 0.25) < 1e-3


def test_fit_gbm_volatility():
    call = k.european_call("S", 100.0, 1.0, k.USD)
    true_vol = 0.22
    price = bs_call(100.0, 100.0, 0.05, true_vol, 1.0)
    fitted = k.fit_gbm_volatility(call, [1.0], [(100.0, price)], 0.05, n_paths=5000)
    assert abs(fitted - true_vol) < 0.05
