"""
Option sur panier (basket option) — 3 actifs équipondérés
=========================================================
Prix d'un call sur panier équipondéré de 3 actifs corrélés, comparé
à l'approximation analytique Black-Scholes avec vol du panier.

Panier :
    B_T = (S1_T + S2_T + S3_T) / 3

Option :
    Payoff = max(B_T - K, 0)

Approximation BS du panier :
    σ_panier = σ × sqrt((1 + 2ρ) / 3)

Cette formule exacte pour un panier de N actifs identiques avec vol σ
et corrélation ρ par paire — elle sous-estime légèrement le prix MC
car elle ignore la log-normalité du panier (Jensen inequality).

Paramètres :
  S0 = 100, σ = 0.20, ρ = 0.50, r = 0.05, K = 100, T = 1 an.
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Formule analytique Black-Scholes (call européen)
# ---------------------------------------------------------------------------

def bs_call(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    d1 = (math.log(S0 / K) + (r + 0.5 * sigma**2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
    return S0 * N(d1) - K * math.exp(-r * T) * N(d2)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
s0   = 100.0   # spot initial de chaque actif
sigma = 0.20   # volatilité de chaque actif
rho   = 0.50   # corrélation entre actifs
r     = 0.05   # taux sans risque
K     = 100.0  # strike
T     = 1.0    # maturité (années)
N_ASSETS = 3

PATHS = 100_000
STEPS =  50
SEED  =  42

# ---------------------------------------------------------------------------
# Modèle multi-actifs : GBM corrélé
# ---------------------------------------------------------------------------
factors = [
    k.GbmFactor("S1", s0, r, sigma),
    k.GbmFactor("S2", s0, r, sigma),
    k.GbmFactor("S3", s0, r, sigma),
]

# Matrice de corrélation 3×3 (corrélation uniforme ρ hors diagonale)
corr = [
    [1.0, rho, rho],
    [rho, 1.0, rho],
    [rho, rho, 1.0],
]

model = k.correlated_gbm(factors, corr, r=r)

# ---------------------------------------------------------------------------
# Contrat : call sur panier équipondéré
# ---------------------------------------------------------------------------
S1, S2, S3 = k.S("S1"), k.S("S2"), k.S("S3")

basket = (
    (S1 + S2 + S3) / 3.0 - k.const_(K)
).clip(0.0) * k.one(k.USD) @ k.at(T)

# ---------------------------------------------------------------------------
# Pricing Monte-Carlo
# ---------------------------------------------------------------------------
res = basket.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)

# ---------------------------------------------------------------------------
# Référence analytique — vol du panier
# ---------------------------------------------------------------------------
sigma_basket = sigma * math.sqrt((1 + 2 * rho) / N_ASSETS)
# Pour un panier, S0_basket = s0 (actifs équipondérés avec même spot initial)
prix_bs_basket = bs_call(s0, K, T, r, sigma_basket)

ecart_rel = abs(res.price - prix_bs_basket) / prix_bs_basket

# ---------------------------------------------------------------------------
# Affichage
# ---------------------------------------------------------------------------
print("=" * 65)
print("  Option sur panier équipondéré — 3 actifs corrélés")
print("=" * 65)
print(f"\nParamètres : S0={s0}, σ={sigma}, ρ={rho}, r={r}, K={K}, T={T} an")
print(f"  n_paths={PATHS:,}, steps/an={STEPS}, seed={SEED}")
print()
print(f"Prix MC (basket call)    : {res.price:.4f}")
print(f"Erreur standard          : {res.std_error:.4f}")
print(f"IC 95 %                  : [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print()
print(f"σ_panier analytique      : {sigma_basket:.4f}  "
      f"  (= σ·√((1+2ρ)/3) = {sigma:.2f}·√({(1+2*rho)/3:.4f}))")
print(f"Prix BS(σ_panier)        : {prix_bs_basket:.4f}")
print(f"Ecart relatif MC vs BS   : {ecart_rel*100:.2f}%")
print()
print("Commentaire :")
print("  - La corrélation ρ = 0.50 réduit la vol effective du panier :")
print(f"    σ_panier = {sigma_basket:.4f} < σ_actif = {sigma:.4f}")
print("  - La diversification ↓ vol → basket moins cher qu'un call mono-actif")
print(f"    BS(σ_mono={sigma}) = {bs_call(s0, K, T, r, sigma):.4f}  vs  basket = {res.price:.4f}")
print("  - L'approximation BS du panier légèrement sous-estime le prix MC")
print("    car elle suppose un panier log-normal (approximation de premier ordre).")
print("  - Plus la corrélation est faible, plus la diversification réduit la vol")
print("    et donc le prix de l'option sur panier.")

# Pour illustrer la dépendance à la corrélation
print()
print("Sensibilité du prix à la corrélation :")
print(f"  {'ρ':<8} {'σ_panier':<12} {'BS_panier':<12} {'Commentaire'}")
print("  " + "-" * 55)
for rho_val in [0.0, 0.3, 0.5, 0.7, 1.0]:
    sig_b = sigma * math.sqrt((1 + 2 * rho_val) / N_ASSETS)
    p_b   = bs_call(s0, K, T, r, sig_b)
    note = "(corrélation parfaite → mono-actif)" if rho_val == 1.0 else (
           "(actifs indépendants → max diversification)" if rho_val == 0.0 else ""
    )
    print(f"  {rho_val:<8} {sig_b:<12.4f} {p_b:<12.4f} {note}")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
assert ecart_rel < 0.04, (
    f"Ecart MC vs BS_panier trop grand : {ecart_rel*100:.2f}% (max 4%)"
)
assert res.price > 0, "Prix du basket doit être positif"
assert res.price < bs_call(s0, K, T, r, sigma) * 1.1, (
    "Basket < call mono-actif × 1.1 (diversification)"
)

print()
print("Assertions OK — prix panier MC dans la tolérance de 4% vs BS(σ_panier).")
