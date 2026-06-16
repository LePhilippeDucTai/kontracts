"""
Modèle de Heston — volatilité stochastique
===========================================
Compare le prix d'un call ATM sous Heston avec le prix Black-Scholes (vol 20 %).

Le modèle de Heston (1993) suppose que la variance instantanée v_t suit
un processus CIR mean-reverting :

    dS = S (r dt + sqrt(v_t) dW_S)
    dv = κ(θ - v) dt + σ_v sqrt(v_t) dW_v    corr(dW_S, dW_v) = ρ

Paramètres retenus : v0 = θ = 0.04 (vol 20 %), κ = 2, σ_v = 0.3, ρ = -0.5.
Quand v0 = θ et κ élevé, Heston ≈ BS(sqrt(θ)) ≈ BS(20 %).
"""

import math
import kontract as k


# ---------------------------------------------------------------------------
# Formule analytique Black-Scholes (call européen)
# ---------------------------------------------------------------------------

def bs_call(S0: float, K: float, T: float, r: float, sigma: float) -> float:
    """Prix Black-Scholes d'un call européen."""
    d1 = (math.log(S0 / K) + (r + 0.5 * sigma**2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    N = lambda x: 0.5 * (1 + math.erf(x / math.sqrt(2)))
    return S0 * N(d1) - K * math.exp(-r * T) * N(d2)


# ---------------------------------------------------------------------------
# Paramètres communs
# ---------------------------------------------------------------------------
S0, K, T, r = 100.0, 100.0, 1.0, 0.05

# Modèle Heston
model_heston = k.heston(
    spot=S0,
    v0=0.04,       # variance initiale (vol 20 %)
    kappa=2.0,     # vitesse de retour à la moyenne
    theta=0.04,    # variance long terme (vol 20 %)
    sigma_v=0.3,   # vol de vol
    rho=-0.5,      # corrélation spot-vol (levier négatif)
    r=r,
    asset="X",
)

# Contrat : call ATM à maturité T
contrat = k.european_call("X", K, T, k.USD)

# ---------------------------------------------------------------------------
# Pricing Monte-Carlo Heston
# ---------------------------------------------------------------------------
print("=" * 60)
print("  Modèle de Heston — call ATM 1 an")
print("=" * 60)

res = contrat.price(model_heston, n_paths=100_000, seed=42, steps_per_year=100)

prix_heston = res.price
prix_bs = bs_call(S0, K, T, r, sigma=0.20)

ecart_abs = prix_heston - prix_bs
ecart_rel = abs(ecart_abs) / prix_bs

print(f"\nPrix Heston  (MC) : {prix_heston:.4f}  ± {res.std_error:.4f}")
print(f"IC 95 %          : [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print(f"Prix BS(20 %)    : {prix_bs:.4f}")
print(f"Ecart absolu     : {ecart_abs:+.4f}")
print(f"Ecart relatif    : {ecart_rel*100:.2f} %")
print()
print("Commentaire :")
print("  - v0 = θ → la variance initiale est à sa valeur long-terme.")
print("  - κ = 2 assure un retour rapide → la vol reste proche de 20 %.")
print("  - ρ = -0.5 crée un levier négatif (skew de volatilité) :")
print("    les puts OTM coûtent plus cher que sous BS, mais l'ATM est proche.")
print("  - σ_v = 0.3 ajoute de la convexité (smile), visible sur les wings.")

# ---------------------------------------------------------------------------
# Comparaison call OTM / put OTM pour observer le skew
# ---------------------------------------------------------------------------
print()
print("Impact du skew Heston (call OTM K=110 vs put OTM K=90) :")

S = k.S("X")

call_otm = (S - k.const_(110.0)).clip(0.0) * k.one(k.USD) @ k.at(T)
put_otm  = (k.const_(90.0) - S).clip(0.0)  * k.one(k.USD) @ k.at(T)

res_co = call_otm.price(model_heston, n_paths=100_000, seed=42, steps_per_year=100)
res_po = put_otm.price(model_heston,  n_paths=100_000, seed=42, steps_per_year=100)

bs_co = bs_call(S0, 110.0, T, r, 0.20)
# put par parité call-put
bs_po = bs_call(S0, 90.0, T, r, 0.20) - S0 + 90.0 * math.exp(-r * T)

print(f"  Call K=110 : Heston {res_co.price:.4f}  |  BS(20 %) {bs_co:.4f}")
print(f"  Put  K=90  : Heston {res_po.price:.4f}  |  BS(20 %) {bs_po:.4f}")
print("  → Le put OTM Heston > BS(20 %) : le skew de vol augmente")
print("    la probabilité implicite des fortes baisses.")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
assert ecart_rel < 1.5 / prix_bs * prix_bs, (
    f"Ecart ATM trop grand : {ecart_abs:.4f} (max autorisé 1.5)"
)
assert abs(ecart_abs) < 1.5, (
    f"Ecart ATM absolu trop grand : {ecart_abs:.4f}"
)

print()
print("Assertions OK — prix Heston ATM dans la tolérance de ±1.5 vs BS(20 %).")
