# 02 — Dérivés linéaires

## Objectif

Illustrer les produits dont le payoff est linéaire en S_T : forwards, futures
(cost-of-carry), obligations zéro-coupon, obligations à coupons et annuités.
Ces produits servent de référence car leurs prix ont des formules fermées.

## Scripts

### `forward_prepaid.py`

- `k.forward("X", K, T, k.USD)` : prix = S0 - K·e^{-rT}
- Prépaid forward `(k.S("X")*k.one(k.USD)) @ k.at(T)` : prix ≈ S0 (sans div.)
- Parité spot-forward : F* = S0·e^{rT} → forward(K=F*) ≈ 0

### `futures_carry_dividends.py`

- `k.GBM(..., q=0.03)` : dividende continu réduit le forward équitable
- F*(r,q) = S0·e^{(r-q)T} : forward équitable prenant en compte le carry
- forward(K=F*) ≈ 0 dans tous les cas (dividende ou non)

### `bonds_annuities.py`

- ZCB via `k.one(k.USD) @ k.at(T)` : exact à la machine (pas de variance MC)
- Obligation à coupons : somme de contrats via `+` (opérateur and)
- Annuité : série de paiements unitaires
- Prix MC identique à Σ flux·e^{-r·t_i} (les flux sont déterministes)

## Lancer les scripts

```bash
python forward_prepaid.py
python futures_carry_dividends.py
python bonds_annuities.py
```

## Interprétation de la sortie attendue

- **Forward(K=95)** ≈ 9.63 : S0 - K·disc (valeur temps de l'argent)
- **Prépaid forward** ≈ 100 : recevoir l'actif à T contre paiement maintenant
- **ZCB(T=1,5)** : erreur MC = 0 car les flux sont constants (pas de vol.)
- **Obligation (coupon=r)** ≈ pair : 99.72 (légèrement sous le pair avec taux continu semestriel)
- **Annuité 10 ans** ≈ 7.67 : valeur actuelle de 10 paiements de 1 USD
