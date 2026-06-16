# 10 — Options multi-actifs

Cette section explore les produits dont le payoff dépend de plusieurs actifs corrélés,
pricés via `correlated_gbm` (GBM multi-dimensionnel avec matrice de corrélation).

## Scripts

| Fichier | Produit | Formule analytique |
|---|---|---|
| `basket.py` | Call sur panier équipondéré (3 actifs) | BS(σ_panier) avec σ_panier = σ·√((1+2ρ)/3) |
| `spread_margrabe.py` | Option d'échange max(S1-S2, 0) | Formule de Margrabe (1978) — exacte |
| `best_of_worst_of.py` | Rainbow best-of / worst-of | Identité max+min = S1+S2 |
| `outperformance.py` | Performance relative max(S1/S1₀ − S2/S2₀, 0) | Margrabe normalisé |

## Modèle multi-actifs

```python
import kontract as k

factors = [
    k.GbmFactor("S1", s0=100, mu=0.05, sigma=0.20),
    k.GbmFactor("S2", s0=100, mu=0.05, sigma=0.20),
]
corr  = [[1.0, 0.5], [0.5, 1.0]]  # matrice de corrélation N×N
model = k.correlated_gbm(factors, corr, r=0.05)
```

Accès aux actifs dans le DSL : `k.S("S1")`, `k.S("S2")`.

## Relations importantes

### Panier (N actifs identiques, corrélation uniforme ρ)
```
σ_panier = σ × sqrt((1 + (N-1)ρ) / N)
```

### Margrabe (option d'échange)
```
Prix = S1₀·N(d1) − S2₀·N(d2)
σ_spread = sqrt(σ1² + σ2² − 2ρσ1σ2)
```
Formule exacte (pas d'approximation), valable pour K = 0.

### Identité rainbow
```
best_of + worst_of = call(S1) + call(S2)
```
car max(a,b) + min(a,b) = a + b payoff par payoff.

## Impact de la corrélation

| Produit | ρ ↑ | ρ ↓ |
|---|---|---|
| Panier | Prix ↓ (moins de diversification) | Prix ↓ |
| Spread / Margrabe | Prix ↓ (actifs bougent ensemble) | Prix ↑ |
| Best-of | Prix ↓ | Prix ↑ |
| Worst-of | Prix ↑ | Prix ↓ |

## Lancement

```bash
python basket.py
python spread_margrabe.py
python best_of_worst_of.py
python outperformance.py
```

Tous les scripts sont autonomes (`import kontract as k`, `import math`).
