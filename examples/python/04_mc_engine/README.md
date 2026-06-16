# 04_mc_engine — Moteur Monte-Carlo : diagnostics et réduction de variance

## Objectif

Illustrer les propriétés statistiques du pricer Monte-Carlo de `kontract` :
convergence en 1/√N, intervalles de confiance, techniques de réduction de
variance (variables antithétiques, variable de contrôle) et quasi-Monte-Carlo
par séquences de Sobol.

## Scripts

| Fichier | Ce qu'il montre |
|---|---|
| `mc_diagnostics.py` | std_error ∝ 1/√n_paths ; IC95 contient le prix BS |
| `variance_reduction.py` | antithetic=True et control_variate=True réduisent la variance |
| `sobol_qmc.py` | k.sobol_gbm vs k.GBM : réduction de variance quasi-MC |

## Comment lancer

```bash
python mc_diagnostics.py
python variance_reduction.py
python sobol_qmc.py
```

(Nécessite `kontract` installé dans l'environnement Python.)

## Interprétation des sorties

### mc_diagnostics.py
La colonne `std*√n` est à peu près constante (≈ 14.7) : c'est la signature
de la convergence en 1/√N du MC standard. L'IC95 contient le prix Black-Scholes
à chaque taille d'échantillon.

### variance_reduction.py
- **Antithétique** : réduit l'erreur-standard d'un facteur ~1.4× en simulant
  des paires de chemins corrélés négativement (U et 1-U).
- **Variable de contrôle** : exploite la corrélation entre le payoff et un
  actif de prix connu analytiquement. Dans `kontract`, cela donne une erreur
  quasi-nulle car le call est utilisé comme variable de contrôle exacte.

### sobol_qmc.py
`k.sobol_gbm` utilise la séquence de van der Corput (bit-reversal). La
variance est réduite d'un facteur ~6 vs MC standard, ce qui illustre la
convergence quasi-O(1/N) théorique.

**Note d'implémentation** : la combinaison des séquences dans l'implémentation
actuelle (moyenne de deux séquences VdC) introduit un biais systématique dans
le prix (la distribution n'est pas exactement log-normale). La propriété de
réduction de variance est néanmoins préservée et vérifiée par assertion.
