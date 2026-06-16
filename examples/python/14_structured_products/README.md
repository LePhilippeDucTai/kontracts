# 14 — Produits structures complexes

Ces scripts pedagogiques **reconstruisent des notes structurees a partir des
combinateurs primitifs** de l'algebre `kontract` (DSL compositionnel a la
Peyton Jones). Aucun de ces produits n'est une fonction Python "prete a
l'emploi" : chacun est une **expression du DSL** compilee puis evaluee par le
pricer Monte-Carlo (modele GBM).

## Parametres communs (valides)

| Parametre | Valeur |
|-----------|--------|
| Spot `s0` | 100.0 |
| Volatilite `sigma` | 0.20 |
| Taux `r` | 0.05 |
| Maturite `T` | 1.0 an |
| Discount `e^{-rT}` | 0.9512 |
| Notional `N` | 100 |
| Call ATM (ref.) | ≈ 10.45 |
| Put ATM (ref.) | ≈ 5.57 |

## Lancer les scripts

```bash
/tmp/j29venv/bin/python examples/python/14_structured_products/<script>.py
```

Chaque script imprime la PV ± erreur standard, l'IC95, les bornes economiques
et une ligne RESUME. Les `assert` encodent les bornes (PV > 0, encadrement par
le plancher/cap actualises, coherence avec une reference analytique).

## La regle d'horizon (essentielle)

L'horizon de simulation Monte-Carlo = le **plus tardif `at(t)`** de tout l'arbre
du contrat. Les conditions de **barriere de prix** (`>=`, `<=`) ne creent PAS
d'horizon a elles seules. Donc toute branche utilisant `.until(cond_prix)`,
`.anytime(cond_prix)` ou `when(cond_prix, ...)` cote **exactement 0** sauf si un
noeud du MEME arbre porte un `@ at(T)`. Dans chaque note, la jambe de
remboursement `(... ) @ at(T)` fournit cet horizon T, ce qui permet de
surveiller les barrieres sur `[0, T]`. Combinez **toujours** les branches a
barriere avec un sibling `@ at(T)`.

> Illustration dans `shark_note.py` : la jambe `rebate` (un `.anytime`) cote 0
> en isolation, mais contribue ~1.01 dans la note complete grace aux siblings
> `@ at(T)`. Le script la valorise en lui adjoignant une ancre `0 one(USD) @ at(T)`.

---

## Les neuf produits

### 1. `autocallable.py` — Note a rachat anticipe
**Payoff.** A la 1re date ou `S >= B`, rappel : on recoit `N + coupon`. Sinon
`N` a maturite. Capital protege a 100 %.
**Construction.** `(N+coupon) one(USD).anytime(S>=B) + ((N one(USD) @ at(T)).until(S>=B))`.
**Combinateurs.** `anytime` (first-touch acquire), `until` (knock-out), `+` (and).
**Interpretation.** PV ≈ 99.01 (B=120). Bornes : `0 < PV < (N+coupon)*disc = 102.73`.
Plus la barriere est basse, plus le rappel est probable/precoce, plus la PV
monte vers le cap.

### 2. `reverse_convertible.py` — Coupon eleve, capital a risque
**Payoff.** `N + coupon - max(K - S_T, 0) * (N/K)` : obligation + coupon, moins
un put short de notionnel `N/K`.
**Construction.** `((N+coupon) one(USD) @ at(T)) + (-( (K-S).clip(0)*(N/K) one(USD) @ at(T)))`.
**Combinateurs.** `scale` (payoff observable), `give` (`-`, vente du put), `+`.
**Interpretation.** PV ≈ 97.14, coherent avec `(N+coupon)*disc - (N/K)*bs_put(K)`.
Bornes : `0 < PV < (N+coupon)*disc`.

### 3. `capital_protected_note.py` — Plancher garanti + participation
**Payoff.** `N + participation*(N/s0)*max(S_T - s0, 0)` : zero-coupon + call ATL
avec participation.
**Construction.** `((N one(USD) @ at(T)) + ((S-s0).clip(0)*(participation*N/s0) one(USD) @ at(T)))`.
**Combinateurs.** `scale`, `+`.
**Interpretation.** PV ≈ 101.38. Bornes : `PV >= plancher N*disc = 95.12`
(a l'epsilon MC), et `participation > 0 => PV > plancher`.

### 4. `bonus_certificate.py` — Action + put knock-out
**Payoff.** Action prepayee + put (strike = niveau bonus) actif tant que la
barriere basse n'est pas touchee. Garantit ~`max(bonus, S_T)` si barriere
intacte ; sinon action seule.
**Construction.** `((S one(USD) @ at(T)) + european_put("X", bonus, T, USD).until(S<=low_barrier))`.
**Combinateurs.** `until` (knock-out), `+`, produit du catalogue `european_put`.
**Interpretation.** PV ≈ 105.09 (100 pas/an). Bornes : `PV > 0`, `PV > s0`.
**LIMITE :** barriere a temps DISCRET ; la valeur depend de `steps_per_year`
(approximation d'une barriere continue, qui diminue la valeur quand on raffine).

### 5. `discount_certificate.py` — Covered call
**Payoff.** `min(S_T, K) = S_T - max(S_T - K, 0)` : action prepayee moins un call
vendu de strike `K` (cap). Achat avec decote, gain plafonne.
**Construction.** `((S one(USD) @ at(T)) + (-european_call("X", K, T, USD)))`.
**Combinateurs.** `give` (`-`, vente du call), `+`.
**Interpretation.** PV ≈ 93.93 = `s0 - bs_call(K)`. Bornes : `PV < s0` (decote
de ~6.07).

### 6. `twin_win.py` — Gain a la hausse comme a la baisse
**Payoff.** `N + (N/s0)*|S_T - s0|` tant que la barriere basse tient ; sinon `N`.
Profite des mouvements dans les deux sens (straddle KO autour du spot).
**Construction.** `((N one(USD) @ at(T)) + (((S-s0).clip(0)+(s0-S).clip(0))*(N/s0) one(USD) @ at(T)).until(S<=low_barrier))`.
**Combinateurs.** `scale`, `until`, `+`.
**Interpretation.** PV ≈ 107.32 (100 pas/an). Bornes : `PV > plancher 95.12`.
**LIMITE :** barriere DISCRETE ; valeur sensible a `steps_per_year`.

### 7. `corridor_note.py` — Coupon digital sur un range (mono-fixing)
**Payoff.** `N + coupon*1{L <= S_T <= H}` : coupon "tout ou rien" si `S_T` finit
dans `[L, H]`. Capital garanti.
**Construction.** `((N one(USD) @ at(T)) + (coupon one(USD) @ ((S>=L)&(S<=H)) @ at(T)))`.
**Combinateurs.** `when` (`@ cond`), `&` (and de conditions), `+`.
**Interpretation.** PV ≈ 98.60, coherent avec `N*disc + coupon*disc*P(L<=S_T<=H)`.
Bornes : `plancher < PV < plancher + coupon*disc`.
**LIMITE (importante) :** observation a UNE seule date (maturite). Un vrai
**range accrual** multi-fixing (coupon proportionnel au temps passe dans le
corridor) n'est PAS exprimable : l'algebre ne fournit pas d'observable
"indicatrice moyennee" sur le temps. On approxime donc par une digitale
mono-fixing a `T`.

### 8. `shark_note.py` — Capital protege + up-and-out call + rebate
**Payoff.** Capital `N` + call up-and-out (KO si `S >= H`) de notionnel `N/s0` +
rebate verse si `H` est touchee.
**Construction.** `((N one(USD) @ at(T)) + (const_(N/s0)*up_and_out_call("X", s0, H, T, USD)) + (rebate one(USD)).anytime(S>=H))`.
**Combinateurs.** `scale` (`Observable * Contract`), `anytime`, `+`, produit du
catalogue `up_and_out_call`.
**Interpretation.** PV ≈ 99.84. Decomposition : plancher 95.12 + call KO 3.71 +
rebate 1.01. Bornes : `PV >= plancher N*disc`.
**LIMITE :** barriere et rebate DISCRETS ; valeur sensible a `steps_per_year`.

### 9. `booster_note.py` — Capital + levier 2x plafonne
**Payoff.** `N + 2*[max(S_T-s0,0) - max(S_T-cap,0)]` : capital + bull call spread
a levier 2x (acceleration de la perf entre `s0` et `cap`, puis saturation).
**Construction.** `((N one(USD) @ at(T)) + (const_(2.0)*bull_call_spread("X", s0, cap, T, USD)))`.
**Combinateurs.** `scale` (`Observable * Contract`), `+`, produit du catalogue
`bull_call_spread`.
**Interpretation.** PV ≈ 109.49 = `N*disc + 2*(bs_call(s0) - bs_call(cap))`.
Bornes : `PV > plancher 95.12`.

---

## Recapitulatif des limites

- **Corridor note** : approximation **mono-fixing** d'un range accrual ; le vrai
  produit multi-fixing n'est pas exprimable dans l'algebre actuelle.
- **Bonus, twin-win, shark** : approximations de **barriere continue** par une
  surveillance en temps DISCRET. La valeur depend de `steps_per_year` (chaque
  script imprime cette sensibilite). On utilise 100 pas/an par defaut.
- **Modele** : GBM uniquement (volatilite et taux constants, discount
  deterministe). Pas de smile/skew ni de taux stochastiques.
