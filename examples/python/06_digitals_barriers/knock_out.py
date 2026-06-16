"""
Options à barrière knock-out : up-and-out et down-and-out.

Un call up-and-out s'annule si le sous-jacent touche ou dépasse la barrière
haute H avant maturité. Un call down-and-out s'annule si le spot descend en
dessous de la barrière basse.

Propriétés vérifiées :
  • Prix(KO) < Prix(vanille) — la barrière réduit toujours la valeur.
  • Barrière très éloignée → prix(KO) ≈ prix(vanille) (la barrière ne se déclenche
    presque jamais).

Note : steps_per_year=100 est requis pour surveiller la barrière en continu
       (monitoring discret plus fin que l'horizon de payoff).
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
BARRIER_UP   = 130.0   # barrière haute (up-and-out)
BARRIER_DOWN = 80.0    # barrière basse (down-and-out)
BARRIER_FAR  = 300.0   # barrière très éloignée (quasi-vanille)
N_PATHS = 100_000
SEED    = 42
STEPS   = 100          # monitoring fin de la barrière


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

c_vanilla = k.european_call("X", K, T, k.USD)
c_uoc     = k.up_and_out_call("X",   K, BARRIER_UP,   T, k.USD)
c_doc     = k.down_and_out_call("X", K, BARRIER_DOWN, T, k.USD)
c_uoc_far = k.up_and_out_call("X",   K, BARRIER_FAR,  T, k.USD)

# ── Pricing ────────────────────────────────────────────────────────────────
r_vanilla = c_vanilla.price(m, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)
r_uoc     = c_uoc.price(    m, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)
r_doc     = c_doc.price(    m, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)
r_uoc_far = c_uoc_far.price(m, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)

# ── Affichage ──────────────────────────────────────────────────────────────
print(f"Calls à barrière knock-out : S={S0}, K={K}, r={r}, σ={sigma}, T={T}")
print(f"steps_per_year={STEPS}, n_paths={N_PATHS}, seed={SEED}\n")
print(f"  Prix Black-Scholes (vanille) : {prix_bs:.4f}\n")

print(f"{'Option':<40}  {'Prix MC':>9}  {'% de vanille':>13}")
print("-" * 68)

for label, r_mc in [
    ("Vanille (référence)",                   r_vanilla),
    (f"Up-and-out    H={BARRIER_UP}",         r_uoc),
    (f"Down-and-out  H={BARRIER_DOWN}",       r_doc),
    (f"Up-and-out    H={BARRIER_FAR} (loin)", r_uoc_far),
]:
    pct = r_mc.price / r_vanilla.price * 100
    print(f"{label:<40}  {r_mc.price:>9.4f}  {pct:>13.2f}%")

# ── Vérifications ──────────────────────────────────────────────────────────
# 1. KO < vanille
assert r_uoc.price < r_vanilla.price, (
    f"Up-and-out ({r_uoc.price:.4f}) devrait être < vanille ({r_vanilla.price:.4f})"
)
assert r_doc.price < r_vanilla.price, (
    f"Down-and-out ({r_doc.price:.4f}) devrait être < vanille ({r_vanilla.price:.4f})"
)

# 2. Barrière très éloignée ≈ vanille (à 10 % près avec MC)
err_far = abs(r_uoc_far.price - r_vanilla.price) / r_vanilla.price
assert err_far < 0.10, (
    f"Barrière H={BARRIER_FAR} : écart à la vanille trop grand ({err_far:.4%})"
)

# 3. Tous les prix sont positifs
assert r_uoc.price > 0.0, "Up-and-out doit être > 0"
assert r_doc.price > 0.0, "Down-and-out doit être > 0"

print(f"\nÉcart entre up-and-out lointain et vanille : {err_far:.4%}  (doit être < 10 %)")
print("\n✓ Toutes les assertions passent.")
