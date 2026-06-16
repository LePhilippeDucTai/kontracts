"""Certificat Discount (Discount Certificate) = Covered Call.

PROFIL DE PAYOFF
----------------
L'investisseur achete le sous-jacent avec une DECOTE (discount), en echange du
plafonnement de son gain au strike K (le "cap"). Payoff terminal :
    min(S_T, K) = S_T - max(S_T - K, 0)

C'est exactement une position d'action prepayee MOINS un call vendu de strike K
(strategie "covered call"). La decote a l'achat correspond a la prime du call
encaissee.

RATIONALE INVESTISSEUR
----------------------
Vue neutre a legerement haussiere : on accepte de plafonner le gain pour
acheter moins cher que le spot. Tant que S_T <= K, on encaisse la decote ;
au-dela, le gain est capte par l'emetteur (call vendu).

CONSTRUCTION DSL
----------------
    stock = S one(USD) @ at(T)
    note  = stock + ( -european_call("X", K, T, USD) )

Le `-call` est le combinateur `give` : position VENDEUSE de call. La somme
reproduit min(S_T, K).

REFERENCE ANALYTIQUE
--------------------
PV = s0 - bs_call(K). Comme bs_call(K) > 0, on a strictement PV < s0 : c'est la
DECOTE du certificat.
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
K = 110.0  # cap (strike du call vendu)
DISC = math.exp(-R * T)

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")


def bs_call(s0, strike, r, sigma, t):
    d1 = (math.log(s0 / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    ncdf = lambda x: 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))
    return s0 * ncdf(d1) - strike * math.exp(-r * t) * ncdf(d2)


stock = (k.S("X") * k.one(k.USD)) @ k.at(T)
note = stock + (-k.european_call("X", K, T, k.USD))

res = note.price(model, n_paths=120000, seed=42, steps_per_year=50)

call_px = bs_call(S0, K, R, SIGMA, T)
analytic = S0 - call_px  # forward actualise (=s0) moins prime call

print("=" * 64)
print("CERTIFICAT DISCOUNT (COVERED CALL)")
print("=" * 64)
print(f"  Spot s0   = {S0:.2f}   cap K = {K:.2f}   discount e^-rT = {DISC:.4f}")
print(f"  BS call(K) = {call_px:.4f}  (= la decote a l'achat)")
print()
print(f"  PV (MC)        = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95           = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print(f"  PV (analytique) = s0 - bs_call(K) = {analytic:.4f}")
print(f"  Decote vs spot  = s0 - PV = {S0 - res.price:.4f}")
print()

# --- Bornes economiques ---
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price < S0, "PV < s0 : le certificat se traite avec une decote"
assert abs(res.price - analytic) < 0.5, "PV MC ~ s0 - bs_call(K)"

print("Bornes verifiees : PV ~ s0 - bs_call(K) et PV < s0 (decote).")
print(f"RESUME : certificat discount -> PV = {res.price:.2f} "
      f"(decote de {S0 - res.price:.2f} vs spot {S0:.0f}, gain plafonne a K={K:.0f}).")
