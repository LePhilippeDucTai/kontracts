"""
Spreads et Collar.

Un bull call spread est l'achat d'un call K_low et la vente d'un call K_high :
    Bull Call Spread = Call(K_low) - Call(K_high)
    Coût < call seul, gain plafonné à K_high - K_low

Un bear put spread est l'achat d'un put K_high et la vente d'un put K_low :
    Bear Put Spread = Put(K_high) - Put(K_low)

Un collar est la combinaison :
    - Long actif
    - Long put OTM (protection)
    - Short call OTM (financement)

Un risk reversal est long call OTM + short put OTM.

Ce script illustre :
- k.bull_call_spread (catalogue) vs (bs_call(K_low) - bs_call(K_high))
- Bear put spread manuel
- Collar = long put + short call
- Risk reversal = long call + short put OTM
"""

import math
import kontract as k


def _ncdf(x: float) -> float:
    return 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))


def bs_call(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return s * _ncdf(d1) - strike * math.exp(-r * t) * _ncdf(d2)


def bs_put(s: float, strike: float, r: float, sigma: float, t: float) -> float:
    d1 = (math.log(s / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    return strike * math.exp(-r * t) * _ncdf(-d2) - s * _ncdf(-d1)


# ---------------------------------------------------------------------------
# Paramètres
# ---------------------------------------------------------------------------
S0    = 100.0
K_LOW = 95.0
K_HIGH = 105.0
T     = 1.0
R     = 0.05
SIGMA = 0.20
N     = 100_000
SEED  = 42
DISC  = math.exp(-R * T)

MODEL = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

print("=" * 60)
print("  KONTRACT — Spreads et Collar")
print("=" * 60)
print(f"\n  S0={S0}, K_low={K_LOW}, K_high={K_HIGH}, T={T}, r={R}, σ={SIGMA}")

# ---------------------------------------------------------------------------
# 1. Bull Call Spread — catalogue vs BS
# ---------------------------------------------------------------------------
print("\n--- 1. Bull Call Spread (K_low=95, K_high=105) ---")
bcs_cat = k.bull_call_spread("X", K_LOW, K_HIGH, T, k.USD)
r_bcs   = bcs_cat.price(MODEL, n_paths=N, seed=SEED)
bs_bcs  = bs_call(S0, K_LOW, R, SIGMA, T) - bs_call(S0, K_HIGH, R, SIGMA, T)

print(f"  Bull Call Spread MC  : {r_bcs.price:.4f} ± {r_bcs.std_error:.4f}")
print(f"  BS analytique        : {bs_bcs:.4f}")
print(f"  Erreur relative      : {abs(r_bcs.price - bs_bcs)/bs_bcs*100:.2f}%")
print(f"  Borne max payoff     : {K_HIGH - K_LOW:.1f}  (spread de strikes)")
print(f"  Borne max PV         : {(K_HIGH - K_LOW) * DISC:.4f}  (actualisé)")
assert abs(r_bcs.price - bs_bcs) / bs_bcs < 0.02
assert r_bcs.price > 0, "Bull call spread doit être positif"
assert r_bcs.price < (K_HIGH - K_LOW) * DISC, "Bull call spread < PV du spread max"
print("  [OK] 0 < Bull CS < (K_high-K_low)·e^{-rT}")

# ---------------------------------------------------------------------------
# 2. Bear Put Spread — manuel
# ---------------------------------------------------------------------------
print("\n--- 2. Bear Put Spread (K_low=95, K_high=105) ---")
put_high = k.european_put("X", K_HIGH, T, k.USD)
put_low  = k.european_put("X", K_LOW, T, k.USD)
bear_put = put_high + (-put_low)   # long put high + short put low

r_bps   = bear_put.price(MODEL, n_paths=N, seed=SEED)
bs_bps  = bs_put(S0, K_HIGH, R, SIGMA, T) - bs_put(S0, K_LOW, R, SIGMA, T)

print(f"  Bear Put Spread MC   : {r_bps.price:.4f} ± {r_bps.std_error:.4f}")
print(f"  BS analytique        : {bs_bps:.4f}")
print(f"  Erreur relative      : {abs(r_bps.price - bs_bps)/bs_bps*100:.2f}%")
assert abs(r_bps.price - bs_bps) / bs_bps < 0.02
assert r_bps.price > 0
print("  [OK] Bear Put Spread MC ≈ BS")

# ---------------------------------------------------------------------------
# 3. Relation put-call spread (parité) — vérifiée analytiquement
# ---------------------------------------------------------------------------
print("\n--- 3. Parité bull CS + bear PS (vérification analytique) ---")
# De la parité C-P = S0 - K·e^{-rT} :
# Bull CS + Bear PS = (K_high - K_low) * e^{-rT}
# Preuve :
#   Bull CS = C(K_l) - C(K_h)
#   Bear PS = P(K_h) - P(K_l)
#   Bull CS + Bear PS = C(K_l) - C(K_h) + P(K_h) - P(K_l)
#     = [C(K_l) + P(K_h)] - [C(K_h) + P(K_l)]
# De la parité : C(K)+P(K) = S0 + K·e^{-rT} - 2·K·e^{-rT} + 2·P  ← complexe
# Plus simple : par les bornes d'arbitrage des call spreads :
#   BCS + BPS = (K_h-K_l)·e^{-rT}  (identité box spread)
sum_th = (K_HIGH - K_LOW) * DISC
sum_bs = bs_bcs + bs_bps
print(f"  Bull CS BS        : {bs_bcs:.4f}")
print(f"  Bear PS BS        : {bs_bps:.4f}")
print(f"  Bull CS + Bear PS (BS): {sum_bs:.4f}")
print(f"  (K_high-K_low)·e^{{-rT}}: {sum_th:.4f}")
print(f"  Cette identité est celle du box spread (arbitrage sans risque)")
assert abs(sum_bs - sum_th) < 0.01, f"Box spread violé : {sum_bs:.4f} vs {sum_th:.4f}"
print("  [OK] Box spread : Bull CS + Bear PS = (K_h-K_l)·e^{-rT}")

# ---------------------------------------------------------------------------
# 4. Collar = long put OTM + short call OTM
# ---------------------------------------------------------------------------
print("\n--- 4. Collar : long put(K=90) + short call(K=110) ---")
K_PUT_COLLAR  = 90.0
K_CALL_COLLAR = 110.0

long_put  = k.european_put("X", K_PUT_COLLAR, T, k.USD)
short_call = -k.european_call("X", K_CALL_COLLAR, T, k.USD)   # give = short
collar    = long_put + short_call

r_collar   = collar.price(MODEL, n_paths=N, seed=SEED)
bs_long_p  = bs_put(S0, K_PUT_COLLAR, R, SIGMA, T)
bs_short_c = bs_call(S0, K_CALL_COLLAR, R, SIGMA, T)
bs_collar  = bs_long_p - bs_short_c

print(f"  Long put(90)  BS     : {bs_long_p:.4f}")
print(f"  Short call(110) BS   : {-bs_short_c:.4f}")
print(f"  Collar net BS        : {bs_collar:.4f}")
print(f"  Collar MC            : {r_collar.price:.4f} ± {r_collar.std_error:.4f}")

# Le collar peut être positif ou négatif selon le skew
print(f"  Note : collar >0 = protection coûte net positive")
print(f"         collar <0 = prime de call > prime put (net crédit)")
assert abs(r_collar.price - bs_collar) / (abs(bs_collar) + 0.01) < 0.05
print("  [OK] Collar MC ≈ BS")

# ---------------------------------------------------------------------------
# 5. Risk Reversal = long call OTM + short put OTM
# ---------------------------------------------------------------------------
print("\n--- 5. Risk Reversal : long call(110) - put(90) ---")
long_call_otm = k.european_call("X", K_CALL_COLLAR, T, k.USD)
short_put_otm = -k.european_put("X", K_PUT_COLLAR, T, k.USD)
risk_reversal = long_call_otm + short_put_otm

r_rr  = risk_reversal.price(MODEL, n_paths=N, seed=SEED)
bs_rr = bs_call(S0, K_CALL_COLLAR, R, SIGMA, T) - bs_put(S0, K_PUT_COLLAR, R, SIGMA, T)

print(f"  Risk Reversal MC     : {r_rr.price:.4f} ± {r_rr.std_error:.4f}")
print(f"  Risk Reversal BS     : {bs_rr:.4f}")
assert abs(r_rr.price - bs_rr) / (abs(bs_rr) + 0.01) < 0.05
print("  [OK] Risk Reversal MC ≈ BS")

# ---------------------------------------------------------------------------
# 6. Profil de payoff du collar
# ---------------------------------------------------------------------------
print("\n--- 6. Profil payoff collar (longue position) ---")
print(f"  S_T : payoff collar (sans actif, net des primes={bs_collar:.4f})")
print(f"  {'S_T':>6}  {'Put OTM':>10}  {'Short call':>12}  {'Collar net':>12}")
for s_t in [70, 80, 90, 100, 110, 120, 130]:
    p_payoff  = max(K_PUT_COLLAR - s_t, 0.0)
    c_payoff  = -max(s_t - K_CALL_COLLAR, 0.0)
    col_net   = p_payoff + c_payoff
    print(f"  {s_t:6.1f}  {p_payoff:10.2f}  {c_payoff:12.2f}  {col_net:12.2f}")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  Bull Call Spread MC  : {r_bcs.price:.4f}  (BS: {bs_bcs:.4f})")
print(f"  Bear Put Spread MC   : {r_bps.price:.4f}  (BS: {bs_bps:.4f})")
print(f"  Collar MC            : {r_collar.price:.4f}  (BS: {bs_collar:.4f})")
print(f"  Risk Reversal MC     : {r_rr.price:.4f}  (BS: {bs_rr:.4f})")
print("  Tous les asserts sont verts — script OK")
