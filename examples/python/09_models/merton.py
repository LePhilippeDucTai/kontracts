"""
Modèle de Merton à sauts — enrichissement des ailes du smile
=============================================================
Compare le prix d'options sous Merton (1976) avec et sans sauts.

Modèle de Merton :
    dS/S = (r - λ μ̄) dt + σ dW + (J - 1) dN

où N est un processus de Poisson d'intensité λ, et J est log-normal :
    ln J ~ N(μ_j, σ_j²)   →   μ̄ = exp(μ_j + σ_j²/2) - 1

Intuition :
  - λ = 0 → modèle purement GBM (Black-Scholes)
  - λ > 0 → sauts aléatoires qui «épaississent» les queues de distribution
    → les options OTM (profondément hors de la monnaie) coûtent plus cher
  - μ_j < 0 (sauts négatifs en moyenne) → smile avec skew négatif marqué :
    les puts OTM s'apprécient davantage que les calls OTM.

Paramètres retenus :
  σ = 0.20, λ = 0.5 (un saut tous les 2 ans en moyenne),
  μ_j = -0.10 (saut de -10 % en moyenne), σ_j = 0.15.
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Formule Black-Scholes de référence
# ---------------------------------------------------------------------------

def bs_call(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    d1 = (math.log(S0 / K) + (r + 0.5 * sigma**2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
    return S0 * N(d1) - K * math.exp(-r * T) * N(d2)

def bs_put(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    return bs_call(S0, K, T, r, sigma) - S0 + K * math.exp(-r * T)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
S0, T, r = 100.0, 1.0, 0.05
sigma  = 0.20
lambda_  = 0.50   # intensité des sauts (0.5/an)
mu_j   = -0.10   # saut moyen log (≈ -10 %)
sigma_j = 0.15   # vol du saut

PATHS = 100_000
STEPS =  50
SEED  =  42

# Modèle Merton avec sauts
model_merton = k.merton(S0, r, sigma, lambda_, mu_j, sigma_j, asset="X")

# Modèle sans sauts (λ = 0 ≈ Black-Scholes)
model_nosaut = k.merton(S0, r, sigma, 0.0, mu_j, sigma_j, asset="X")

# ---------------------------------------------------------------------------
# Contrats : ATM, call OTM, put OTM, call très OTM
# ---------------------------------------------------------------------------
S = k.S("X")
call_atm  = k.european_call("X", 100.0, T, k.USD)
call_otm  = (S - k.const_(110.0)).clip(0.0) * k.one(k.USD) @ k.at(T)
call_dotm = (S - k.const_(125.0)).clip(0.0) * k.one(k.USD) @ k.at(T)
put_otm   = (k.const_(90.0) - S).clip(0.0)  * k.one(k.USD) @ k.at(T)
put_dotm  = (k.const_(80.0) - S).clip(0.0)  * k.one(k.USD) @ k.at(T)

print("=" * 68)
print("  Modèle de Merton — impact des sauts sur les options OTM")
print("=" * 68)
print(f"\nParamètres : σ={sigma}, λ={lambda_}, μ_j={mu_j}, σ_j={sigma_j}")
print(f"             S0={S0}, K_ATM=100, T={T} an, r={r}")

# ---------------------------------------------------------------------------
# Pricing
# ---------------------------------------------------------------------------
def price(contrat, model):
    return contrat.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)

r_atm_m  = price(call_atm,  model_merton)
r_otm_m  = price(call_otm,  model_merton)
r_dotm_m = price(call_dotm, model_merton)
r_pot_m  = price(put_otm,   model_merton)
r_pdt_m  = price(put_dotm,  model_merton)

r_atm_0  = price(call_atm,  model_nosaut)
r_otm_0  = price(call_otm,  model_nosaut)
r_dotm_0 = price(call_dotm, model_nosaut)
r_pot_0  = price(put_otm,   model_nosaut)
r_pdt_0  = price(put_dotm,  model_nosaut)

bs_atm  = bs_call(S0, 100.0, T, r, sigma)
bs_otm  = bs_call(S0, 110.0, T, r, sigma)
bs_dotm = bs_call(S0, 125.0, T, r, sigma)
bs_pot  = bs_put( S0,  90.0, T, r, sigma)
bs_pdt  = bs_put( S0,  80.0, T, r, sigma)

print()
print(f"{'Produit':<24} {'BS(20%)':<10} {'Merton λ=0':<13} {'Merton λ=0.5':<14} {'Δ prix':<10} {'Δ%'}")
print("-" * 78)

rows = [
    ("Call ATM  K=100",  bs_atm,  r_atm_0.price,  r_atm_m.price),
    ("Call OTM  K=110",  bs_otm,  r_otm_0.price,  r_otm_m.price),
    ("Call DOTM K=125",  bs_dotm, r_dotm_0.price, r_dotm_m.price),
    ("Put  OTM  K=90 ",  bs_pot,  r_pot_0.price,  r_pot_m.price),
    ("Put  DOTM K=80 ",  bs_pdt,  r_pdt_0.price,  r_pdt_m.price),
]
for label, bs_v, nosaut, merton in rows:
    delta    = merton - nosaut
    pct      = delta / nosaut * 100 if nosaut > 0.001 else float("inf")
    print(f"  {label:<22} {bs_v:<10.4f} {nosaut:<13.4f} {merton:<14.4f} {delta:+.4f}   {pct:+.1f}%")

print()
print("Commentaire :")
print("  - λ = 0 : le modèle Merton est exactement BS(σ). ✓")
print("  - Les sauts (λ = 0.5) gonflent toutes les options, mais l'effet est")
print("    DISPROPORTIONNÉ sur les wings (OTM et DOTM) :")
print("    les sauts créent une masse dans les queues que le GBM n'a pas.")
print("  - μ_j = -0.10 (sauts négatifs) : les puts OTM s'enrichissent plus")
print("    que les calls OTM → skew négatif caractéristique des marchés actions.")
print("  - Le call DOTM K=125 s'apprécie moins que le put DOTM K=80 car les")
print("    sauts tirent le spot vers le bas en moyenne.")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
# λ=0 ≈ BS
rel_lambda0 = abs(r_atm_0.price - bs_atm) / bs_atm
assert rel_lambda0 < 0.02, (
    f"Merton λ=0 devrait ≈ BS : écart relatif = {rel_lambda0*100:.2f}%"
)

# Merton > BS pour toutes les options (queues plus épaisses)
for label, bs_v, merton_v in [
    ("call ATM",  bs_atm,  r_atm_m.price),
    ("call OTM",  bs_otm,  r_otm_m.price),
    ("call DOTM", bs_dotm, r_dotm_m.price),
    ("put OTM",   bs_pot,  r_pot_m.price),
    ("put DOTM",  bs_pdt,  r_pdt_m.price),
]:
    assert merton_v >= bs_v * 0.98, (
        f"{label}: Merton ({merton_v:.4f}) devrait être ≥ BS ({bs_v:.4f})"
    )

# Les sauts enrichissent proportionnellement plus les wings
enrichissement_otm  = (r_otm_m.price  - r_otm_0.price)  / r_otm_0.price
enrichissement_dotm = (r_dotm_m.price - r_dotm_0.price) / r_dotm_0.price
enrichissement_atm  = (r_atm_m.price  - r_atm_0.price)  / r_atm_0.price
assert enrichissement_dotm > enrichissement_otm, (
    f"DOTM devrait s'enrichir plus que OTM : {enrichissement_dotm:.3f} vs {enrichissement_otm:.3f}"
)
assert enrichissement_otm > enrichissement_atm, (
    f"OTM devrait s'enrichir plus que ATM : {enrichissement_otm:.3f} vs {enrichissement_atm:.3f}"
)

print()
print("Assertions OK — sauts Merton enrichissent les wings plus que l'ATM.")
