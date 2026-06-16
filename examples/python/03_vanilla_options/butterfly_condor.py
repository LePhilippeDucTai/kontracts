"""
Butterfly et Iron Condor.

Un butterfly avec calls est construit comme deux bull call spreads opposés :
    Long butterfly = Bull CS (K1, K2) - Bull CS (K2, K3)
    = Call(K1) - 2·Call(K2) + Call(K3)
    Payoff : triangle entre K1 et K3, maximum en K2

L'iron condor combine un bear call spread et un bull put spread :
    Iron Condor = short call spread(K3,K4) + short put spread(K1,K2)
    = Call(K3) - Call(K4) + Put(K2) - Put(K1)  (tous reçus en prime)
    Rentable si S_T reste dans le range [K2, K3]

Ce script illustre :
- Long butterfly via bull_call_spread(K1,K2) + (-bull_call_spread(K2,K3))
- Vérification : PV > 0 et PV < (K2-K1)·e^{-rT} (borne max payoff)
- Iron condor analogue
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
K1    = 90.0
K2    = 100.0     # strike du corps (maximum du butterfly)
K3    = 110.0
T     = 1.0
R     = 0.05
SIGMA = 0.20
N     = 100_000
SEED  = 42
DISC  = math.exp(-R * T)
WING  = K2 - K1  # largeur d'aile (K2-K1 = K3-K2 = 10)

MODEL = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")

print("=" * 60)
print("  KONTRACT — Butterfly et Iron Condor")
print("=" * 60)
print(f"\n  S0={S0}, K1={K1}, K2={K2}, K3={K3}")
print(f"  T={T}, r={R}, σ={SIGMA}")
print(f"  Largeur d'aile = {WING:.1f}")

# ---------------------------------------------------------------------------
# 1. Long Butterfly — via deux bull call spreads
# ---------------------------------------------------------------------------
print("\n--- 1. Long Butterfly Call (K1=90, K2=100, K3=110) ---")
bcs_low  = k.bull_call_spread("X", K1, K2, T, k.USD)   # long [90,100]
bcs_high = k.bull_call_spread("X", K2, K3, T, k.USD)   # long [100,110]
butterfly = bcs_low + (-bcs_high)                        # long low, short high

r_bfly = butterfly.price(MODEL, n_paths=N, seed=SEED)

# BS analytique : C(K1) - 2*C(K2) + C(K3)
bs_bfly = (bs_call(S0, K1, R, SIGMA, T)
           - 2.0 * bs_call(S0, K2, R, SIGMA, T)
           + bs_call(S0, K3, R, SIGMA, T))

print(f"  Butterfly MC         : {r_bfly.price:.4f} ± {r_bfly.std_error:.4f}")
print(f"  Butterfly BS         : {bs_bfly:.4f}")
print(f"  Erreur relative      : {abs(r_bfly.price - bs_bfly)/bs_bfly*100:.2f}%")
print(f"  Borne max payoff PV  : (K2-K1)·e^{{-rT}} = {WING * DISC:.4f}")

assert r_bfly.price > 0, "Butterfly doit avoir une valeur positive"
assert r_bfly.price < WING * DISC, "Butterfly < largeur d'aile actualisée"
assert abs(r_bfly.price - bs_bfly) / bs_bfly < 0.03
print("  [OK] 0 < Butterfly < (K2-K1)·e^{-rT}")

# ---------------------------------------------------------------------------
# 2. Long Butterfly — construction directe via 3 calls
# ---------------------------------------------------------------------------
print("\n--- 2. Butterfly via 3 calls directs ---")
call_k1 = ((k.S("X") - K1).clip(0.0) * k.one(k.USD)) @ k.at(T)
call_k2 = ((k.S("X") - K2).clip(0.0) * k.one(k.USD)) @ k.at(T)
call_k3 = ((k.S("X") - K3).clip(0.0) * k.one(k.USD)) @ k.at(T)
# C(K1) - 2*C(K2) + C(K3)
butterfly_direct = call_k1 + (-(call_k2 + call_k2)) + call_k3

r_bfly_d = butterfly_direct.price(MODEL, n_paths=N, seed=SEED)
print(f"  Butterfly direct MC  : {r_bfly_d.price:.4f} ± {r_bfly_d.std_error:.4f}")
print(f"  Butterfly spreads MC : {r_bfly.price:.4f}")
assert abs(r_bfly_d.price - r_bfly.price) < 0.2, \
    "Les deux constructions doivent donner le même prix"
print("  [OK] Deux constructions cohérentes")

# ---------------------------------------------------------------------------
# 3. Iron Condor — short call spread + short put spread
# ---------------------------------------------------------------------------
print("\n--- 3. Iron Condor (K1=85, K2=95, K3=105, K4=115) ---")
K1C, K2C, K3C, K4C = 85.0, 95.0, 105.0, 115.0

# Short call spread [K3, K4] = -(Call(K3) - Call(K4)) = Call(K4) - Call(K3)
short_call_spread = -(k.bull_call_spread("X", K3C, K4C, T, k.USD))

# Short put spread [K1, K2] = -(Put(K2) - Put(K1))
# = short put(K2) + long put(K1)
long_put_k1  = k.european_put("X", K1C, T, k.USD)
short_put_k2 = -k.european_put("X", K2C, T, k.USD)
short_put_spread = long_put_k1 + short_put_k2

iron_condor = short_call_spread + short_put_spread

r_ic = iron_condor.price(MODEL, n_paths=N, seed=SEED)

bs_ic = (
    (bs_call(S0, K4C, R, SIGMA, T) - bs_call(S0, K3C, R, SIGMA, T))  # short call spread → reçu
    + (bs_put(S0, K1C, R, SIGMA, T) - bs_put(S0, K2C, R, SIGMA, T))  # short put spread → reçu
)

print(f"  Iron Condor MC       : {r_ic.price:.4f} ± {r_ic.std_error:.4f}")
print(f"  Iron Condor BS       : {bs_ic:.4f}")
print(f"  Note : valeur positive = prime nette reçue (crédit)")

# Un iron condor est une stratégie de crédit — on reçoit une prime
# Sa valeur est positive du point de vue du vendeur
print(f"  IC positif signifie : prime reçue > risque max à T")
assert abs(r_ic.price - bs_ic) / (abs(bs_ic) + 0.01) < 0.05
print("  [OK] Iron Condor MC ≈ BS")

# ---------------------------------------------------------------------------
# 4. Profil payoff du butterfly
# ---------------------------------------------------------------------------
print("\n--- 4. Profil payoff du butterfly (K1=90, K2=100, K3=110) ---")
print(f"  {'S_T':>6}  {'Call K1':>8}  {'Call K2':>8}  {'Call K3':>8}  {'Butterfly':>10}  {'Profit':>10}")
premium = bs_bfly
for s_t in [75, 85, 90, 95, 100, 105, 110, 115, 125]:
    c1 = max(s_t - K1, 0.0)
    c2 = max(s_t - K2, 0.0)
    c3 = max(s_t - K3, 0.0)
    bfly_payoff = c1 - 2 * c2 + c3
    profit = bfly_payoff - premium
    print(f"  {s_t:6.1f}  {c1:8.2f}  {c2:8.2f}  {c3:8.2f}  {bfly_payoff:10.2f}  {profit:10.2f}")

# ---------------------------------------------------------------------------
# 5. Sensibilité à la largeur d'aile
# ---------------------------------------------------------------------------
print("\n--- 5. Sensibilité à la largeur d'aile (ATM butterfly) ---")
print(f"  {'Aile W':>8}  {'K1':>6}  {'K3':>6}  {'PV BS':>10}  {'Borne max':>12}")
for w in [5, 10, 15, 20, 25]:
    k1_w = S0 - w
    k3_w = S0 + w
    bs_w = (bs_call(S0, k1_w, R, SIGMA, T)
            - 2.0 * bs_call(S0, S0, R, SIGMA, T)
            + bs_call(S0, k3_w, R, SIGMA, T))
    borne = w * DISC
    print(f"  {w:8.0f}  {k1_w:6.1f}  {k3_w:6.1f}  {bs_w:10.4f}  {borne:12.4f}")
    assert bs_w > 0, f"Butterfly (w={w}) doit être positif"
    assert bs_w < borne, f"Butterfly (w={w}) doit être < borne max"
print("  [OK] PV butterfly dans [0, borne max] pour toutes largeurs")

# ---------------------------------------------------------------------------
# Résumé
# ---------------------------------------------------------------------------
print("\n" + "=" * 60)
print("  RÉSUMÉ")
print("=" * 60)
print(f"  Long Butterfly MC    : {r_bfly.price:.4f}  (BS: {bs_bfly:.4f})")
print(f"  Borne max PV         : {WING * DISC:.4f}  (largeur × discount)")
print(f"  Iron Condor MC       : {r_ic.price:.4f}  (BS: {bs_ic:.4f})")
print(f"  Butterfly direct MC  : {r_bfly_d.price:.4f}")
print("  Tous les asserts sont verts — script OK")
