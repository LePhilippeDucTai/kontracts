"""Note Corridor (coupon digital conditionnel a un range).

PROFIL DE PAYOFF
----------------
A maturite T, l'investisseur recoit le notional N, plus un coupon DIGITAL verse
uniquement si le sous-jacent termine DANS le corridor [L, H] :
    N + coupon * 1{ L <= S_T <= H }

Le coupon est "tout ou rien" : present si S_T appartient a [L, H] a l'echeance,
nul sinon. Le capital N est garanti.

RATIONALE INVESTISSEUR
----------------------
Pari sur la STABILITE du sous-jacent dans une fourchette. Si l'investisseur
anticipe que le marche restera dans [L, H], il encaisse le coupon ; sinon il ne
perd que le rendement (capital protege).

CONSTRUCTION DSL
----------------
    capital = N one(USD) @ at(T)
    digital = coupon one(USD) @ ( (S>=L) & (S<=H) ) @ at(T)
    note    = capital + digital

La condition `(S>=L) & (S<=H)` combine deux comparaisons via l'operateur `&`.
L'horizon T est fourni par les `@ at(T)`.

LIMITE IMPORTANTE (mono-fixing)
-------------------------------
Cette note observe le corridor a UNE SEULE date (l'echeance T). Un vrai RANGE
ACCRUAL accumulerait le coupon proportionnellement au nombre de jours passes
dans le corridor sur toute la vie du produit. Cela necessiterait une observable
"indicatrice moyennee" sur le temps, qui n'est PAS exprimable dans l'algebre
actuelle (pas de moyenne d'indicatrice de barriere). On approxime donc le range
accrual par une digitale mono-fixing a maturite.
"""

import math

import kontract as k

S0 = 100.0
SIGMA = 0.2
R = 0.05
T = 1.0
N = 100.0
COUPON = 8.0
L = 90.0
H = 115.0
DISC = math.exp(-R * T)
FLOOR = N * DISC

model = k.GBM(s0=S0, sigma=SIGMA, r=R, asset="X")


def bs_digital_range(s0, lo, hi, r, sigma, t):
    """Cash-or-nothing analytique : disc * P(lo <= S_T <= hi)."""
    ncdf = lambda x: 0.5 * (1.0 + math.erf(x / math.sqrt(2.0)))
    def d2(strike):
        return (math.log(s0 / strike) + (r - 0.5 * sigma**2) * t) / (sigma * math.sqrt(t))
    # Sous la mesure risque-neutre, P(S_T <= strike) = N(-d2(strike)).
    # P(lo <= S_T <= hi) = P(S_T <= hi) - P(S_T <= lo) = N(-d2(hi)) - N(-d2(lo))
    prob = ncdf(-d2(hi)) - ncdf(-d2(lo))
    return math.exp(-r * t) * prob


capital = (k.const_(N) * k.one(k.USD)) @ k.at(T)
digital = (k.const_(COUPON) * k.one(k.USD)) @ ((k.S("X") >= L) & (k.S("X") <= H)) @ k.at(T)
note = capital + digital

res = note.price(model, n_paths=150000, seed=42, steps_per_year=50)

prob_disc = bs_digital_range(S0, L, H, R, SIGMA, T)
analytic = FLOOR + COUPON * prob_disc

print("=" * 64)
print("NOTE CORRIDOR (digital range, mono-fixing)")
print("=" * 64)
print(f"  Notional N    = {N:.2f}   coupon = {COUPON:.2f}")
print(f"  Corridor [L,H] = [{L:.1f}, {H:.1f}]   discount e^-rT = {DISC:.4f}")
print(f"  Plancher N*disc = {FLOOR:.4f}")
print(f"  P(L<=S_T<=H)*disc = {prob_disc:.4f}")
print()
print(f"  PV (MC)        = {res.price:.4f}  +/- {res.std_error:.4f}")
print(f"  IC95           = [{res.ci95_low:.4f}, {res.ci95_high:.4f}]")
print(f"  PV (analytique) = {analytic:.4f}")
print()

# --- Bornes economiques ---
cap = FLOOR + COUPON * DISC
assert res.price > 0.0, "PV doit etre strictement positive"
assert res.price > FLOOR, "le coupon digital ajoute une valeur > 0"
assert res.price < cap, "PV < plancher + coupon*disc (la digitale n'est pas certaine)"
assert abs(res.price - analytic) < 0.5, "PV MC ~ reference analytique"

print("Bornes verifiees : plancher < PV < plancher + coupon*disc, PV ~ analytique.")
print(f"RESUME : note corridor -> PV = {res.price:.2f} "
      f"(plancher {FLOOR:.2f}, esperance du coupon digital ~{res.price - FLOOR:.2f}).")
