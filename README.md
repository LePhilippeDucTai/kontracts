# kontract

> Une algèbre des contrats financiers, compositionnelle et pricée par Monte-Carlo.
> Cœur en Rust, exposé à Python via PyO3/maturin.

Fondation : Peyton Jones, Eber, Seward — *Composing contracts: an adventure in
financial engineering* (ICFP 2000). Tout produit financier se décompose en
combinateurs primitifs formant une algèbre fermée :

```
zero | one(ccy) | give(c) | and(c1,c2) | or(c1,c2)
| scale(obs,c) | when(cond,c) | anytime(cond,c) | until(cond,c)
```

## Aperçu (cible)

```python
from kontract import one, scale, when, until, at, S

call = when(at(1.0), scale((S("AAPL") - 150).clip(0), one("USD")))
ko   = until(S("AAPL") >= 200, call)

res = ko.price(model=GBM(S0=180, sigma=0.25, r=0.05),
               n_paths=1_000_000, seed=42)
print(res.price, res.delta, res.vega)
```

## État

Auto-implémenté jalon par jalon par Claude Code. Voir `ROADMAP.md` et
`PROGRESS.md`. Faire avancer d'un jalon : commande `/loop`.

## Build local

```
maturin develop --features python
pytest
```
