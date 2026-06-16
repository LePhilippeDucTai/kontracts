"""
Options FX — Garman-Kohlhagen, Forward FX, Quanto.

Quatre vérifications pédagogiques :
  1. Parité put-call Garman-Kohlhagen :
       C − P = X0·e^{−r_f·T} − K·e^{−r_d·T}   (exacte à la machine)
  2. Forward FX vs intérêt-parité :
       FX_fwd = X0 · e^{(r_d − r_f)·T}
  3. Prix call et put GK (affichage)
  4. Quanto call : monotone décroissant en ρ (corrélation spot/FX)
"""

import math
import kontract as k

# ---------------------------------------------------------------------------
# Paramètres GK
# ---------------------------------------------------------------------------
X0    = 1.20   # spot EUR/USD
K     = 1.25   # strike
T     = 1.0    # maturité 1 an
R_D   = 0.04   # taux domestique (USD)
R_F   = 0.01   # taux étranger  (EUR)
SIGMA = 0.10   # volatilité implicite

# ---------------------------------------------------------------------------
# 1. Prix call et put GK
# ---------------------------------------------------------------------------
gk_call = k.garman_kohlhagen_call(X0, K, T, R_D, R_F, SIGMA)
gk_put  = k.garman_kohlhagen_put(X0, K, T, R_D, R_F, SIGMA)

print("=" * 60)
print("  OPTIONS FX — Garman-Kohlhagen / Forward / Quanto")
print("=" * 60)
print(f"  Paramètres : X0={X0}, K={K}, T={T}Y, r_d={R_D}, r_f={R_F}, σ={SIGMA}")
print(f"\n--- 1. Prix GK ---")
print(f"  Call GK  : {gk_call:.8f}")
print(f"  Put  GK  : {gk_put:.8f}")

# ---------------------------------------------------------------------------
# 2. Parité put-call GK : C − P = X0·e^{−r_f·T} − K·e^{−r_d·T}
# ---------------------------------------------------------------------------
parity_rhs = X0 * math.exp(-R_F * T) - K * math.exp(-R_D * T)
parity_err = abs((gk_call - gk_put) - parity_rhs)

print(f"\n--- 2. Parité Put-Call GK ---")
print(f"  C − P             : {gk_call - gk_put:.10f}")
print(f"  X0·e^−r_f − K·e^−r_d : {parity_rhs:.10f}")
print(f"  |Erreur parité|   : {parity_err:.2e}")
assert parity_err < 1e-9, f"Parité violée : {parity_err:.2e}"
print(f"  [OK] Parité exacte (erreur < 1e-9)")

# ---------------------------------------------------------------------------
# 3. Forward FX vs intérêt-parité
# ---------------------------------------------------------------------------
fx_fwd         = k.fx_forward(X0, T, R_D, R_F)
irp_analytic   = X0 * math.exp((R_D - R_F) * T)
fwd_rel_err    = abs(fx_fwd - irp_analytic) / irp_analytic

print(f"\n--- 3. Forward FX vs Intérêt-Parité ---")
print(f"  k.fx_forward     : {fx_fwd:.8f}")
print(f"  X0·e^(r_d−r_f)T  : {irp_analytic:.8f}")
print(f"  Erreur relative  : {fwd_rel_err:.2e}")
assert fwd_rel_err < 1e-9, f"IRP violée : {fwd_rel_err:.2e}"
print(f"  [OK] Forward = IRP analytique (erreur < 1e-9)")

# ---------------------------------------------------------------------------
# 4. Quanto call — monotone décroissant en ρ
#    Intuition : une corrélation spot/FX élevée réduit l'ajustement quanto
#    (dividend ajusté q_s → r_d − r_f − ρ·σ_S·σ_X)
# ---------------------------------------------------------------------------
S0, KQ, RD, RF, Q_S, SIG_S, SIG_X = 100.0, 100.0, 0.04, 0.02, 0.0, 0.25, 0.15

RHO_LOW  = -0.50
RHO_HIGH = +0.50

quanto_low  = k.quanto_call(S0, KQ, T, RD, RF, Q_S, SIG_S, SIG_X, RHO_LOW)
quanto_high = k.quanto_call(S0, KQ, T, RD, RF, Q_S, SIG_S, SIG_X, RHO_HIGH)

print(f"\n--- 4. Quanto Call — Monotonie en ρ ---")
print(f"  S0={S0}, K={KQ}, T={T}Y, σ_S={SIG_S}, σ_X={SIG_X}")
print(f"  k.quanto_call (ρ={RHO_LOW:+.2f}) : {quanto_low:.6f}")
print(f"  k.quanto_call (ρ={RHO_HIGH:+.2f}) : {quanto_high:.6f}")
print(f"  ρ↑  →  quanto↓ ?  {quanto_low > quanto_high}")
assert quanto_low > quanto_high, (
    f"Quanto non décroissant en ρ : {quanto_low:.6f} ≤ {quanto_high:.6f}"
)
print(f"  [OK] Quanto décroissant en ρ")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  GK call / put     : {gk_call:.6f} / {gk_put:.6f}")
print(f"  Parité |C−P−rhs|  : {parity_err:.2e}  [OK < 1e-9]")
print(f"  FX forward        : {fx_fwd:.6f}  (IRP : {irp_analytic:.6f})")
print(f"  Quanto ρ={RHO_LOW:+.2f}     : {quanto_low:.6f}")
print(f"  Quanto ρ={RHO_HIGH:+.2f}     : {quanto_high:.6f}  (décroissant [OK])")
print("  Tous les asserts sont verts — script OK")
