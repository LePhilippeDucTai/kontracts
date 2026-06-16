"""
Modèle Rough Bergomi — volatilité rugueuse
==========================================
Illustre le modèle Rough Bergomi (Bayer, Friz, Gatheral 2016) avec exposant
de Hurst H = 0.1 (vol rugueuse), et montre l'effet du levier ρ < 0.

Modèle Rough Bergomi :
    dS_t / S_t = sqrt(V_t) dW_t^1
    V_t = V_0 exp(η ∫_0^t (t-s)^(H-1/2) dW_s^2 - η²/2 · t^(2H))
    corr(dW^1, dW^2) = ρ

Paramètres clés :
  - H : exposant de Hurst. H < 0.5 → volatilité rugueuse (non-Markovienne).
    H = 0.1 est cohérent avec les observations de marché (Gatheral 2018).
  - ρ < 0 : corrélation négative spot-vol (levier). Génère un skew négatif :
    les puts OTM coûtent plus cher que les calls OTM en vol implicite.
  - η (xi) : niveau de vol initiale.

Cette script :
  1. Price un call ATM 1 an sous Rough Bergomi et affiche le prix + IC 95 %.
  2. Compare call OTM (K=110) vs put OTM (K=90) pour illustrer le skew.
  3. Montre la dépendance à ρ : avec ρ = 0 le smile est symétrique.
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
v0  = 0.04    # variance initiale (vol 20 %)
xi  = 0.04    # niveau de vol-variance (variance forward)
H   = 0.10    # exposant de Hurst (rugosité de la vol)
rho_neg = -0.70  # levier négatif (typique actions)
rho_sym =  0.00  # pas de levier (smile symétrique)

PATHS = 60_000   # rBergomi est plus lent — 60k est un bon compromis
STEPS = 100
SEED  =  42

# Modèles
model_rho_neg = k.rough_bergomi(
    spot=S0, v0=v0, xi=xi, h=H, rho=rho_neg, r=r, asset="X"
)
model_rho_sym = k.rough_bergomi(
    spot=S0, v0=v0, xi=xi, h=H, rho=rho_sym, r=r, asset="X"
)

# Contrats
S = k.S("X")
call_atm  = k.european_call("X", 100.0, T, k.USD)
call_otm  = (S - k.const_(110.0)).clip(0.0) * k.one(k.USD) @ k.at(T)
put_otm   = (k.const_(90.0) - S).clip(0.0)  * k.one(k.USD) @ k.at(T)
call_dotm = (S - k.const_(120.0)).clip(0.0) * k.one(k.USD) @ k.at(T)
put_dotm  = (k.const_(80.0) - S).clip(0.0)  * k.one(k.USD) @ k.at(T)

print("=" * 70)
print("  Modèle Rough Bergomi — vol rugueuse, H = 0.1")
print("=" * 70)
print(f"\nParamètres : v0={v0} (vol 20%), xi={xi}, H={H}, r={r}")
print(f"  n_paths={PATHS:,}, steps/an={STEPS}, seed={SEED}")

# ---------------------------------------------------------------------------
# Partie 1 : Call ATM avec intervalle de confiance
# ---------------------------------------------------------------------------
print()
print("Partie 1 — Call ATM 1 an, prix et intervalle de confiance 95 %")

res_atm = call_atm.price(model_rho_neg, n_paths=PATHS, seed=SEED, steps_per_year=STEPS)
bs_atm  = bs_call(S0, 100.0, T, r, math.sqrt(v0))

print(f"  Prix Rough Bergomi (ρ={rho_neg}) : {res_atm.price:.4f}")
print(f"  Erreur standard               : {res_atm.std_error:.4f}")
print(f"  IC 95 %                       : [{res_atm.ci95_low:.4f}, {res_atm.ci95_high:.4f}]")
print(f"  BS(20 %) référence            : {bs_atm:.4f}")
print(f"  Ecart relatif                 : {(res_atm.price - bs_atm)/bs_atm*100:+.2f}%")

# ---------------------------------------------------------------------------
# Partie 2 : Skew — ρ < 0 vs ρ = 0
# ---------------------------------------------------------------------------
print()
print("Partie 2 — Skew : ρ négatif vs symétrique")
print(f"  ρ = {rho_neg} (levier négatif — actions) vs ρ = {rho_sym} (symétrique)")
print()

def price_all(model) -> dict:
    return {
        "atm":  call_atm.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS).price,
        "co":   call_otm.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS).price,
        "po":   put_otm.price( model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS).price,
        "cdotm":call_dotm.price(model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS).price,
        "pdotm":put_dotm.price( model, n_paths=PATHS, seed=SEED, steps_per_year=STEPS).price,
    }

p_neg = price_all(model_rho_neg)
p_sym = price_all(model_rho_sym)

sigma_ref = math.sqrt(v0)
bs_vals = {
    "atm":   bs_call(S0, 100.0, T, r, sigma_ref),
    "co":    bs_call(S0, 110.0, T, r, sigma_ref),
    "po":    bs_put( S0,  90.0, T, r, sigma_ref),
    "cdotm": bs_call(S0, 120.0, T, r, sigma_ref),
    "pdotm": bs_put( S0,  80.0, T, r, sigma_ref),
}

labels = [
    ("Call ATM  K=100", "atm"),
    ("Call OTM  K=110", "co"),
    ("Call DOTM K=120", "cdotm"),
    ("Put  OTM  K=90 ", "po"),
    ("Put  DOTM K=80 ", "pdotm"),
]

print(f"{'Produit':<24} {'BS(20%)':<10} {'rBerg ρ=-0.7':<14} {'rBerg ρ=0.0':<13} {'Δ(ρ neg-sym)'}")
print("-" * 72)
for label, key in labels:
    bs_v = bs_vals[key]
    delta_rho = p_neg[key] - p_sym[key]
    print(f"  {label:<22} {bs_v:<10.4f} {p_neg[key]:<14.4f} {p_sym[key]:<13.4f} {delta_rho:+.4f}")

print()
print("Commentaire :")
print("  - H = 0.1 (rugosité) : la volatilité est elle-même très fluctuante")
print("    à court terme, ce qui produit un terme structure de vol implicite")
print("    en 'power law' pour les courtes maturités, très cohérent avec le marché.")
print("  - ρ = -0.7 (levier négatif) : quand le spot baisse, la vol monte.")
print("    → Les puts OTM deviennent significativement plus chers (skew négatif).")
print("    → Les calls OTM peuvent être moins chers ou similaires.")
print("  - ρ = 0 : smile symétrique — put OTM ≈ call OTM (en vol implicite).")
print("  - La différence Δ(ρ neg - sym) mesure l'intensité du skew induit par ρ.")
print("  - Note : rBergomi est plus lent que GBM/Heston car il simule un")
print("    processus fractionnaire (mémoire longue) → n_paths = 60k.")

# ---------------------------------------------------------------------------
# Assertions
# ---------------------------------------------------------------------------
# Le prix ATM doit être dans l'IC 95%
assert res_atm.ci95_low <= res_atm.price <= res_atm.ci95_high, \
    "Le prix doit être dans son propre IC (invariant trivial)"

# Relative à ρ symétrique, ρ négatif doit augmenter les puts OTM plus que les calls OTM
# (on mesure le signe de l'enrichissement relatif put/call sous levier négatif)
delta_put  = p_neg["po"]  - p_sym["po"]
delta_call = p_neg["co"]  - p_sym["co"]
assert delta_put > delta_call, (
    f"ρ<0 devrait enrichir davantage les puts OTM que les calls OTM "
    f"(Δput={delta_put:.4f}, Δcall={delta_call:.4f})"
)

# Prix positifs partout
for label, key in labels:
    assert p_neg[key] > 0, f"{label}: prix doit être > 0"

print()
print("Assertions OK — levier ρ<0 enrichit les puts OTM > calls OTM (skew négatif).")
