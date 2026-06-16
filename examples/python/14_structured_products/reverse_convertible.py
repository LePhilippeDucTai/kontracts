"""Reverse Convertible (coupon eleve, capital a risque).

PROFIL DE PAYOFF
----------------
L'investisseur recoit a maturite T le notional N MAJORE d'un coupon eleve
(N + coupon), MAIS il est vendeur d'un put de strike K sur le sous-jacent, pour
un notional N/K (soit N/K actions). A l'echeance :
    - si S_T >= K : il recoit N + coupon (le put expire sans valeur),
    - si S_T <  K : il subit la perte du put, (K - S_T) * (N/K), pouvant
      eroder voire annuler le capital.

Le payoff terminal vaut donc :
    N + coupon - max(K - S_T, 0) * (N/K)

RATIONALE INVESTISSEUR
----------------------
Le coupon eleve remunere la vente de la protection a la baisse. Produit adapte
a une vue neutre/legerement haussiere : on encaisse le coupon tant que le
sous-jacent ne chute pas sous K.

CONSTRUCTION DSL
----------------
    bond_coupon = (N+coupon) one(USD) @ at(T)
    short_put   = -( (K - S).clip(0) * (N/K) one(USD) @ at(T) )
    note        = bond_coupon + short_put

Le `-(...)` est le combinateur `give` (inversion de signe), qui encode la
position VENDEUSE de put.
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
N = 100.0
COUPON = 8.0
K = 100.0  # strike du put short (ATM)
DISC = math.exp(-R * T)

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")


def bs_put(s0, strike, r, sigma, t):
    """Reference Black-Scholes pour le put (sans dividende)."""
    d1 = (math.log(s0 / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    ncdf = lambda x: 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))
    return strike * math.exp(-r * t) * ncdf(-d2) - s0 * ncdf(-d1)


bond_coupon = (k.const_(N + COUPON) * k.one(k.USD)) @ k.at(T)
short_put = -(((k.const_(K) - k.S("X")).clip(0.0) * (N / K)) * k.one(k.USD) @ k.at(T))
note = bond_coupon + short_put

res = note.price(model, n_paths=120000, seed=42, steps_per_year=50)

# Reference analytique : (N+coupon)*disc - (N/K)*bs_put(K)
put_px = bs_put(S0, K, R, SIGMA, T)
analytic = (N + COUPON) * DISC - (N / K) * put_px

print("=" * 64)
print("REVERSE CONVERTIBLE")
print("=" * 64)
print(f"  Notional N    = {N:.2f}   coupon = {COUPON:.2f}   strike put K = {K:.2f}")
print(f"  discount e^-rT = {DISC:.4f}")
print(f"  BS put(K)     = {put_px:.4f}   short put notionnel N/K = {N / K:.4f}")
print()
print(f"  PV (MC)       = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95          = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print(f"  PV (analytique) = {analytic:.4f}")
print(f"  cap (N+coupon)*disc = {(N + COUPON) * DISC:.4f}")
print()

# --- Bornes economiques ---
cap = (N + COUPON) * DISC
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price < cap, "PV < (N+coupon)*disc (la vente de put reduit la valeur)"
assert abs(res.price - analytic) < 0.5, "PV MC ~ reference analytique"

print("Bornes verifiees : 0 < PV < (N+coupon)*disc, et PV ~ analytique.")
print(f"RESUME : reverse convertible -> PV = {res.price:.2f} "
      f"(le coupon de {COUPON:.0f} remunere la vente de protection).")
