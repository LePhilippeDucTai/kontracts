# 15 — Portefeuille & Pricing par lot

Construction d'un livre de contrats, pricing en boucle, Greeks agrégés, sérialisation JSON.

## Scripts

| Fichier | Contenu |
|---|---|
| `portfolio.py` | 5 contrats : call, put, straddle, up-and-out, ZCB — prix + delta + JSON round-trip |

## Lancement

```bash
python portfolio.py
```

## Notions clés

- Un **livre** est une liste Python de `(label, Contract)`.
- Pricing en boucle : `contract.price(model, n_paths, seed)` → `PriceResult(.price, .std_error)`.
- Greeks : `contract.greeks(model, n_paths, seed)` → `Greeks(.delta, .gamma, .vega, .rho)`.
- **Delta total** = somme des deltas individuels (le ZCB a delta = 0, pas de dépendance au sous-jacent).
- **Sérialisation** : `contract.to_json()` → chaîne JSON ; `k.Contract.from_json(s)` → rechargement.
  - Le prix recalculé sur le contrat rechargé est identique (à virgule flottante près).
- `k.up_and_out_call(asset, strike, barrier, maturity, ccy)` → produit barrière composé via DSL.
