"""
Options digitales : cash-or-nothing, asset-or-nothing, corridor.

Illustre trois types d'options binaires construits avec le DSL kontract :

1. **Cash-or-nothing call** (k.cash_or_nothing_call)
   Verse `payout` si S_T ≥ K à maturité T.
   Prix analytique : payout × e^{-rT} × N(d₂)

2. **Asset-or-nothing call** (construit en DSL pur)
   Verse S_T si S_T ≥ K à maturité T.
   Forme DSL : `(k.S("X") * k.one(k.USD)) @ (k.S("X") >= K) @ k.at(T)`
   Le premier `@` applique la condition de payoff, le second `@` fournit
   l'horizon temporel (obligatoire — sans `at(T)`, le prix serait 0).
   Prix analytique : S₀ × N(d₁)

3. **Corridor digital** (construit en DSL pur)
   Verse 10 USD si 95 ≤ S_T ≤ 105.
   Forme DSL : `(k.const_(10) * k.one(k.USD)) @ ((S≥95) & (S≤105)) @ k.at(T)`
   Prix analytique : 10 × e^{-rT} × (N(d₂(95)) − N(d₂(105)))

NOTE DSL — double @ pour l'asset-or-nothing :
  La chaîne `payoff @ cond @ k.at(T)` se lit de gauche à droite :
    1. `payoff @ cond`   → contrat conditionnel au spot final ≥ K
    2. résultat @ at(T)  → évalué à la date T (horizon nécessaire pour le pricer)
  C'est la forme testée qui donne un résultat positif et cohérent avec BS.
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
PAYOUT = 10.0
N_PATHS = 100_000
SEED = 42


# ── Fonctions BS ────────────────────────────────────────────────────────────
def N_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def d1_d2(S: float, K: float, r: float, sigma: float, T: float):
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    return d1, d1 - sigma * math.sqrt(T)


d1, d2 = d1_d2(S0, K, r, sigma, T)

# Prix analytiques
con_analytic = PAYOUT * math.exp(-r * T) * N_cdf(d2)   # cash-or-nothing
aon_analytic = S0 * N_cdf(d1)                           # asset-or-nothing

# Corridor : 10 USD si 95 ≤ S_T ≤ 105
# = CoN(K=95) - CoN(K=105), payout=10
d2_95 = (math.log(S0 / 95.0) + (r - 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
d2_105 = (math.log(S0 / 105.0) + (r - 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
corridor_analytic = PAYOUT * math.exp(-r * T) * (N_cdf(d2_95) - N_cdf(d2_105))

# ── Contrats DSL ───────────────────────────────────────────────────────────
m = k.GBM(s0=S0, sigma=sigma, r=r, asset="X")

# 1. Cash-or-nothing (produit catalogue)
c_con = k.cash_or_nothing_call("X", K, PAYOUT, T, k.USD)

# 2. Asset-or-nothing (DSL pur — double @ : condition puis horizon)
#    Le premier @ applique le payoff conditionnel (S_T ≥ K),
#    le second @ fournit l'horizon at(T) sans lequel le pricer renvoie 0.
c_aon = (k.S("X") * k.one(k.USD)) @ (k.S("X") >= K) @ k.at(T)

# 3. Corridor digital (DSL pur)
c_corridor = (
    (k.const_(PAYOUT) * k.one(k.USD))
    @ ((k.S("X") >= 95.0) & (k.S("X") <= 105.0))
    @ k.at(T)
)

# ── Pricing ────────────────────────────────────────────────────────────────
r_con      = c_con.price(m,      n_paths=N_PATHS, seed=SEED)
r_aon      = c_aon.price(m,      n_paths=N_PATHS, seed=SEED)
r_corridor = c_corridor.price(m, n_paths=N_PATHS, seed=SEED)

# ── Affichage ──────────────────────────────────────────────────────────────
print(f"Options digitales : S={S0}, K={K}, r={r}, σ={sigma}, T={T}")
print(f"  d₁={d1:.4f}  d₂={d2:.4f}  N(d₁)={N_cdf(d1):.5f}  N(d₂)={N_cdf(d2):.5f}\n")

print(f"{'Option':<28}  {'Prix MC':>9}  {'Analytique':>11}  {'Err. rel.':>10}")
print("-" * 65)

for label, r_mc, analytic in [
    ("Cash-or-nothing (catalogue)", r_con.price,      con_analytic),
    ("Asset-or-nothing (DSL pur)",  r_aon.price,      aon_analytic),
    ("Corridor [95, 105]",          r_corridor.price, corridor_analytic),
]:
    err_rel = abs(r_mc - analytic) / analytic if analytic != 0 else float("nan")
    print(f"{label:<28}  {r_mc:>9.4f}  {analytic:>11.4f}  {err_rel:>10.4%}")

print()
print("Formules analytiques :")
print(f"  CoN  = payout × e^{{-rT}} × N(d₂) = {PAYOUT} × {math.exp(-r*T):.4f} × {N_cdf(d2):.5f}")
print(f"  AoN  = S₀ × N(d₁) = {S0} × {N_cdf(d1):.5f}")
print(f"  Corr = 10 × e^{{-rT}} × (N(d₂(95))−N(d₂(105)))")

# ── Vérifications ──────────────────────────────────────────────────────────
# Cash-or-nothing < 5 % de l'analytique
assert abs(r_con.price - con_analytic) / con_analytic < 0.05, (
    f"CoN : erreur relative {abs(r_con.price - con_analytic)/con_analytic:.4%}"
)
# Asset-or-nothing positif et cohérent avec BS < 5 %
assert r_aon.price > 0.0, "Asset-or-nothing doit être positif"
assert abs(r_aon.price - aon_analytic) / aon_analytic < 0.05, (
    f"AoN : erreur relative {abs(r_aon.price - aon_analytic)/aon_analytic:.4%}"
)
# Corridor positif et cohérent avec analytique < 10 % (moins de chemins ici)
assert r_corridor.price > 0.0, "Corridor doit être positif"
assert abs(r_corridor.price - corridor_analytic) / corridor_analytic < 0.10, (
    f"Corridor : erreur relative {abs(r_corridor.price - corridor_analytic)/corridor_analytic:.4%}"
)

print("\n✓ Toutes les assertions passent.")
