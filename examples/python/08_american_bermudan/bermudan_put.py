"""
Put bermudéen — spectre européen → bermudéen → américain

Intuition économique :
  Un put bermudéen est intermédiaire entre le put européen (1 date) et
  le put américain (exercice continu).  On peut exercer seulement à un
  ensemble fini de dates d'exercice prédéfinies.

  Hiérarchie de prix (plus de dates ⟹ plus de valeur) :
    Européen (1 date)  ≤  Bermudéen (trimestriel)  ≤  Américain (mensuel)

  Ce script compare trois jeux de dates d'exercice :
    · Européen approché : [T]  (1 seule date, = maturité)
    · Bermudéen trimestriel : [0.25, 0.50, 0.75, 1.0]  (4 dates)
    · Américain approché : [1/12, 2/12, …, 12/12]  (12 dates mensuelles)

  La hiérarchie est vérifiée avec tolérance Monte-Carlo.

Ce script :
  - Calcule les trois prix avec le même payoff put et le même modèle
  - Affiche un tableau comparatif
  - Vérifie eu_approx ≤ berm ≤ us_approx (± 3σ MC)
  - Montre la « valeur marginale » de chaque tranche d'exercices supplémentaires
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Référence Black-Scholes (put européen)
# ---------------------------------------------------------------------------

def _norm_cdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_put(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return strike * math.exp(-r * t) * _norm_cdf(-d2) - s * _norm_cdf(-d1)


# ---------------------------------------------------------------------------
# Paramètres — ITM avec taux élevé pour une prime d'exercice anticipé visible
# ---------------------------------------------------------------------------
S0    = 90.0
K     = 100.0
T     = 1.0
SIGMA = 0.30
R     = 0.08
N_PATHS = 120_000
SEED    = 42
N_BASIS = 5

# Trois jeux de dates d'exercice
DATES_EU   = [T]                                          # européen (1 date)
DATES_BERM = [0.25, 0.50, 0.75, T]                       # bermudéen trimestriel
DATES_US   = [i / 12 for i in range(1, 13)]              # américain mensuel

print("=" * 65)
print("  KONTRACT — Spectre européen → bermudéen → américain")
print(f"  S0={S0}, K={K}, σ={SIGMA}, r={R}, T={T}Y")
print("=" * 65)

# ---------------------------------------------------------------------------
# Modèle GBM
# ---------------------------------------------------------------------------
model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")
S = k.S("X")

# ---------------------------------------------------------------------------
# Payoff du put  max(K - S, 0)
# ---------------------------------------------------------------------------
payoff = (k.const_(K) - S).clip(0.0) * k.one(k.USD)

# ---------------------------------------------------------------------------
# Référence analytique
# ---------------------------------------------------------------------------
bs_ref = bs_put(S0, K, R, SIGMA, T)

# ---------------------------------------------------------------------------
# Calcul des trois prix (même seed, même n_paths pour comparabilité)
# ---------------------------------------------------------------------------
print(f"\n--- Calcul en cours ... ---")

# Européen approché via LSM (1 date = T)
res_eu = payoff.price_american(
    model, exercise_dates=DATES_EU,
    n_paths=N_PATHS, seed=SEED, n_basis=N_BASIS,
)

# Bermudéen trimestriel
res_berm = payoff.price_american(
    model, exercise_dates=DATES_BERM,
    n_paths=N_PATHS, seed=SEED, n_basis=N_BASIS,
)

# Américain mensuel
res_us = payoff.price_american(
    model, exercise_dates=DATES_US,
    n_paths=N_PATHS, seed=SEED, n_basis=N_BASIS,
)

# Put européen MC pur (contrôle)
eu_mc = (payoff @ k.at(T)).price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=50)

# ---------------------------------------------------------------------------
# Tableau comparatif
# ---------------------------------------------------------------------------
print(f"\n--- Tableau comparatif ---")
print(f"  {'Produit':<30}  {'Dates':>5}  {'Prix':>8}  {'± σ_MC':>8}  {'Prime/EU':>10}")
print(f"  {'-'*65}")

rows = [
    ("Européen (BS analytique)", "-", bs_ref, 0.0, 0.0),
    ("Européen MC (@ k.at(T))",  "-", eu_mc.price, eu_mc.std_error, eu_mc.price - bs_ref),
    ("Européen LSM (1 date=T)",  "1", res_eu.price, res_eu.std_error, res_eu.price - bs_ref),
    ("Bermudéen trimestriel",    "4", res_berm.price, res_berm.std_error, res_berm.price - bs_ref),
    ("Américain mensuel",       "12", res_us.price, res_us.std_error, res_us.price - bs_ref),
]

for nom, n_dates, prix, sigma_mc, prime in rows:
    if sigma_mc == 0.0:
        print(f"  {nom:<30}  {n_dates:>5}  {prix:>8.4f}  {'—':>8}  {prime:>+10.4f}")
    else:
        print(f"  {nom:<30}  {n_dates:>5}  {prix:>8.4f}  {sigma_mc:>8.4f}  {prime:>+10.4f}")

# ---------------------------------------------------------------------------
# Valeur marginale de chaque tranche d'exercices supplémentaires
# ---------------------------------------------------------------------------
eu_base = res_eu.price

print(f"\n--- Valeur marginale des dates supplémentaires ---")
val_eu_to_berm = res_berm.price - eu_base
val_berm_to_us = res_us.price - res_berm.price
val_eu_to_us   = res_us.price - eu_base

print(f"  EU → Bermudéen (1 → 4 dates)  : +{val_eu_to_berm:.4f}")
print(f"  Bermudéen → US (4 → 12 dates)  : +{val_berm_to_us:.4f}")
print(f"  EU → US (1 → 12 dates, total)  : +{val_eu_to_us:.4f}")
print(f"")
print(f"  Interprétation : la plus grande valeur marginale est souvent")
print(f"  au passage EU → Bermudéen (premières opportunités d'exercice).")
print(f"  Le passage Bermudéen → mensuel apporte une prime incrémentale")
print(f"  plus modeste : on approche déjà l'exercice continu.")

# ---------------------------------------------------------------------------
# Assertions — hiérarchie eu ≤ berm ≤ us (avec tolérance MC)
# ---------------------------------------------------------------------------
tol_eu_berm = 3 * (res_eu.std_error + res_berm.std_error)
tol_berm_us = 3 * (res_berm.std_error + res_us.std_error)

assert res_berm.price >= res_eu.price - tol_eu_berm, (
    f"Bermudéen ({res_berm.price:.4f}) < Européen ({res_eu.price:.4f}) - tol ({tol_eu_berm:.4f})"
)
assert res_us.price >= res_berm.price - tol_berm_us, (
    f"US ({res_us.price:.4f}) < Bermudéen ({res_berm.price:.4f}) - tol ({tol_berm_us:.4f})"
)
print(f"\n  [OK] EU ≤ Bermudéen  (delta={val_eu_to_berm:+.4f}, tol={tol_eu_berm:.4f})")
print(f"  [OK] Bermudéen ≤ US  (delta={val_berm_to_us:+.4f}, tol={tol_berm_us:.4f})")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 65)
print("  RÉSUMÉ")
print("=" * 65)
print(f"  BS européen (référence)  : {bs_ref:.4f}")
print(f"  Européen LSM (1 date)    : {res_eu.price:.4f}  ≤")
print(f"  Bermudéen trimestriel    : {res_berm.price:.4f}  ≤")
print(f"  Américain mensuel        : {res_us.price:.4f}")
print(f"  Prime EU→US totale       : +{val_eu_to_us:.4f}")
print(f"  Tous les asserts sont verts — script OK")
