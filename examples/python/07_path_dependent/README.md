# 07 — Options path-dépendantes

## Objectif

Illustrer les quatre familles d'options dont le payoff dépend de l'intégralité
du chemin du spot (pas seulement de S_T), en montrant leurs propriétés de prix
clés par rapport au call/put vanille de référence.

## Scripts et fonctions illustrées

| Fichier | Produit | Observable | Propriété vérifiée |
|---|---|---|---|
| `asian_fixed_strike.py` | Call asiatique frappe fixe | `k.average(S)` | Asiatique < vanille (Jensen) |
| `asian_floating_strike.py` | Call asiatique strike flottant | `S - k.average(S)` | Prix > 0 |
| `asian_windowed.py` | Call asiatique fenêtré | `k.average_over(S, 0.5, 1.0)` | Fenêtré ≥ global |
| `lookback_fixed.py` | Lookback frappe fixe | `k.running_max(S)` | Lookback > vanille |
| `lookback_floating.py` | Lookback strike flottant | `S - k.running_min(S)` | Prix > 0, prix > vanille |

## Comment lancer

```bash
# Activer l'environnement avec kontract installé
source /tmp/j29venv/bin/activate

python examples/python/07_path_dependent/asian_fixed_strike.py
python examples/python/07_path_dependent/asian_floating_strike.py
python examples/python/07_path_dependent/asian_windowed.py
python examples/python/07_path_dependent/lookback_fixed.py
python examples/python/07_path_dependent/lookback_floating.py
```

Chaque script s'exécute de façon autonome (pas de dépendance entre eux).
La seed est fixée (`seed=42`) et `n_paths=150 000` pour la reproductibilité.
Grille de simulation : `steps_per_year=100` (nécessaire pour bien approcher
les moyennes et extrema continus).

## Interprétation des résultats

### Options asiatiques (moyenne)

La **moyenne arithmétique** a une variance inférieure à celle de S_T (inégalité
de Jensen).  Le call asiatique à frappe fixe (average – K)⁺ vaut donc moins que
le call vanille de même K.  Avec σ=20 % sur 1 an, l'écart est d'environ 45 %.

La **fenêtre de fixings** `average_over(0.5, 1.0)` ne couvre que la 2ᵉ moitié
de l'année : la moyenne porte sur moins de points, le lissage est moindre, et
le prix est intermédiaire entre l'asiatique global et le call vanille.

Le **strike flottant** (S_T – average)⁺ ne dépend pas de K : l'acheteur
profite de la sur-performance finale par rapport à la moyenne.

### Options lookback (extrema)

Le **lookback à frappe fixe** (running_max – K)⁺ est toujours plus cher que le
call vanille car running_max(S) ≥ S_T par construction.  La prime d'hindsight
croît avec σ (70 % de surcoût pour σ=20 %).

Le **lookback à strike flottant** (S_T – running_min)⁺ est toujours ≥ 0 et
domine le call vanille ATM.  L'acheteur profite d'avoir « acheté au plus bas ».
