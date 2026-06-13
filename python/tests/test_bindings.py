"""Tests du jalon J8 — bindings PyO3.

Critère : `import kontract` puis construire et pricer un contrat depuis Python.
"""

import math

import kontract as k
from kontract import S, one, at, USD, GBM


def bs_call(s, strike, r, sigma, t):
    d1 = (math.log(s / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    nd1 = 0.5 * (1 + math.erf(d1 / math.sqrt(2)))
    nd2 = 0.5 * (1 + math.erf(d2 / math.sqrt(2)))
    return s * nd1 - strike * math.exp(-r * t) * nd2


def test_import_and_version():
    assert isinstance(k.__version__, str)
    assert k.USD == "USD"


def test_fluent_construction():
    call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
    assert isinstance(call, k.Contract)
    # round-trip JSON
    again = k.Contract.from_json(call.to_json())
    assert again.to_json() == call.to_json()


def test_european_call_prices_close_to_black_scholes():
    call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
    model = GBM(s0=100.0, sigma=0.20, r=0.05, asset="AAPL")
    res = call.price(model, n_paths=400_000, seed=2024, steps_per_year=1)
    bs = bs_call(100.0, 100.0, 0.05, 0.20, 1.0)
    assert abs(res.price - bs) / bs < 0.01
    # diagnostics présents
    assert res.std_error > 0.0
    assert res.ci95_low < res.price < res.ci95_high


def test_greeks_from_python():
    call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
    model = GBM(s0=100.0, sigma=0.20, r=0.05, asset="AAPL")
    g = call.greeks(model, n_paths=300_000, seed=7, steps_per_year=1)
    # delta d'un call ATM ~ 0.6 ; bornes larges
    assert 0.0 < g.delta < 1.0
    assert g.vega > 0.0
    assert g.gamma > 0.0


def test_knock_out_via_until():
    call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
    ko = call.until(S("AAPL") >= 150.0)
    model = GBM(s0=100.0, sigma=0.20, r=0.05, asset="AAPL")
    res = ko.price(model, n_paths=100_000, seed=1, steps_per_year=100)
    vanilla = call.price(model, n_paths=100_000, seed=1, steps_per_year=100)
    # un up-and-out vaut moins que la vanille
    assert 0.0 < res.price < vanilla.price


def test_portfolio_via_addition():
    # `+` = and : un portefeuille de deux unités.
    book = one(USD) + one(USD)
    model = GBM(s0=100.0, sigma=0.2, r=0.0, asset="AAPL")
    res = book.price(model, n_paths=1000, seed=1, steps_per_year=1)
    assert abs(res.price - 2.0) < 1e-9
