"""kontract — algèbre des contrats financiers (cœur Rust).

API fluide exposée depuis le module natif `_kontract` :

    from kontract import S, one, at, USD, GBM
    call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
    res = call.price(GBM(s0=100, sigma=0.2, r=0.05), n_paths=200_000)
    print(res.price, res.std_error)

Modèles avancés, taux stochastiques, FX, produits et calibration sont aussi
exposés (cf. `__all__`).
"""

from ._kontract import (  # module natif Rust
    __version__,
    # Algèbre / pricing de base
    Observable,
    Condition,
    Contract,
    GBM,
    Model,
    GbmFactor,
    RateModel,
    Swaption,
    PriceResult,
    Greeks,
    zero,
    one,
    give,
    S,
    spot,
    const_,
    at,
    USD,
    EUR,
    GBP,
    JPY,
    # Observables temporels (J26) + basket corrélé (J27)
    average,
    average_over,
    running_max,
    running_min,
    correlated_gbm,
    # Modèles (J12–J16)
    heston,
    sabr,
    merton,
    rough_bergomi,
    sobol_gbm,
    # Taux stochastiques (J24)
    vasicek,
    hull_white,
    swaption_mc,
    vasicek_swaption_analytic,
    # Catalogue de produits (J9)
    zero_coupon_bond,
    european_call,
    european_put,
    forward,
    straddle,
    bull_call_spread,
    cash_or_nothing_call,
    up_and_out_call,
    down_and_out_call,
    # FX (J25)
    garman_kohlhagen_call,
    garman_kohlhagen_put,
    fx_forward,
    quanto_call,
    # Calibration / données de marché
    implied_volatility,
    fit_gbm_volatility,
)

__all__ = [
    "__version__",
    "Observable",
    "Condition",
    "Contract",
    "GBM",
    "Model",
    "GbmFactor",
    "RateModel",
    "Swaption",
    "PriceResult",
    "Greeks",
    "zero",
    "one",
    "give",
    "S",
    "spot",
    "const_",
    "at",
    "USD",
    "EUR",
    "GBP",
    "JPY",
    "average",
    "average_over",
    "running_max",
    "running_min",
    "correlated_gbm",
    "heston",
    "sabr",
    "merton",
    "rough_bergomi",
    "sobol_gbm",
    "vasicek",
    "hull_white",
    "swaption_mc",
    "vasicek_swaption_analytic",
    "zero_coupon_bond",
    "european_call",
    "european_put",
    "forward",
    "straddle",
    "bull_call_spread",
    "cash_or_nothing_call",
    "up_and_out_call",
    "down_and_out_call",
    "garman_kohlhagen_call",
    "garman_kohlhagen_put",
    "fx_forward",
    "quanto_call",
    "implied_volatility",
    "fit_gbm_volatility",
]
