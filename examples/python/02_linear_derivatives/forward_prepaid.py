"""
Forward et Forward Prépayé.

Un forward sur un actif est l'engagement d'acheter l'actif à une date T
au prix de livraison K. Sa valeur aujourd'hui est :

    V = S0 - K * e^{-rT}    (sans dividende)

Le forward prépayé est la version où l'acheteur paie immédiatement
et reçoit l'actif à T. Sa valeur (sans dividende) est S0.

Ce script illustre :
- k.forward(asset, K, T, ccy) — produit catalogue
- Construction manuelle du prépaid forward : (S * one) @ at(T)
- Parité spot-forward : F* = S0 * e^{rT}  (forward équitable)
"""

import math
import kontract as k

S0    = 100.0
K     = 95.0
T     = 1.0
R     = 0.05
SIGMA = 0.20
N     = 80_000
SEED  = 42

MODEL = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")
DISC  = math.exp(-R * T)
F_FAIR = S0 * math.exp(R * T)   # forward équitable (r-q=r, q=0)

print("=" * 60)
print("  KONTRACT — Forward et Forward Prépayé")
print("=" * 60)
print(f"\n  S0={S0}, K={K}, T={T}, r={R}, σ={SIGMA}")
print(f"  Discount e^{{-rT}} = {DISC:.6f}")
print(f"  Forward équitable F* = S0·e^{{rT}} = {F_FAIR:.4f}")

# ---------------------------------------------------------------------------
# 1. Forward (catalogue) avec K=95
# ---------------------------------------------------------------------------
print("\n--- 1. k.forward('X', 95, T, USD) ---")
fwd = k.forward("X", K, T, k.USD)
r_fwd = fwd.price(MODEL, n_paths=N, seed=SEED)
analytic_fwd = S0 - K * DISC
print(f"  Prix MC       : {r_fwd.price:.4f} ± {r_fwd.std_error:.4f}")
print(f"  Analytique    : S0 - K·e^{{-rT}} = {S0} - {K}·{DISC:.4f} = {analytic_fwd:.4f}")
rel_err = abs(r_fwd.price - analytic_fwd) / abs(analytic_fwd)
print(f"  Erreur rel.   : {rel_err*100:.3f}%")
assert rel_err < 0.02, f"Erreur trop grande: {rel_err:.4f}"
print("  [OK] Forward ≈ S0 - K·e^{-rT}")

# ---------------------------------------------------------------------------
# 2. Forward prépayé : (S * one) @ at(T) ≈ S0 (sans dividende)
# ---------------------------------------------------------------------------
print("\n--- 2. Prépaid Forward : (S * one) @ at(T) ---")
prepaid = (k.S("X") * k.one(k.USD)) @ k.at(T)
r_prep = prepaid.price(MODEL, n_paths=N, seed=SEED)
print(f"  Prix MC       : {r_prep.price:.4f} ± {r_prep.std_error:.4f}")
print(f"  Attendu       : S0 = {S0:.4f}  (sans dividende)")
assert abs(r_prep.price - S0) < 1.5, "Prépaid forward ≈ S0"
print("  [OK] Prépaid forward ≈ S0")

# ---------------------------------------------------------------------------
# 3. Parité spot-forward : forward(K=F*) vaut ≈ 0
# ---------------------------------------------------------------------------
print("\n--- 3. Parité : forward(K=F*) ≈ 0 ---")
fwd_fair = k.forward("X", F_FAIR, T, k.USD)
r_fair = fwd_fair.price(MODEL, n_paths=N, seed=SEED)
print(f"  K équitable   : {F_FAIR:.4f}")
print(f"  Prix MC       : {r_fair.price:.4f} ± {r_fair.std_error:.4f}")
print(f"  IC 95%%        : [{r_fair.ci95_low:.4f}, {r_fair.ci95_high:.4f}]")
# Le prix doit être proche de 0 (dans les IC)
assert r_fair.ci95_low < 0.0 < r_fair.ci95_high or abs(r_fair.price) < 0.5, \
    "Forward équitable doit valoir ≈ 0"
print("  [OK] Forward équitable ≈ 0")

# ---------------------------------------------------------------------------
# 4. Forward ITM vs OTM
# ---------------------------------------------------------------------------
print("\n--- 4. Forward ITM (K=90) vs ATM (K=100) vs OTM (K=110) ---")
for strike, label in [(90, "ITM"), (100, "ATM"), (110, "OTM")]:
    fwd_k = k.forward("X", float(strike), T, k.USD)
    r_k = fwd_k.price(MODEL, n_paths=N, seed=SEED)
    analytic_k = S0 - strike * DISC
    print(f"  K={strike} ({label}): MC={r_k.price:7.4f}  analytique={analytic_k:7.4f}")

# ---------------------------------------------------------------------------
# 5. Relation prépaid - forward
# ---------------------------------------------------------------------------
print("\n--- 5. Relation : Prépaid = Forward + K·e^{-rT} ---")
# Prepaid = Forward(K) + K * discount
reconstructed_prepaid = r_fwd.price + K * DISC
print(f"  Fwd(K={K}) + K·e^{{-rT}} = {r_fwd.price:.4f} + {K*DISC:.4f} = {reconstructed_prepaid:.4f}")
print(f"  Prépaid direct          = {r_prep.price:.4f}")
assert abs(reconstructed_prepaid - r_prep.price) < 0.5
print("  [OK] Parité prépaid-forward vérifiée")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  Forward (K=95)        : {r_fwd.price:.4f}  (analytique {analytic_fwd:.4f})")
print(f"  Prépaid forward       : {r_prep.price:.4f}  (attendu S0={S0})")
print(f"  Forward équitable F*  : {F_FAIR:.4f}  → prix ≈ {r_fair.price:.4f} ≈ 0")
print("  Parité spot-forward confirmée")
print("  Tous les asserts sont verts — script OK")
