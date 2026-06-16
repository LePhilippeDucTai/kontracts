"""
Option d'échange (Spread option) — formule de Margrabe
======================================================
Price l'option max(S1_T - S2_T, 0) via Monte-Carlo et compare avec
la formule analytique de Margrabe (1978).

Formule de Margrabe :
    Payoff = max(S1_T - S2_T, 0)
    Prix   = S1_0 · N(d1) - S2_0 · N(d2)

    σ_spread = sqrt(σ1² + σ2² - 2ρ σ1 σ2)
    d1 = [ln(S1_0/S2_0) + σ_spread²/2 · T] / (σ_spread · √T)
    d2 = d1 - σ_spread · √T

Cette formule est exacte (pas une approximation) car l'option d'échange
peut se ramener à un call sur S1 avec S2 comme numéraire.

Note : pour l'option de spread classique max(S1-S2-K, 0), K > 0,
il n'existe pas de formule analytique exacte (approximations de Kirk, etc.).
Ici K = 0 → formule de Margrabe exacte.
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Formule de Margrabe
# ---------------------------------------------------------------------------

def margrabe(S1: float, S2: float, T: float, sigma_spread: float) -> float:
    """
    Option d'échange max(S1_T - S2_T, 0) avec S1_0 = S1, S2_0 = S2.
    Taux sans risque : pas de discount car on échange deux actifs.
    """
    if sigma_spread <= 0 or T <= 0:
        return max(S1 - S2, 0.0)
    d1 = (math.log(S1 / S2) + 0.5 * sigma_spread**2 * T) / (sigma_spread * math.sqrt(T))
    d2 = d1 - sigma_spread * math.sqrt(T)
    N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
    return S1 * N(d1) - S2 * N(d2)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
s1_0  = 100.0   # spot initial actif 1
s2_0  = 100.0   # spot initial actif 2
sigma1 = 0.20   # vol actif 1
sigma2 = 0.20   # vol actif 2
rho    = 0.50   # corrélation S1–S2
r      = 0.05   # taux sans risque
T      = 1.0    # maturité

PATHS = 100_000
STEPS =  50
SEED  =  42

# Vol du spread
sigma_spread = math.sqrt(sigma1**2 + sigma2**2 - 2 * rho * sigma1 * sigma2)

# Modèle GBM corrélé 2 actifs
factors = [
    k.GbmFactor("S1", s1_0, r, sigma1),
    k.GbmFactor("S2", s2_0, r, sigma2),
]
corr = [[1.0, rho], [rho, 1.0]]
model = k.correlated_gbm(factors, corr, r=r)

# ---------------------------------------------------------------------------
# Contrat : option d'échange (spread option K=0)
# Payoff = max(S1_T - S2_T, 0) actualisé
# Note : dans kontract l'actualisation est incluse dans le pricing MC
# ---------------------------------------------------------------------------
S1, S2_obs = k.S("S1"), k.S("S2")
spread_option = (S1 - S2_obs).clip(0.0) * k.one(k.USD) @ k.at(T)

# ---------------------------------------------------------------------------
# Pricing
# ---------------------------------------------------------------------------
res = spread_option.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)

# La formule de Margrabe donne directement le prix de l'option d'échange
# sans discount supplémentaire : elle est formulée dans le numéraire S2
# et revient à S1_0·N(d1) - S2_0·N(d2), prix d'arbitrage équitable.
# Le moteur MC actualise via exp(-rT) mais les drifts GBM sont r pour les deux actifs,
# ce qui compense : E_Q[e^{-rT} max(S1_T-S2_T, 0)] = Margrabe(S1_0, S2_0, σ_spread, T).
prix_margrabe = margrabe(s1_0, s2_0, T, sigma_spread)

ecart_rel = abs(res.price - prix_margrabe) / prix_margrabe

# ---------------------------------------------------------------------------
# Affichage
# ---------------------------------------------------------------------------
print("=" * 65)
print("  Option d'échange (Spread K=0) — formule de Margrabe")
print("=" * 65)
print(f"\nParamètres : S1_0={s1_0}, S2_0={s2_0}, σ1={sigma1}, σ2={sigma2}")
print(f"  ρ={rho}, r={r}, T={T} an, K=0")
print(f"  σ_spread = √(σ1²+σ2²-2ρσ1σ2) = {sigma_spread:.4f}")
print(f"  n_paths={PATHS:,}, steps/an={STEPS}, seed={SEED}")
print()
print(f"Prix MC (spread option)  : {res.price:.4f}")
print(f"Erreur standard          : {res.std_error:.4f}")
print(f"IC 95 %                  : [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print()
print(f"Prix Margrabe (exact)    : {prix_margrabe:.4f}")
print(f"Ecart relatif MC vs Margrabe : {ecart_rel*100:.2f}%")
print()
print("Commentaire :")
print("  - La formule de Margrabe est EXACTE (pas une approximation) :")
print("    elle change de numéraire en utilisant S2 comme unité de compte.")
print("  - La corrélation ρ = 0.5 réduit la vol du spread :")
print(f"    si ρ→1 : σ_spread → 0 (actifs se déplacent ensemble)")
print(f"    si ρ→-1 : σ_spread → {math.sqrt(sigma1**2+sigma2**2+2*sigma1*sigma2):.4f} (max)")
print("  - S1_0 = S2_0 → l'option est ATM (d'échange) : prix ≠ 0 car σ_spread > 0.")

# Sensibilité à la corrélation
print()
print("Sensibilité à la corrélation ρ :")
print(f"  {'ρ':<8} {'σ_spread':<12} {'Margrabe (disc.)':<18} {'Commentaire'}")
print("  " + "-" * 60)
for rho_val in [-0.5, 0.0, 0.3, 0.5, 0.8]:
    sig_s = math.sqrt(sigma1**2 + sigma2**2 - 2 * rho_val * sigma1 * sigma2)
    p_m   = margrabe(s1_0, s2_0, T, sig_s)
    note  = ("← max corrélation, min prix" if rho_val == 0.8 else
             "← anti-corrélation, max prix" if rho_val == -0.5 else "")
    print(f"  {rho_val:<8} {sig_s:<12.4f} {p_m:<18.4f} {note}")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
assert ecart_rel < 0.04, (
    f"Ecart MC vs Margrabe trop grand : {ecart_rel*100:.2f}% (tolérance 4%)"
)
assert res.price > 0, "Prix spread option doit être > 0"

# Parité : S1_0 = S2_0 → prix symétrique (call sur spread = put sur spread)
# max(S1-S2,0) = max(S2-S1,0) en loi quand S1_0=S2_0, ρ uniforme → prix égaux
spread_inv = (S2_obs - S1).clip(0.0) * k.one(k.USD) @ k.at(T)
res_inv    = spread_inv.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
sym_ecart  = abs(res.price - res_inv.price) / res.price
assert sym_ecart < 0.05, (
    f"Avec S1_0=S2_0, max(S1-S2)+ ≈ max(S2-S1)+ en prix : "
    f"écart={sym_ecart*100:.2f}% > 5%"
)
print()
print(f"Vérification parité (S1_0=S2_0) :")
print(f"  max(S1-S2,0) = {res.price:.4f}  vs  max(S2-S1,0) = {res_inv.price:.4f}  "
      f"(écart {sym_ecart*100:.2f}%)")

print()
print("Assertions OK — prix MC dans la tolérance de 4% vs Margrabe.")
