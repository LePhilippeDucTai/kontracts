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

## Aperçu

```python
from kontract import S, one, at, USD, GBM

# Un call à barrière, construit par composition d'opérateurs :
call = ((S("AAPL") - 100.0).clip(0.0) * one(USD)) @ at(1.0)
ko   = call.until(S("AAPL") >= 200.0)          # up-and-out

res = ko.price(GBM(s0=180, sigma=0.25, r=0.05, asset="AAPL"),
               n_paths=1_000_000, seed=42)
print(res.price, "±", res.std_error)           # prix + erreur Monte-Carlo

g = call.greeks(GBM(s0=180, sigma=0.25, r=0.05, asset="AAPL"))
print(g.delta, g.gamma, g.vega)
```

L'opérateur `@` mappe `when`, `*` met à l'échelle (`scale`), `+` compose (`and`),
`-` unaire inverse les flux (`give`), et les comparaisons produisent des
conditions de barrière/exercice.

## État — Phase 1 (MVP Trader) terminée ✅

Les jalons **J1–J10** sont `DONE` (voir `PROGRESS.md`) : AST sérialisable,
observables, simulateur GBM, compilateur, pricer Monte-Carlo compositionnel,
diagnostics MC (erreur standard, IC 95 %), barrières (`until`/`anytime`),
Greeks (Δ/Γ/ν/ρ par bump-and-reprice CRN), surfaces de Greeks, DSL ergonomique,
bindings PyO3, batch pricing (100 contrats en < 0,2 s), catalogue de produits
validés contre des formules fermées, CI et wheels.

Phases 2–4 (J11–J25) ajoutent : Heston/Dupire/SABR/Merton/Rough Bergomi,
réduction de variance, quasi-MC (Sobol), américaines (LSM), multilevel MC,
EDP Crank-Nicolson 1D / ADI 2D Heston (terme croisé), données de marché,
calibration (trust-region + CMA-ES), backtesting, **taux stochastiques**
(Vasicek/Hull-White, swaptions) et **FX** (Garman-Kohlhagen, quanto, composite).

Le binding Python expose tout cela : modèles avancés, dividende, réduction de
variance, américaines, taux, FX, produits et calibration — voir `python/`.

```python
from kontract import european_call, heston, USD
call = european_call("AAPL", 100.0, 1.0, USD)
res = call.price(heston(spot=100, v0=0.04, kappa=2, theta=0.04,
                        sigma_v=0.3, rho=-0.5, r=0.05, asset="AAPL"))
print(res.price, "±", res.std_error)
```

Auto-implémenté jalon par jalon par Claude Code. Faire avancer d'un jalon :
commande `/loop` (ou `./.claude/skills/jalon.sh start`).

## Installation Python sans Rust

Pour utiliser `kontract` dans un autre projet Python sans installer de
compilateur Rust, installe une wheel précompilée attachée à une GitHub Release :

```bash
uv add 'kontract @ https://github.com/LePhilippeDucTai/kontracts/releases/download/v0.2.3/kontract-0.2.3-cp39-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl'
uv run python -c "import kontract; print(kontract.__version__)"
```

Choisis le fichier `.whl` qui correspond à ta plateforme dans les assets de la
release GitHub :

- Linux ARM64 : `manylinux_2_17_aarch64.manylinux2014_aarch64.whl`
- Windows x64 : `win_amd64.whl`

Installer directement depuis le dépôt Git reste possible, mais cette forme
compile le module natif et nécessite Rust côté consommateur :

```bash
uv add 'kontract @ git+ssh://git@github.com/LePhilippeDucTai/kontracts.git'
```

## Build local

```bash
# Cœur Rust
cargo test                       # 80+ tests
cargo clippy --all-targets -- -D warnings

# Extension Python (dans un virtualenv)
python -m venv .venv && source .venv/bin/activate
pip install maturin numpy pytest
maturin develop --features python
pytest python/tests
```
