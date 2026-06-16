# 01 — Briques de base du DSL

## Objectif

Illustrer systématiquement chaque primitive et opérateur du DSL kontract :
constructeurs, observables, conditions temporelles et de prix, opérateurs de
composition, et sérialisation JSON.

## Scripts

### `building_blocks.py`

Chaque primitive pricée individuellement :

| Primitive | Comportement attendu |
|-----------|----------------------|
| `k.zero()` | Prix = 0 (aucun flux) |
| `k.one(k.USD) @ k.at(T)` | ZCB = e^{-rT} |
| `k.give(c)` | Inverse le signe |
| `k.const_(x) * k.one(k.USD)` | Flux fixe actualisé |
| `k.S("X") * k.one(k.USD)` | Prépaid forward ≈ S0 |
| `c1 + c2` | Somme des parties (and) |
| `c1.or_(c2)` | Construction OK ; pricing LSM requis (J17) |

### `operators.py`

Syntaxe des opérateurs et types retournés :

| Opérateur | Entrée | Sortie |
|-----------|--------|--------|
| `@` | Contract, Condition | Contract (when) |
| `*` | Observable, one(ccy) | Contract (scale) |
| `+` | Contract, Contract | Contract (and) |
| unaire `-` | Contract | Contract (give) |
| `>=`, `<=`, `>`, `<` | Observable, float | Condition |
| `&` | Condition, Condition | Condition (et) |
| `\|` | Condition, Condition | Condition (ou) |
| `~` | Condition | Condition (non) |

### `serialization.py`

Round-trip JSON des contrats :

- `c.to_json()` produit un JSON compact lisible
- `k.Contract.from_json(s)` reconstruit le contrat
- Le prix du round-trip est **exactement identique** bit-à-bit (même seed)

## Lancer les scripts

```bash
python building_blocks.py
python operators.py
python serialization.py
```

## Interprétation de la sortie attendue

- `zero()` vaut 0, `give()` inverse le signe : fondements algébriques.
- Le ZCB vaut exactement `e^{-rT}` (aucune variance MC car flux déterministe).
- Le prépaid forward `S*one@at(T)` vaut `S0` sous GBM sans dividende.
- Le round-trip JSON est bit-à-bit exact : l'AST est fidèlement sérialisé.
