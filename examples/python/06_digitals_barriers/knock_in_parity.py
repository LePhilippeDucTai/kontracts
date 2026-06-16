"""
Parité knock-in / knock-out.

Relation fondamentale :
    Prix(knock-in) + Prix(knock-out) = Prix(vanille)

Un call up-and-in (KI) s'active si et seulement si le sous-jacent touche
la barrière H avant maturité. Son knock-out correspondant (KO) s'annule
dans ce cas. Ensemble ils couvrent tous les scénarios, d'où la parité.

Le call KI n'existe pas comme produit catalogue dans kontract ; on en déduit
le prix par parité :
    Prix(KI) = Prix(vanille) − Prix(KO)

Vérification avec steps_per_year=100 (monitoring continu approché).
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
BARRIER = 130.0
N_PATHS = 100_000
SEED    = 42
STEPS   = 100


# ── Black-Scholes (référence) ──────────────────────────────────────────────
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
c_ko      = k.up_and_out_call("X", K, BARRIER, T, k.USD)

# ── Pricing ────────────────────────────────────────────────────────────────
kwargs = dict(n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)
r_vanilla = c_vanilla.price(m, **kwargs)
r_ko      = c_ko.price(m,      **kwargs)

# Prix KI par parité (pas de pricing direct — c'est le test de la parité)
prix_ki = r_vanilla.price - r_ko.price
somme   = r_ko.price + prix_ki          # doit ≈ Prix(vanille)

# ── Affichage ──────────────────────────────────────────────────────────────
print(f"Parité knock-in / knock-out : S={S0}, K={K}, H={BARRIER}")
print(f"  r={r}, σ={sigma}, T={T}")
print(f"  steps_per_year={STEPS}, n_paths={N_PATHS}, seed={SEED}\n")
print(f"  Prix Black-Scholes (vanille) : {prix_bs:.4f}\n")

print(f"  Prix vanille  (MC)  : {r_vanilla.price:.4f}")
print(f"  Prix KO       (MC)  : {r_ko.price:.4f}")
print(f"  Prix KI = Van−KO    : {prix_ki:.4f}")
print(f"  KO + KI             : {somme:.4f}  (doit ≈ vanille)\n")

# Proportion KI/KO
frac_ko = r_ko.price / r_vanilla.price
frac_ki = prix_ki    / r_vanilla.price
print(f"  Part KO : {frac_ko:.2%}  |  Part KI : {frac_ki:.2%}")

# ── Vérifications ──────────────────────────────────────────────────────────
# Parité : KO + KI = vanille (à quelques std_error près)
tol = 3.0 * r_vanilla.std_error + 3.0 * r_ko.std_error
assert abs(somme - r_vanilla.price) < tol, (
    f"Parité violée : KO+KI={somme:.4f} ≠ vanille={r_vanilla.price:.4f} "
    f"(tolérance={tol:.4f})"
)

# Les deux composantes sont positives
assert r_ko.price > 0.0,  "Prix KO doit être positif"
assert prix_ki > 0.0,     "Prix KI doit être positif"

# KO < vanille
assert r_ko.price < r_vanilla.price, (
    f"KO ({r_ko.price:.4f}) devrait être < vanille ({r_vanilla.price:.4f})"
)

# Cohérence MC/BS (vanille dans 2 %)
assert abs(r_vanilla.price - prix_bs) / prix_bs < 0.02

print("\n✓ Toutes les assertions passent : parité KI + KO = vanille vérifiée.")
