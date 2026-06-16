"""
Comparaison de modèles — même call ATM sous GBM, Heston, Merton, SABR
======================================================================
Price un call européen ATM 1 an sous quatre modèles différents et
affiche un tableau comparatif avec la référence Black-Scholes.

Chaque modèle est calibré pour avoir une volatilité ATM effective ~ 20 %,
ce qui permet d'observer les écarts provenant de la structure stochastique
propre à chaque modèle (queues de distribution, mean-reversion, sauts…).

Paramètres de calibration :
  - GBM   : σ = 0.20 (Black-Scholes exact)
  - Heston: v0 = θ = 0.04, κ = 2, σ_v = 0.3, ρ = -0.5 (vol stochastique)
  - Merton: σ = 0.20, λ = 0.5, μ_j = -0.10, σ_j = 0.15 (sauts négatifs)
  - SABR  : α = 0.20, β = 1.0, ν = 0.4, ρ = -0.5 (vol de vol + skew)
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Formule analytique Black-Scholes
# ---------------------------------------------------------------------------

def bs_call(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    d1 = (math.log(S0 / K) + (r + 0.5 * sigma**2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
    return S0 * N(d1) - K * math.exp(-r * T) * N(d2)

def bs_put(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    return bs_call(S0, K, T, r, sigma) - S0 + K * math.exp(-r * T)


# ---------------------------------------------------------------------------
# Paramètres communs
# ---------------------------------------------------------------------------
S0, T, r = 100.0, 1.0, 0.05
BS_REF   = bs_call(S0, S0, T, r, 0.20)

SEED  = 42
PATHS = 100_000

# ---------------------------------------------------------------------------
# Définition des modèles
# ---------------------------------------------------------------------------
models = {
    "GBM": k.GBM(s0=S0, sigma=0.20, r=r, asset="X"),
    "Heston": k.heston(
        spot=S0, v0=0.04, kappa=2.0, theta=0.04,
        sigma_v=0.3, rho=-0.5, r=r, asset="X"
    ),
    "Merton": k.merton(S0, r, 0.20, 0.5, -0.10, 0.15, asset="X"),
    "SABR":   k.sabr(spot=S0, alpha=0.20, beta=1.0, nu=0.4, rho=-0.5, r=r, asset="X"),
}

steps_per_model = {
    "GBM":    50,
    "Heston": 100,
    "Merton":  50,
    "SABR":   100,
}

# ---------------------------------------------------------------------------
# Contrats : ATM, OTM call, OTM put
# ---------------------------------------------------------------------------
S = k.S("X")
strikes = {
    "Call ATM  K=100": k.european_call("X", 100.0, T, k.USD),
    "Call OTM  K=110": (S - k.const_(110.0)).clip(0.0) * k.one(k.USD) @ k.at(T),
    "Put  OTM  K=90 ": (k.const_(90.0) - S).clip(0.0)  * k.one(k.USD) @ k.at(T),
}

bs_refs = {
    "Call ATM  K=100": bs_call(S0, 100.0, T, r, 0.20),
    "Call OTM  K=110": bs_call(S0, 110.0, T, r, 0.20),
    "Put  OTM  K=90 ": bs_put( S0,  90.0, T, r, 0.20),
}

# ---------------------------------------------------------------------------
# Pricing
# ---------------------------------------------------------------------------
print("=" * 80)
print("  Comparaison de modèles — Call ATM 1 an (S0=100, r=5%, vol eff. 20%)")
print("=" * 80)
print()

results = {}
for nom, contrat in strikes.items():
    results[nom] = {}
    for model_name, model in models.items():
        steps = steps_per_model[model_name]
        res = contrat.price(model, n_paths=PATHS, seed=SEED, steps_per_year=steps)
        results[nom][model_name] = res

# ---------------------------------------------------------------------------
# Tableau principal
# ---------------------------------------------------------------------------
col_w = 13
header = f"{'Produit':<24} {'BS(20%)':<10}" + "".join(
    f"{m:<{col_w}}" for m in models.keys()
)
print(header)
print("-" * (24 + 10 + col_w * len(models)))

for nom, contrat in strikes.items():
    bs_v = bs_refs[nom]
    ligne = f"  {nom:<22} {bs_v:<10.4f}"
    for model_name in models:
        prix = results[nom][model_name].price
        ligne += f"{prix:<{col_w}.4f}"
    print(ligne)

# ---------------------------------------------------------------------------
# Tableau des écarts relatifs vs BS
# ---------------------------------------------------------------------------
print()
print("Ecarts relatifs vs Black-Scholes (%) :")
print(f"{'Produit':<24}" + "".join(f"{m:<{col_w}}" for m in models.keys()))
print("-" * (24 + col_w * len(models)))

for nom in strikes:
    bs_v  = bs_refs[nom]
    ligne = f"  {nom:<22}"
    for model_name in models:
        prix = results[nom][model_name].price
        ecart = (prix - bs_v) / bs_v * 100
        ligne += f"{ecart:+.2f}%      "[:col_w]
    print(ligne)

# ---------------------------------------------------------------------------
# Erreurs standard (incertitude MC)
# ---------------------------------------------------------------------------
print()
print("Erreurs standard Monte-Carlo (call ATM uniquement) :")
for model_name in models:
    se = results["Call ATM  K=100"][model_name].std_error
    print(f"  {model_name:<10} : σ_MC = {se:.4f}  ({se/BS_REF*100:.2f}% du prix BS)")

# ---------------------------------------------------------------------------
# Commentaires interprétatifs
# ---------------------------------------------------------------------------
print()
print("Commentaire :")
print("  GBM (Black-Scholes)")
print("    → Référence exacte. Volatilité constante, distribution log-normale.")
print("    Prix ATM = valeur analytique BS.")
print()
print("  Heston (vol stochastique)")
print("    → ATM proche de BS(20%) car v0 = θ = 0.04 (stationnarité).")
print("    La vol de vol (σ_v = 0.3) crée un smile — OTM calls légèrement")
print("    enrichis. Le levier ρ = -0.5 génère un skew favorable aux puts OTM.")
print()
print("  Merton (sauts)")
print("    → Les sauts négatifs (μ_j = -10%) épaississent la queue gauche.")
print("    Put OTM K=90 fortement enrichi. Call ATM > BS car la variance")
print("    totale effective est σ² + λ(μ_j² + σ_j²) > σ².")
print()
print("  SABR (vol stochastique, backbone β = 1)")
print("    → À ATM, SABR(α=0.20, β=1, ν=0.4) ≈ GBM(0.20) (résultat attendu).")
print("    L'effet ρ et ν sur les prix OTM se manifeste via le smile implicite.")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
# GBM doit être proche de BS
gbm_atm = results["Call ATM  K=100"]["GBM"].price
rel_gbm = abs(gbm_atm - BS_REF) / BS_REF
assert rel_gbm < 0.02, f"GBM doit ≈ BS : {rel_gbm*100:.2f}%"

# Merton avec sauts enrichit le call ATM vs GBM
merton_atm = results["Call ATM  K=100"]["Merton"].price
assert merton_atm > gbm_atm * 1.05, (
    f"Merton (sauts) devrait être > GBM à ATM : {merton_atm:.4f} vs {gbm_atm:.4f}"
)

# Merton enrichit fortement les puts OTM
merton_put = results["Put  OTM  K=90 "]["Merton"].price
gbm_put    = results["Put  OTM  K=90 "]["GBM"].price
assert merton_put > gbm_put * 1.30, (
    f"Merton enrichit fortement le put OTM : {merton_put:.4f} vs GBM {gbm_put:.4f}"
)

# Tous les prix sont positifs
for nom in strikes:
    for model_name in models:
        prix = results[nom][model_name].price
        assert prix >= 0, f"Prix négatif : {nom} / {model_name} = {prix}"

print()
print("Assertions OK — comparaison de modèles cohérente.")
