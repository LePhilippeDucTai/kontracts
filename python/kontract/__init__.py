"""kontract — algèbre des contrats financiers (cœur Rust).

API fluide exposée depuis le module natif `_kontract` (jalon J8) :

    from kontract import S, one, at, USD, GBM
    call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
    res = call.price(GBM(s0=100, sigma=0.2, r=0.05), n_paths=200_000)
    print(res.price, res.std_error)
"""

from ._kontract import (  # module natif Rust
    __version__,
    Observable,
    Condition,
    Contract,
    GBM,
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
)

__all__ = [
    "__version__",
    "Observable",
    "Condition",
    "Contract",
    "GBM",
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
]
