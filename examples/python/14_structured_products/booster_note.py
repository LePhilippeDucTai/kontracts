"""Booster Note (capital + levier 2x plafonne).

PROFIL DE PAYOFF
----------------
Note a capital garanti offrant un effet de LEVIER (2x) sur la hausse du
sous-jacent, mais PLAFONNE par un cap. A maturite :
    N + 2 * [ max(S_T - s0, 0) - max(S_T - cap, 0) ]

Le terme entre crochets est un bull call spread (call long au strike s0, call
short au strike cap) : le gain accelere a 2x entre s0 et cap, puis sature. Le
capital N est protege a 100 %.

RATIONALE INVESTISSEUR
----------------------
Vue haussiere : on amplifie (x2) la performance dans une zone cible [s0, cap],
ce qui maximise le rendement d'un rebond modere, tout en gardant le capital
protege. Le plafond (cap) finance le levier.

CONSTRUCTION DSL
----------------
    capital = N one(USD) @ at(T)
    boost   = const_(2.0) * bull_call_spread("X", s0, cap, T, USD)   # levier 2x
    note    = capital + boost

`Observable * Contract` (`const_(2.0) * spread`) applique le combinateur `scale`
(facteur de levier). L'horizon T est porte par `@ at(T)`.
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
N = 100.0
CAP = 120.0       # plafond du bull call spread
LEVERAGE = 2.0
DISC = math.exp(-R * T)
FLOOR = N * DISC

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")


def bs_call(s0, strike, r, sigma, t):
    d1 = (math.log(s0 / strike) + (r + 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    d2 = d1 - sigma * math.sqrt(t)
    ncdf = lambda x: 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))
    return s0 * ncdf(d1) - strike * math.exp(-r * t) * ncdf(d2)


capital = (k.const_(N) * k.one(k.USD)) @ k.at(T)
boost = k.const_(LEVERAGE) * k.bull_call_spread("X", S0, CAP, T, k.USD)
note = capital + boost

res = note.price(model, n_paths=120000, seed=42, steps_per_year=50)

# Reference : N*disc + leverage * (bs_call(s0) - bs_call(cap))
spread_px = bs_call(S0, S0, R, SIGMA, T) - bs_call(S0, CAP, R, SIGMA, T)
analytic = FLOOR + LEVERAGE * spread_px

print("=" * 64)
print("BOOSTER NOTE (levier 2x plafonne)")
print("=" * 64)
print(f"  Notional N    = {N:.2f}   levier = {LEVERAGE:.1f}x")
print(f"  Zone boost [s0,cap] = [{S0:.1f}, {CAP:.1f}]   discount e^-rT = {DISC:.4f}")
print(f"  Plancher N*disc = {FLOOR:.4f}")
print(f"  Bull call spread (BS) = {spread_px:.4f}")
print()
print(f"  PV (MC)        = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95           = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print(f"  PV (analytique) = {analytic:.4f}")
print()

# --- Bornes economiques ---
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price > FLOOR, "le booster (spread leverage) ajoute une valeur > plancher"
assert abs(res.price - analytic) < 0.5, "PV MC ~ reference analytique"

print("Bornes verifiees : PV > plancher N*disc, et PV ~ analytique.")
print(f"RESUME : booster note -> PV = {res.price:.2f} "
      f"(plancher {FLOOR:.2f} + boost 2x ~{res.price - FLOOR:.2f}).")
