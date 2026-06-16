"""
Option de performance relative (Outperformance option)
======================================================
Price l'option sur la sur-performance de S1 par rapport à S2 :

    Payoff = max((S1_T/S1_0) - (S2_T/S2_0), 0)

Cela mesure si l'actif S1 a mieux performé que S2 sur la période [0, T].
Quand S1_0 = S2_0, ce payoff est équivalent à max(S1_T - S2_T, 0) / S1_0,
c'est-à-dire l'option de Margrabe normalisée.

Intuition financière :
  - Payoff positif si S1 surperforme S2 en termes relatifs.
  - Utile pour les gérants : droit à la sur-performance par rapport à un indice.
  - Plus la corrélation ρ est faible, plus la sur-performance est volatile
    → option plus chère.

Ce script montre :
  1. Le prix pour ρ = 0.3 et ρ = 0.7 pour illustrer la dépendance à ρ.
  2. Une vérification que le prix est positif (S1_0 = S2_0 → symétrie).
  3. La relation avec la formule de Margrabe normalisée.
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Formule de Margrabe normalisée (outperformance option)
# ---------------------------------------------------------------------------

def margrabe_norm(S1_0: float, S2_0: float, T: float,
                  sigma1: float, sigma2: float, rho: float) -> float:
    """
    Prix de l'option max((S1_T/S1_0) - (S2_T/S2_0), 0).
    Equivalent à Margrabe(1, 1, T, sigma_spread) / S1_0 × S1_0.
    """
    sigma_spread = math.sqrt(sigma1**2 + sigma2**2 - 2 * rho * sigma1 * sigma2)
    if sigma_spread < 1e-10:
        return max(1.0 - S2_0 / S1_0, 0.0)
    # Margrabe: prix de max(S1/S1_0 - S2/S2_0, 0)
    # = Margrabe(1, S2_0/S1_0, T, sigma_spread) avec S = 1, K = S2_0/S1_0
    # Si S1_0 = S2_0 → Margrabe(1, 1, T, sigma_spread)
    ratio = S2_0 / S1_0
    d1 = (math.log(1.0 / ratio) + 0.5 * sigma_spread**2 * T) / (sigma_spread * math.sqrt(T))
    d2 = d1 - sigma_spread * math.sqrt(T)
    N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
    return 1.0 * N(d1) - ratio * N(d2)  # normalisé par S1_0


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
s1_0  = 100.0
s2_0  = 100.0
sigma1 = 0.20
sigma2 = 0.20
r      = 0.05
T      = 1.0

PATHS  = 100_000
STEPS  =  50
SEED   =  42

# ---------------------------------------------------------------------------
# Contrat — outperformance
# ---------------------------------------------------------------------------
S1 = k.S("S1")
S2 = k.S("S2")

# Payoff = max(S1_T/s1_0 - S2_T/s2_0, 0) en USD
# Division par s1_0 et s2_0 (scalaires)
outperf = (S1 / s1_0 - S2 / s2_0).clip(0.0) * k.one(k.USD) @ k.at(T)

# ---------------------------------------------------------------------------
# Pricing pour deux corrélations
# ---------------------------------------------------------------------------
print("=" * 68)
print("  Option de performance relative — Outperformance S1 vs S2")
print("=" * 68)
print(f"\nPayoff = max(S1_T/{s1_0:.0f} - S2_T/{s2_0:.0f}, 0)")
print(f"Paramètres : S1_0={s1_0}, S2_0={s2_0}, σ1=σ2={sigma1}, r={r}, T={T}")
print(f"  n_paths={PATHS:,}, steps/an={STEPS}, seed={SEED}")

rho_vals = [0.3, 0.7]
resultats = {}

print()
print(f"{'ρ':<10} {'Prix MC':<12} {'σ_MC':<10} {'IC 95%':<24} {'Margrabe':<12} {'Ecart'}")
print("-" * 80)

for rho in rho_vals:
    factors = [
        k.GbmFactor("S1", s1_0, r, sigma1),
        k.GbmFactor("S2", s2_0, r, sigma2),
    ]
    corr  = [[1.0, rho], [rho, 1.0]]
    model = k.correlated_gbm(factors, corr, r=r)

    res   = outperf.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
    mgrab = margrabe_norm(s1_0, s2_0, T, sigma1, sigma2, rho)
    ecart = (res.price - mgrab) / mgrab * 100

    resultats[rho] = {"mc": res.price, "se": res.std_error,
                      "ci_lo": res.ci95_low, "ci_hi": res.ci95_high,
                      "mgrab": mgrab}

    print(f"  {rho:<8} {res.price:<12.5f} {res.std_error:<10.5f} "
          f"[{res.ci95_low:.5f}, {res.ci95_high:.5f}]  "
          f"{mgrab:<12.5f} {ecart:+.2f}%")

print()
print("Commentaire :")
print("  - S1_0 = S2_0 = 100 → l'outperformance est ATM (symétrique en loi).")
print("  - Le prix est toujours > 0 : même si les actifs sont identiques,")
print("    la dispersion aléatoire crée une probabilité de sur-performance > 0.")
print(f"  - ρ = 0.3 → σ_spread = {math.sqrt(sigma1**2+sigma2**2-2*0.3*sigma1*sigma2):.4f}")
print(f"    prix = {resultats[0.3]['mc']:.5f}  (plus cher car dispersion élevée)")
print(f"  - ρ = 0.7 → σ_spread = {math.sqrt(sigma1**2+sigma2**2-2*0.7*sigma1*sigma2):.4f}")
print(f"    prix = {resultats[0.7]['mc']:.5f}  (moins cher car actifs très corrélés)")
print()
print("  Interprétation : plus ρ est faible, plus les actifs divergent,")
print("  plus la sur-performance potentielle est grande → option plus chère.")
print("  À la limite ρ = 1 : S1 et S2 bougent de concert → outperformance nulle.")

# Sensibilité complète à la corrélation
print()
print("Sensibilité à la corrélation (Margrabe normalisé) :")
print(f"  {'ρ':<8} {'σ_spread':<12} {'Margrabe':<12} {'Commentaire'}")
print("  " + "-" * 52)
for rho_val in [-0.5, 0.0, 0.3, 0.5, 0.7, 1.0]:
    sig_s = math.sqrt(sigma1**2 + sigma2**2 - 2 * rho_val * sigma1 * sigma2)
    if rho_val < 1.0:
        d1 = (0 + 0.5 * sig_s**2 * T) / (sig_s * math.sqrt(T))
        d2 = d1 - sig_s * math.sqrt(T)
        N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
        p_m = N(d1) - N(d2)
    else:
        p_m = 0.0
    note = ("← ρ parfait : prix = 0" if rho_val == 1.0 else
            "← anti-corrélé : prix max" if rho_val == -0.5 else "")
    print(f"  {rho_val:<8} {sig_s:<12.4f} {p_m:<12.5f} {note}")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
# Prix > 0 pour les deux corrélations
for rho, d in resultats.items():
    assert d["mc"] > 0, f"Prix doit être > 0 pour ρ={rho} : {d['mc']}"

# Prix décroissant avec ρ (plus de corrélation → moins de dispersion → moins cher)
assert resultats[0.3]["mc"] > resultats[0.7]["mc"], (
    f"outperf(ρ=0.3)={resultats[0.3]['mc']:.5f} devrait être > "
    f"outperf(ρ=0.7)={resultats[0.7]['mc']:.5f}"
)

# Prix proche de Margrabe (tolérance 4%)
for rho, d in resultats.items():
    ecart_rel = abs(d["mc"] - d["mgrab"]) / d["mgrab"]
    assert ecart_rel < 0.04, (
        f"Ecart MC vs Margrabe pour ρ={rho} : {ecart_rel*100:.2f}% (max 4%)"
    )

print()
print("Assertions OK — prix > 0, décroissant avec ρ, proche de Margrabe (< 4%).")
