"""Note a capital protege (plancher garanti + participation).

PROFIL DE PAYOFF
----------------
A maturite T, l'investisseur recoit AU MINIMUM le notional N (capital garanti a
100 %), plus une PARTICIPATION a la hausse du sous-jacent :
    N + participation * (N/s0) * max(S_T - s0, 0)

Le terme max(S_T - s0, 0) est un call ATM ; la note est donc economiquement un
zero-coupon (plancher) + un call avec un facteur de participation.

RATIONALE INVESTISSEUR
----------------------
Profil defensif : capital protege a l'echeance, avec une exposition partielle
(participation) au rebond du sous-jacent. Le cout de la protection (la valeur
temps "perdue" par rapport a un investissement direct) finance le plancher.

CONSTRUCTION DSL
----------------
    floor = N one(USD) @ at(T)
    up    = (S - s0).clip(0) * (participation*N/s0) one(USD) @ at(T)
    note  = floor + up

PLANCHER ACTUALISE : la valeur presente de la branche garantie est N*e^{-rT}.
La PV totale doit donc etre >= ce plancher (a l'epsilon Monte-Carlo pres), et
strictement superieure des que participation > 0 (le call a une valeur > 0).
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
N = 100.0
PARTICIPATION = 0.6
DISC = math.exp(-R * T)
FLOOR = N * DISC

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")


def bs_call(s0, strike, r, sigma, t):
    d1 = (math.log(s0 / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    ncdf = lambda x: 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))
    return s0 * ncdf(d1) - strike * math.exp(-r * t) * ncdf(d2)


floor_leg = (k.const_(N) * k.one(k.USD)) @ k.at(T)
up_leg = (k.S("X") - S0).clip(0.0) * (PARTICIPATION * N / S0) * k.one(k.USD) @ k.at(T)
note = floor_leg + up_leg

res = note.price(model, n_paths=120000, seed=42, steps_per_year=50)

# Reference : N*disc + participation*(N/s0)*bs_call(ATM)
call_px = bs_call(S0, S0, R, SIGMA, T)
analytic = FLOOR + PARTICIPATION * (N / S0) * call_px

print("=" * 64)
print("NOTE A CAPITAL PROTEGE")
print("=" * 64)
print(f"  Notional N      = {N:.2f}   participation = {PARTICIPATION:.2f}")
print(f"  discount e^-rT   = {DISC:.4f}")
print(f"  Plancher N*disc  = {FLOOR:.4f}")
print(f"  BS call ATM      = {call_px:.4f}")
print()
print(f"  PV (MC)        = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95           = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print(f"  PV (analytique) = {analytic:.4f}")
print()

# --- Bornes economiques ---
EPS = 3.0 * res.std_error + 1e-6
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price >= FLOOR - EPS, "PV >= plancher N*disc (a epsilon MC)"
assert PARTICIPATION <= 0.0 or res.price > FLOOR, \
    "participation > 0 => PV strictement > plancher"
assert abs(res.price - analytic) < 0.5, "PV MC ~ reference analytique"

print("Bornes verifiees : PV >= plancher N*disc, et participation>0 => PV>plancher.")
print(f"RESUME : note capital protege -> PV = {res.price:.2f} "
      f"(plancher {FLOOR:.2f}, surplus {res.price - FLOOR:.2f} = valeur de la participation).")
