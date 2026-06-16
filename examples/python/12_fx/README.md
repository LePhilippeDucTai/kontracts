# 12 — Options FX

Pricing FX avec les formules analytiques Garman-Kohlhagen, Forward, et Quanto.

## Scripts

| Fichier | Contenu |
|---|---|
| `fx_options.py` | GK call/put, parité put-call exacte, Forward FX = IRP, Quanto monotone en ρ |

## Lancement

```bash
python fx_options.py
```

## Notions clés

- `k.garman_kohlhagen_call(x0, k, t, r_d, r_f, sigma)` → call FX (formule fermée)
- `k.garman_kohlhagen_put(...)` → put FX
- Parité put-call GK : `C − P = X0·e^{−r_f T} − K·e^{−r_d T}` (exacte à la machine)
- `k.fx_forward(x0, t, r_d, r_f)` = `X0·e^{(r_d−r_f)T}` (intérêt-parité)
- `k.quanto_call(s0, k, t, r_d, r_f, q_s, sigma_s, sigma_x, rho)` → call quanto
- Ajustement quanto : dividend drift → `r_d − r_f − ρ·σ_S·σ_X` ; prix décroissant en ρ
