# 11 — Taux stochastiques

Pricing de produits de taux avec les modèles courts Vasicek et Hull-White.

## Scripts

| Fichier | Contenu |
|---|---|
| `zero_coupon_stochastic.py` | ZCB Monte-Carlo vs `discount_bond0()` analytique — Vasicek & Hull-White |
| `coupon_bond.py` | Obligation à coupons = somme de ZCB via `+` — prix MC vs somme analytique |
| `swaptions.py` | Swaption payeuse/receveuse ; MC vs Jamshidian ; parité payeur–receveur |

## Lancement

```bash
python zero_coupon_stochastic.py
python coupon_bond.py
python swaptions.py
```

## Notions clés

- `k.vasicek(r0, a, b, sigma)` / `k.hull_white(r0, a, sigma)` → `RateModel`
- `k.zero_coupon_bond(k.USD, T).price_under_rates(rm)` → prix par simulation du taux court
- `rm.discount_bond0(T)` → prix analytique (formule fermée)
- `k.Swaption.level(expiry, tenor, n, fixed_rate, is_payer)` → swaption
- `k.swaption_mc(rm, sw)` / `k.vasicek_swaption_analytic(r0, a, b, sigma, sw)` → prix
- Parité payeur–receveur : `P − R = P(0,T_start) − P(0,T_end) − K·Σδ·P(0,T_i)`
