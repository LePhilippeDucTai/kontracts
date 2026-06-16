# 13 — Calibration

Inversion de prix de marché pour extraire la volatilité implicite ou calibrer un modèle GBM.

## Scripts

| Fichier | Contenu |
|---|---|
| `implied_vol.py` | Prix BS → `k.implied_volatility` → σ implicite ; surface de vol plate |
| `fit_gbm_volatility.py` | Prix cible → `k.fit_gbm_volatility` (trust-region) → σ calibré |

## Lancement

```bash
python implied_vol.py
python fit_gbm_volatility.py
```

## Notions clés

- `k.implied_volatility(call_price, spot, strike, maturity, rate, dividend_yield)` → σ implicite
  - Inversion exacte par méthode numérique (Brent/Newton) — erreur typiquement < 1e-7.
- `k.fit_gbm_volatility(contract, maturities, market_prices, rate, n_paths)` → σ calibré
  - `market_prices` : liste de tuples `(spot, prix_observé)`.
  - Optimisation trust-region depuis σ₀ = 0.20 ; convergence locale.
  - Pour une inversion exacte sur options vanilles, préférer `implied_volatility`.
  - Utile pour les contrats path-dependent sans formule fermée.
