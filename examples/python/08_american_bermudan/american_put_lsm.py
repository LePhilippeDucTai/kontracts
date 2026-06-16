"""
Put américain vs put européen — méthode LSM (Longstaff-Schwartz)

Intuition économique :
  Un put européen ne peut être exercé qu'à maturité T.
  Un put américain peut être exercé à tout instant t ∈ [0, T].

  Cette flexibilité d'exercice anticipé a une valeur :
    · Si le spot chute très bas, l'acheteur peut encaisser K - S_t
      immédiatement plutôt d'attendre que le prix remonte.
    · Plus le taux sans risque r est élevé, plus l'exercice anticipé
      est attrayant (la valeur actualisée de K - S décline avec le temps).
    · Plus le spot est sous K (ITM profond), plus la prime est grande.

  Prime d'exercice anticipé = Prix américain - Prix européen ≥ 0

  Paramètres utilisés : s0=90, σ=0.30, r=0.08 (ITM, forte prime).

Ce script :
  - Prix européen  = payoff @ k.at(T)   (MC classique)
  - Prix américain = payoff.price_american(model, dates_mensuelles)
  - Calcule et affiche la prime d'exercice anticipé
  - Vérifie la dominance US ≥ EU (moins une tolérance MC)
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
S0    = 90.0     # spot sous la frappe → put in-the-money
K     = 100.0
T     = 1.0
SIGMA = 0.30
R     = 0.08
N_PATHS = 120_000
SEED    = 42
N_BASIS = 5      # polynômes de Laguerre pour la régression LSM
STEPS   = 50

# Dates d'exercice mensuel (américain approché)
EXERCISE_DATES = [i / 12 for i in range(1, 13)]

print("=" * 65)
print("  KONTRACT — Put américain vs européen (LSM)")
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
# 1. Put européen  (exercice à T uniquement)
# ---------------------------------------------------------------------------
eu_put  = payoff @ k.at(T)
res_eu  = eu_put.price(model, n_paths=N_PATHS, seed=SEED, steps_per_year=STEPS)
bs_ref  = bs_put(S0, K, R, SIGMA, T)
rel_err = abs(res_eu.price - bs_ref) / bs_ref

print(f"\n--- 1. Put européen (exercice en T={T}) ---")
print(f"  Prix MC      : {res_eu.price:.4f} ± {res_eu.std_error:.4f}")
print(f"  IC 95%%       : [{res_eu.ci95_low:.4f}, {res_eu.ci95_high:.4f}]")
print(f"  Prix BS      : {bs_ref:.4f}")
print(f"  Erreur rel.  : {rel_err * 100:.2f}%")
assert rel_err < 0.03, f"Erreur MC trop grande pour le put EU : {rel_err:.4f}"
print("  [OK] Erreur MC < 3%")

# ---------------------------------------------------------------------------
# 2. Put américain (exercice mensuel — LSM Longstaff-Schwartz)
# ---------------------------------------------------------------------------
print(f"\n--- 2. Put américain (exercice mensuel, LSM) ---")
print(f"  Dates d'exercice : {[f'{d:.4f}' for d in EXERCISE_DATES]}")
res_us = payoff.price_american(
    model,
    exercise_dates=EXERCISE_DATES,
    n_paths=N_PATHS,
    seed=SEED,
    n_basis=N_BASIS,
)

print(f"  Prix MC      : {res_us.price:.4f} ± {res_us.std_error:.4f}")
print(f"  IC 95%%       : [{res_us.ci95_low:.4f}, {res_us.ci95_high:.4f}]")

# ---------------------------------------------------------------------------
# 3. Prime d'exercice anticipé
# ---------------------------------------------------------------------------
prime = res_us.price - res_eu.price
prime_rel = prime / res_eu.price * 100

print(f"\n--- 3. Prime d'exercice anticipé ---")
print(f"  Put européen     : {res_eu.price:.4f}")
print(f"  Put américain    : {res_us.price:.4f}")
print(f"  Prime absolue    : +{prime:.4f}")
print(f"  Prime relative   : +{prime_rel:.1f}%")
print(f"")
print(f"  Interprétation : avec S0={S0} < K={K} (ITM de {(K-S0)/K*100:.0f}%),")
print(f"  σ={SIGMA} et r={R}, il vaut parfois mieux encaisser K-S")
print(f"  maintenant plutôt que d'attendre : prime = {prime:.4f}.")
print(f"  Plus r est élevé, plus le « coût d'attente » (discount de K)")
print(f"  est grand → la prime d'exercice anticipé augmente.")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
tol = 3 * (res_eu.std_error + res_us.std_error)
assert res_us.price >= res_eu.price - tol, (
    f"US put ({res_us.price:.4f}) < EU put ({res_eu.price:.4f}) - tol ({tol:.4f}) : "
    f"violation de la dominance américaine"
)
assert prime > 0, (
    f"Prime d'exercice anticipé négative ({prime:.4f}) — inattendu pour ces paramètres"
)
print("\n  [OK] Put américain ≥ put européen (dominance vérifiée)")
print("  [OK] Prime d'exercice anticipé strictement positive")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 65)
print("  RÉSUMÉ")
print("=" * 65)
print(f"  Put européen   (T={T}Y, S0={S0}) : {res_eu.price:.4f}  (BS: {bs_ref:.4f})")
print(f"  Put américain  (mensuel, LSM)   : {res_us.price:.4f}")
print(f"  Prime d'exercice anticipé       : +{prime:.4f}  (+{prime_rel:.1f}%)")
print(f"  Tous les asserts sont verts — script OK")
