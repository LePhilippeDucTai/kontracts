"""Tests des bindings J29 : observables temporels (J26) et basket corrélé (J27).

Couvre l'API Python exposant `average`, `running_max`, `running_min`
(Asian / lookback) et `correlated_gbm` / `GbmFactor` (basket multi-actifs).
"""

from math import erf, exp, log, sqrt

import kontract as k


def bs_call(s, kk, r, sigma, t):
    d1 = (log(s / kk) + (r + 0.5 * sigma * sigma) * t) / (sigma * sqrt(t))
    d2 = d1 - sigma * sqrt(t)
    cdf = lambda x: 0.5 * (1.0 + erf(x / sqrt(2.0)))
    return s * cdf(d1) - kk * exp(-r * t) * cdf(d2)


S0, K, R, SIGMA, T = 100.0, 100.0, 0.05, 0.20, 1.0


def _gbm():
    return k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")


def test_asian_call_below_vanilla():
    """Asian call (moyenne) < vanilla call (inégalité de Jensen), et > 0."""
    model = _gbm()
    asian = ((k.average(k.S("X")) - K).clip(0.0) * k.one(k.USD)) @ k.at(T)
    vanilla = k.european_call("X", K, T, k.USD)

    a = asian.price(model, n_paths=120_000, seed=7, steps_per_year=50).price
    v = vanilla.price(model, n_paths=120_000, seed=7, steps_per_year=50).price

    assert a > 0.0
    assert a < v, f"asian {a:.4f} doit être < vanilla {v:.4f} (Jensen)"


def test_average_pipeline_method_matches_function():
    """`S("X").average()` ≡ `average(S("X"))` (même AST → même prix)."""
    model = _gbm()
    via_fn = ((k.average(k.S("X")) - K).clip(0.0) * k.one(k.USD)) @ k.at(T)
    via_method = ((k.S("X").average() - K).clip(0.0) * k.one(k.USD)) @ k.at(T)
    assert via_fn.to_json() == via_method.to_json()


def test_lookback_call_above_vanilla():
    """Lookback call (running_max) ≥ vanilla call (dominance du max sur S_T)."""
    model = _gbm()
    lookback = ((k.running_max(k.S("X")) - K).clip(0.0) * k.one(k.USD)) @ k.at(T)
    vanilla = k.european_call("X", K, T, k.USD)

    lb = lookback.price(model, n_paths=120_000, seed=9, steps_per_year=50).price
    v = vanilla.price(model, n_paths=120_000, seed=9, steps_per_year=50).price

    assert lb > v, f"lookback {lb:.4f} doit dépasser vanilla {v:.4f}"


def test_running_min_below_running_max():
    """Le minimum courant reste inférieur au maximum courant sur la trajectoire."""
    model = _gbm()
    # Floor call sur le min (toujours moins cher qu'un call sur le max).
    on_min = ((k.running_min(k.S("X")) - 80.0).clip(0.0) * k.one(k.USD)) @ k.at(T)
    on_max = ((k.running_max(k.S("X")) - 80.0).clip(0.0) * k.one(k.USD)) @ k.at(T)
    pmin = on_min.price(model, n_paths=80_000, seed=4, steps_per_year=50).price
    pmax = on_max.price(model, n_paths=80_000, seed=4, steps_per_year=50).price
    assert pmin < pmax


def test_basket_call_vs_black_scholes():
    """Basket equal-weight 3 actifs vs BS(σ_basket), σ_basket = σ√((1+2ρ)/3)."""
    rho = 0.5
    factors = [
        k.GbmFactor("S1", S0, R, SIGMA),
        k.GbmFactor("S2", S0, R, SIGMA),
        k.GbmFactor("S3", S0, R, SIGMA),
    ]
    corr = [[1.0, rho, rho], [rho, 1.0, rho], [rho, rho, 1.0]]
    model = k.correlated_gbm(factors, corr, r=R)

    basket = (k.S("S1") + k.S("S2") + k.S("S3")) / 3.0
    contract = ((basket - K).clip(0.0) * k.one(k.USD)) @ k.at(T)
    price = contract.price(model, n_paths=200_000, seed=7, steps_per_year=50).price

    sigma_basket = SIGMA * sqrt((1.0 + 2.0 * rho) / 3.0)
    analytic = bs_call(S0, K, R, sigma_basket, T)
    rel = abs(price - analytic) / analytic
    assert rel < 0.04, f"basket {price:.4f} vs BS {analytic:.4f} (rel {rel:.4f})"


def test_correlated_gbm_constructor_validates():
    """Matrice de mauvaise taille → exception (validation du constructeur Rust)."""
    factors = [k.GbmFactor("S1", S0, R, SIGMA), k.GbmFactor("S2", S0, R, SIGMA)]
    try:
        k.correlated_gbm(factors, [[1.0, 0.5, 0.0]], r=R)
    except (ValueError, RuntimeError):
        return
    raise AssertionError("matrice non 2×2 aurait dû lever une exception")
