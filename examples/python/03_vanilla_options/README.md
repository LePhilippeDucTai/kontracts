# 03 — Options vanilles

## Objectif

Illustrer les options européennes standards et leurs combinaisons classiques :
call/put, straddle/strangle, spreads, collar, butterfly et iron condor.
Chaque prix MC est comparé à Black-Scholes analytique.

## Scripts

### `european_call_put.py`

- `k.european_call` / `k.european_put` (catalogue) vs DSL direct
- Parité call-put : C - P = S0 - K·e^{-rT}
- Sensibilité à la moneyness et à la volatilité

### `straddle_strangle.py`

- `k.straddle` (catalogue) = call + put, même strike
- Strangle OTM : call(K=110) + put(K=90) via `+`
- Le straddle coûte plus cher mais le breakeven est plus proche

### `spreads_collar.py`

- `k.bull_call_spread(K_low, K_high)` vs BS : C(K_l) - C(K_h)
- Bear put spread : P(K_h) - P(K_l) via `+(-put)`
- Box spread : Bull CS + Bear PS = (K_h - K_l)·e^{-rT}
- Collar = long put OTM + short call OTM
- Risk reversal = long call OTM + short put OTM

### `butterfly_condor.py`

- Long butterfly : `bull_call_spread(K1,K2) + (-bull_call_spread(K2,K3))`
- Borne payoff : 0 < PV < (K2-K1)·e^{-rT}
- Iron condor (crédit) : short call spread + short put spread
- Profil du payoff butterfly en fonction de S_T

## Lancer les scripts

```bash
python european_call_put.py
python straddle_strangle.py
python spreads_collar.py
python butterfly_condor.py
```

## Interprétation de la sortie attendue

- **Call ATM 1Y** ≈ 10.45 : formule BS avec S0=100, K=100, r=5%, σ=20%
- **Parité C-P** ≈ 4.88 = S0 - K·e^{-r} : vérifiée à < 0.15 en MC
- **Straddle ATM** ≈ 16.02 = 2 × vega approximatif
- **Strangle OTM** ≈ 8.35 : moins cher, breakeven plus éloigné
- **Butterfly** ≈ 1.84 : coût faible, gain maximum si S_T = K2 = 100
- **Iron condor** : valeur négative (crédit) = prime reçue par le vendeur
