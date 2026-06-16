# 09 — Modèles de volatilité avancés

Cette section explore les modèles de dynamique du sous-jacent disponibles dans `kontract`
au-delà du GBM standard (Black-Scholes).

## Scripts

| Fichier | Modèle | Concept illustré |
|---|---|---|
| `heston.py` | Heston (1993) | Volatilité stochastique, skew via ρ, smile via σ_v |
| `sabr.py` | SABR (Hagan 2002) | Backbone CEV, vol backbone β, levier sur les wings |
| `merton.py` | Merton (1976) | Sauts de Poisson, enrichissement des queues OTM |
| `rough_bergomi.py` | Rough Bergomi (2016) | Vol rugueuse H=0.1, skew via ρ<0, queues lourdes |
| `model_comparison.py` | GBM / Heston / Merton / SABR | Tableau comparatif prix pour un call ATM |

## Concepts clés

- **Volatilité stochastique** (Heston, SABR, rBergomi) : la vol est elle-même aléatoire,
  ce qui produit un smile de volatilité implicite que BS ne peut pas reproduire.

- **Sauts** (Merton) : discontinuités dans le prix du sous-jacent → queues épaisses,
  options OTM profondément hors de la monnaie significativement renchéries.

- **Rugosité** (Rough Bergomi) : la vol est un processus non-Markovien (H < 0.5),
  cohérent avec la structure en puissance observée sur les marchés réels.

- **Levier (ρ < 0)** : dans tous les modèles à vol stochastique, la corrélation
  négative spot-vol génère le skew négatif observé sur les marchés actions.

## Référence analytique

Tous les scripts comparent leurs résultats à la formule de Black-Scholes :

```python
import math

def bs_call(S0, K, T, r, sigma):
    d1 = (math.log(S0/K) + (r + 0.5*sigma**2)*T) / (sigma*math.sqrt(T))
    d2 = d1 - sigma*math.sqrt(T)
    N = lambda x: 0.5*(1 + math.erf(x/math.sqrt(2)))
    return S0*N(d1) - K*math.exp(-r*T)*N(d2)
```

## Lancement

```bash
python heston.py
python sabr.py
python merton.py
python rough_bergomi.py
python model_comparison.py
```

Tous les scripts sont autonomes (`import kontract as k`).
`rough_bergomi.py` est plus lent (processus fractionnaire) — prévoir ~30–60 s.
