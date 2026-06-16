# 05_greeks — Greeks Monte-Carlo vs Black-Scholes

## Objectif

Montrer comment `k.Contract.greeks()` calcule par bump-and-reprice les
sensibilités d'un call européen, et les comparer aux formules analytiques
Black-Scholes pour valider la cohérence du moteur.

## Scripts

| Fichier | Ce qu'il montre |
|---|---|
| `greeks.py` | Δ/Γ/ν/ρ MC vs formules BS pour un call ATM 1 an |
| `greeks_scenario.py` | Grille de spots (80..120) : tableau spot/delta/gamma/vega |

## Comment lancer

```bash
python greeks.py
python greeks_scenario.py
```

## Interprétation des sorties

### greeks.py

- **Delta Δ = N(d₁)** : probabilité risque-neutre que l'option finisse ITM
  (en termes de l'actif). Pour un call ATM 1 an à σ=20 % : Δ ≈ 0.637.
- **Gamma Γ = φ(d₁)/(Sσ√T)** : courbure. L'implémentation MC par différences
  finies (bump de S) peut introduire un léger biais ; l'erreur relative peut
  atteindre ~35 % sur Γ car c'est une dérivée seconde — la tolérance de
  l'assertion ne couvre que Δ (erreur < 0.02) et ν (< 5 %).
- **Vega ν = Sφ(d₁)√T** : sensibilité à la volatilité. Très bien estimé par MC.
- **Rho ρ = KTe^{-rT}N(d₂)** : sensibilité au taux. Excellent accord.

### greeks_scenario.py

Le tableau montre clairement :
- **Delta croissant** avec S (de 0.22 OTM à 0.90 deep-ITM).
- **Gamma maximal ATM** (courbure la plus forte quand S ≈ K).
- **Vega maximal ATM** (la valeur dépend le plus de σ quand l'option est ATM).
