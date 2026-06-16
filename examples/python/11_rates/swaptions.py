"""
Swaptions Vasicek — MC vs analytique, parité payeur-receveur.

Trois vérifications pédagogiques :
  1. Swaption payeuse MC ≈ formule analytique de Jamshidian (Vasicek) — err < 5 %
  2. Swaption receveuse MC ≈ analytique (la formule retourne le prix du payeur,
     parité utilisée pour le receveur)
  3. Parité payeur-receveur :
       Payeur − Receveur = valeur actuelle du swap forward
     La valeur analytique du swap forward est :
       P(0, T_start) − P(0, T_end) − K × Σ_i δ × P(0, T_i)
"""

import kontract as k

# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
R0, A, B, SIGMA = 0.04, 0.5, 0.05, 0.012
EXPIRY    = 1.0   # option expire dans 1 an
TENOR     = 0.5   # fréquence semestrielle
N_PMT     = 4     # 4 paiements → swap de 2 ans
FIXED_K   = 0.05  # taux fixe du swap

N_PATHS = 200_000
SEED    = 42
STEPS   = 100

RM = k.vasicek(r0=R0, a=A, b=B, sigma=SIGMA)

# ---------------------------------------------------------------------------
# Construction des swaptions
# ---------------------------------------------------------------------------
sw_payer    = k.Swaption.level(EXPIRY, TENOR, N_PMT, FIXED_K, True)
sw_receiver = k.Swaption.level(EXPIRY, TENOR, N_PMT, FIXED_K, False)

# ---------------------------------------------------------------------------
# 1. Swaption payeuse — MC vs analytique
# ---------------------------------------------------------------------------
print("=" * 65)
print("  SWAPTIONS VASICEK — MC vs Analytique — Parité P/R")
print("=" * 65)
print(f"  Vasicek : r0={R0}, a={A}, b={B}, σ={SIGMA}")
print(f"  Swaption : expiry={EXPIRY}Y  tenor={TENOR}Y  N={N_PMT}  K={FIXED_K}")
print(f"  N chemins = {N_PATHS:,}  —  seed = {SEED}")

mc_payer  = k.swaption_mc(RM, sw_payer,    n_paths=N_PATHS, seed=SEED, steps=STEPS)
mc_receiv = k.swaption_mc(RM, sw_receiver, n_paths=N_PATHS, seed=SEED, steps=STEPS)
analytic  = k.vasicek_swaption_analytic(R0, A, B, SIGMA, sw_payer)

rel_payer = abs(mc_payer.price - analytic) / analytic

print(f"\n--- 1. Swaption PAYEUSE ---")
print(f"  MC         : {mc_payer.price:.6f} ± {mc_payer.std_error:.6f}")
print(f"  Analytique : {analytic:.6f}")
print(f"  Err. rel.  : {rel_payer:.4%}")
assert rel_payer < 0.05, f"Erreur payeur trop grande : {rel_payer:.4%}"
print(f"  [OK] Erreur < 5 %")

# ---------------------------------------------------------------------------
# 2. Swaption receveuse
# ---------------------------------------------------------------------------
print(f"\n--- 2. Swaption RECEVEUSE ---")
print(f"  MC receveur : {mc_receiv.price:.6f} ± {mc_receiv.std_error:.6f}")
print(f"  (pas de formule fermée directe — vérification via parité)")

# ---------------------------------------------------------------------------
# 3. Parité payeur-receveur : P - R = valeur swap forward
# ---------------------------------------------------------------------------
# Dates de paiement du swap
payment_dates = [EXPIRY + (i + 1) * TENOR for i in range(N_PMT)]

# Valeur analytique du swap forward à t=0
#   = P(0, T_start) - P(0, T_end) - K * Σ_i TENOR * P(0, T_i)
p_start = RM.discount_bond0(EXPIRY)
p_end   = RM.discount_bond0(payment_dates[-1])
annuity = sum(TENOR * RM.discount_bond0(t) for t in payment_dates)
swap_fwd_analytic = p_start - p_end - FIXED_K * annuity

mc_parity_diff = mc_payer.price - mc_receiv.price
abs_err_parity  = abs(mc_parity_diff - swap_fwd_analytic)

print(f"\n--- 3. Parité Payeur − Receveur = Swap Forward ---")
print(f"  MC (P − R)       : {mc_parity_diff:+.6f}")
print(f"  Swap fwd analyt. : {swap_fwd_analytic:+.6f}")
print(f"  |Écart|          : {abs_err_parity:.6f}")
assert abs_err_parity < 0.001, f"Parité violée : |écart| = {abs_err_parity:.6f}"
print(f"  [OK] Parité vérifiée (|écart| < 0.001)")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 65)
print("  RÉSUMÉ")
print("=" * 65)
print(f"  Swaption payeuse  MC : {mc_payer.price:.6f}  (analytique : {analytic:.6f})")
print(f"  Swaption receveuse MC: {mc_receiv.price:.6f}")
print(f"  Payeur − Receveur    : {mc_parity_diff:+.6f}  (swap fwd : {swap_fwd_analytic:+.6f})")
print(f"  Err. rel. payeur     : {rel_payer:.4%}")
print("  Tous les asserts sont verts — script OK")
