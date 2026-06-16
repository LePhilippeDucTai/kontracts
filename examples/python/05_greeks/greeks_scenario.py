"""
Analyse scénario : Greeks en fonction du spot.

Boucle sur une grille de spots (80 à 120 par pas de 10) et calcule
Δ, Γ et ν pour chaque valeur en reconstruisant un modèle GBM avec le
spot correspondant. Imprime un tableau et vérifie la monotonie de delta.

Intuition financière :
  • Delta croît avec S (le call devient plus ITM → Δ → 1).
  • Gamma est maximal ATM (courbure maximale de la valeur en fonction de S).
  • Vega est aussi maximal ATM (la valeur est la plus sensible à la volatilité
    quand l'option est proche de la monnaie).
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
K      = 100.0       # strike
r      = 0.05
sigma  = 0.20
T      = 1.0
SPOTS  = [80.0, 90.0, 100.0, 110.0, 120.0]
N_PATHS = 100_000
SEED   = 42


# ── Formules BS analytiques (référence) ───────────────────────────────────
def N_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def phi(x: float) -> float:
    return math.exp(-0.5 * x * x) / math.sqrt(2.0 * math.pi)


def bs_delta(S: float) -> float:
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    return N_cdf(d1)


def bs_gamma(S: float) -> float:
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    return phi(d1) / (S * sigma * math.sqrt(T))


def bs_vega(S: float) -> float:
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    return S * phi(d1) * math.sqrt(T)


# ── Calcul sur la grille de spots ──────────────────────────────────────────
call   = k.european_call("X", K, T, k.USD)

results = [
    (
        s,
        call.greeks(k.GBM(s0=s, sigma=sigma, r=r, asset="X"),
                    n_paths=N_PATHS, seed=SEED),
    )
    for s in SPOTS
]

# ── Tableau de résultats ───────────────────────────────────────────────────
print(f"Greeks d'un call européen (K={K}, r={r}, σ={sigma}, T={T})\n")
print(f"{'Spot':>6}  {'Δ (MC)':>9}  {'Δ (BS)':>9}  {'Γ (MC)':>9}  "
      f"{'Γ (BS)':>9}  {'ν (MC)':>9}  {'ν (BS)':>9}")
print("-" * 75)

deltas_mc = []
for s, g in results:
    deltas_mc.append(g.delta)
    print(
        f"{s:>6.0f}  {g.delta:>9.4f}  {bs_delta(s):>9.4f}  "
        f"{g.gamma:>9.5f}  {bs_gamma(s):>9.5f}  "
        f"{g.vega:>9.3f}  {bs_vega(s):>9.3f}"
    )

# ── Vérification : delta est strictement croissant ─────────────────────────
est_croissant = all(
    deltas_mc[i] < deltas_mc[i + 1]
    for i in range(len(deltas_mc) - 1)
)
assert est_croissant, (
    f"Delta non strictement croissant : {[f'{d:.4f}' for d in deltas_mc]}"
)

# Vérification : delta ∈ (0, 1) pour tous les spots
assert all(0.0 < d < 1.0 for d in deltas_mc), (
    "Delta doit être dans (0, 1) pour un call"
)

# Delta doit être proche de BS (tolérance 0.02)
for s, g in results:
    err = abs(g.delta - bs_delta(s))
    assert err < 0.02, (
        f"Delta MC ({g.delta:.4f}) trop éloigné de BS ({bs_delta(s):.4f}) pour S={s}"
    )

print(f"\nDeltas MC : {[f'{d:.4f}' for d in deltas_mc]}")
print("→ Delta est strictement croissant avec le spot (call plus ITM).")
print("\n✓ Toutes les assertions passent.")
