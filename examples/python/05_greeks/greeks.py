"""
Greeks d'un call européen : MC vs formules Black-Scholes analytiques.

Calcule Δ (delta), Γ (gamma), ν (vega) et ρ (rho) via k.Contract.greeks()
et les compare aux valeurs analytiques BS :

  Δ = N(d₁)
  Γ = φ(d₁) / (S₀ σ √T)
  ν = S₀ φ(d₁) √T         (pour une variation de σ de 1, i.e. +100 vol points)
  ρ = K T e^{-rT} N(d₂)   (pour une variation de r de 1)

Les greeks MC sont calculés par différences finies sur les paramètres du modèle
(bump-and-reprice intégré dans kontract).
"""

import math
import kontract as k

# ── Paramètres ─────────────────────────────────────────────────────────────
S0, K, r, sigma, T = 100.0, 100.0, 0.05, 0.20, 1.0
N_PATHS = 200_000
SEED = 42

# ── Fonctions auxiliaires BS ────────────────────────────────────────────────
def N_cdf(x: float) -> float:
    """Loi normale cumulée N(x)."""
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def phi(x: float) -> float:
    """Densité de la loi normale standard φ(x)."""
    return math.exp(-0.5 * x * x) / math.sqrt(2.0 * math.pi)


def bs_greeks(S: float, K: float, r: float, sigma: float, T: float) -> dict:
    """Greeks analytiques BS pour un call européen."""
    d1 = (math.log(S / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
    d2 = d1 - sigma * math.sqrt(T)
    return {
        "price": S * N_cdf(d1) - K * math.exp(-r * T) * N_cdf(d2),
        "delta": N_cdf(d1),
        "gamma": phi(d1) / (S * sigma * math.sqrt(T)),
        "vega":  S * phi(d1) * math.sqrt(T),
        "rho":   K * T * math.exp(-r * T) * N_cdf(d2),
    }


# ── Calcul des Greeks MC ────────────────────────────────────────────────────
modele = k.GBM(s0=S0, sigma=sigma, r=r, asset="X")
call   = k.european_call("X", K, T, k.USD)

grec_mc = call.greeks(modele, n_paths=N_PATHS, seed=SEED)
grec_bs = bs_greeks(S0, K, r, sigma, T)

# ── Affichage ──────────────────────────────────────────────────────────────
print(f"Call européen ATM : S={S0}, K={K}, r={r}, σ={sigma}, T={T}")
print(f"n_paths={N_PATHS}, seed={SEED}\n")

d1 = (math.log(S0 / K) + (r + 0.5 * sigma ** 2) * T) / (sigma * math.sqrt(T))
d2 = d1 - sigma * math.sqrt(T)
print(f"  d₁ = {d1:.4f}   d₂ = {d2:.4f}   N(d₁) = {N_cdf(d1):.5f}   N(d₂) = {N_cdf(d2):.5f}\n")

print(f"{'Grecque':<8}  {'MC':>10}  {'BS analytique':>14}  {'Erreur abs.':>12}  {'Erreur rel.':>12}")
print("-" * 65)

for name, mc_val, bs_val in [
    ("Prix",  grec_mc.price, grec_bs["price"]),
    ("Δ",     grec_mc.delta, grec_bs["delta"]),
    ("Γ",     grec_mc.gamma, grec_bs["gamma"]),
    ("ν",     grec_mc.vega,  grec_bs["vega"]),
    ("ρ",     grec_mc.rho,   grec_bs["rho"]),
]:
    err_abs = abs(mc_val - bs_val)
    err_rel = err_abs / abs(bs_val) if bs_val != 0 else float("nan")
    print(f"{name:<8}  {mc_val:>10.5f}  {bs_val:>14.5f}  {err_abs:>12.5f}  {err_rel:>12.4%}")

# ── Vérifications ──────────────────────────────────────────────────────────
# Delta doit être très proche (différences finies bien conditionnées)
assert abs(grec_mc.delta - grec_bs["delta"]) < 0.02, (
    f"Delta MC ({grec_mc.delta:.5f}) trop éloigné de BS ({grec_bs['delta']:.5f})"
)

# Vega doit être dans 5 % (bump en σ, bien conditionné)
assert abs(grec_mc.vega - grec_bs["vega"]) / grec_bs["vega"] < 0.05, (
    f"Vega relatif trop éloigné de BS "
    f"({grec_mc.vega:.4f} vs {grec_bs['vega']:.4f})"
)

# Prix dans 2 % de BS
assert abs(grec_mc.price - grec_bs["price"]) / grec_bs["price"] < 0.02, (
    f"Prix MC hors tolérance BS"
)

print("\n✓ Toutes les assertions passent : Greeks MC cohérents avec BS analytique.")
