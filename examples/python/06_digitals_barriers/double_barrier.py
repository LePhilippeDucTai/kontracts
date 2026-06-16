"""
Double barrière knock-out.

Un call double KO s'annule dès que le sous-jacent sort d'un corridor
[H_bas, H_haut] pendant la vie de l'option.

Construction DSL :
    c_double_ko = k.european_call(...).until(
        (k.S("X") >= H_haut) | (k.S("X") <= H_bas)
    )

Le `.until(cond)` termine le contrat dès que la condition est vraie.
L'horizon temporel (T=1 an) est fourni par le `at(T)` interne de
`european_call`.

Propriétés vérifiées :
  • Prix(double KO) < Prix(single KO haut)   (double contrainte plus restrictive)
  • Prix(double KO) < Prix(vanille)
  • Prix(double KO) > 0                      (scénario résiduel non nul)
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
H_HAUT = 130.0
H_BAS  = 80.0
N_PATHS = 100_000
SEED    = 42
STEPS   = 100   # monitoring continu approché


# ── Black-Scholes (référence vanille) ─────────────────────────────────────
def N_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(S: float, K: float, r: float, sigma: float, T: float) -> float:
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    return S * N_cdf(d1) - K * math.exp(-r * T) * N_cdf(d2)


prix_bs = bs_call(S0, K, r, sigma, T)

# ── Contrats ───────────────────────────────────────────────────────────────
m = k.GBM(s0=S0, sigma=sigma, r=r, asset="X")

c_vanilla   = k.european_call("X", K, T, k.USD)
c_uko       = k.up_and_out_call("X", K, H_HAUT, T, k.USD)           # single KO haut
c_double_ko = k.european_call("X", K, T, k.USD).until(
    (k.S("X") >= H_HAUT) | (k.S("X") <= H_BAS)
)

# ── Pricing ────────────────────────────────────────────────────────────────
kwargs = dict(n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

r_vanilla    = c_vanilla.price(  m, **kwargs)
r_uko        = c_uko.price(      m, **kwargs)
r_double_ko  = c_double_ko.price(m, **kwargs)

# ── Affichage ──────────────────────────────────────────────────────────────
print(f"Double barrière knock-out : S={S0}, K={K}")
print(f"  Barrière haute H_haut={H_HAUT}, Barrière basse H_bas={H_BAS}")
print(f"  r={r}, σ={sigma}, T={T}")
print(f"  steps_per_year={STEPS}, n_paths={N_PATHS}, seed={SEED}\n")
print(f"  Prix Black-Scholes (vanille) : {prix_bs:.4f}\n")

print(f"{'Option':<35}  {'Prix MC':>9}  {'% vanille':>10}")
print("-" * 60)

for label, r_mc in [
    ("Vanille",                       r_vanilla),
    (f"Up-and-out H={H_HAUT}",        r_uko),
    (f"Double KO [{H_BAS}, {H_HAUT}]", r_double_ko),
]:
    pct = r_mc.price / r_vanilla.price * 100 if r_vanilla.price > 0 else 0.0
    print(f"{label:<35}  {r_mc.price:>9.4f}  {pct:>10.2f}%")

# ── Vérifications ──────────────────────────────────────────────────────────
# Double KO < single KO haut (barrière supplémentaire en bas)
assert r_double_ko.price < r_uko.price, (
    f"Double KO ({r_double_ko.price:.4f}) devrait être < single KO haut "
    f"({r_uko.price:.4f})"
)

# Double KO < vanille
assert r_double_ko.price < r_vanilla.price, (
    f"Double KO ({r_double_ko.price:.4f}) devrait être < vanille "
    f"({r_vanilla.price:.4f})"
)

# Double KO > 0 (corridor [80, 130] — le spot peut rester dedans)
assert r_double_ko.price > 0.0, "Prix du double KO doit être positif"

print(f"\nRéduction vs vanille        : {(1 - r_double_ko.price/r_vanilla.price):.2%}")
print(f"Réduction vs single KO haut : {(1 - r_double_ko.price/r_uko.price):.2%}")
print("\n✓ Toutes les assertions passent.")
