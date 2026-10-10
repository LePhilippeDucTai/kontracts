# Functional specification — market data, curves, instruments and volatility

Status: dirty-room functional specification for the `clean-room-core` plan,
unit J01-P2. It states **what** kontract v0.3 must provide for market data
and market objects, never **how** a reference implementation does it.
Implementers follow `docs/clean-room/PROTOCOL.md`, choose their own names
and structure, and cite the public source of every formula and convention.

Scope: the market-data store with a quote file format designed for kontract,
fixings, scenario shifts and projected market data; yield-curve construction
and multi-curve; rate instruments and bond analytics; dividends, repo and
forwards; volatility quoting formulas, implied-volatility inversion, quote
conversions, smiles and surfaces including SVI, cap/floor and swaption
structures; credit curves and CDS conventions.

Out of scope here (specified with the models, plan units J08–J10): local
volatility and stochastic-volatility model calibration, CMS and other
convexity adjustments beyond futures, credit portfolio loss models (copulas,
tranches, base correlation), and inflation models.

Global conventions:

- Values are `f64`. Rates, yields, spreads and volatilities are stored as
  decimals (one percent is 0.01, one basis point is 0.0001); quote files may
  declare another unit per line (see M1) and are converted on load.
- Time to a date is a year fraction under a convention attached to the
  object (ACT/365 Fixed by default), never an implicit global.
- Every market object is immutable once built, serialisable, and knows the
  as-of date it was built for.
- All fallible operations return typed errors; a bootstrap or fit that cannot
  reach its target returns an error carrying diagnostics, never a silently
  degraded object.
- Dates, tenors, calendars, day counts and schedules come from time.md;
  root finding, interpolation, quadrature and optimisation come from
  numerics.md.

Mapping onto the plan:

| Group | Capability group | Plan unit |
|-------|------------------|-----------|
| M1 | Market-data store, quote files, fixings, scenarios | J04-P1 |
| M2 | Yield curves, bootstrapping and multi-curve | J04-P2 |
| M3 | Rate instruments and bond analytics | J04-P3 |
| M4 | Volatility: formulas, inversion, conversions, smiles, surfaces, cap and swaption structures | J04-P4 |
| M5 | Credit curves and CDS | J04-P5 |
| M6 | Equity forwards, dividends and repo; FX and commodity forwards | J04-P6 |

## Capabilities

### M1 — Market-data store, quote files, fixings and scenarios (J04-P1)

- M1.1 Typed quote identifiers covering at least these quote kinds:
  - rates: deposit rates by money-market tenor; FRA rates (start × end);
    interest-rate futures prices by contract; par swap rates by index and
    tenor; OIS rates; tenor basis swap spreads; cross-currency basis spreads
    (constant or resetting notional, deliverable or not); zero-coupon
    discount factors or zero rates by date and curve; a flat rate; government
    bond yields; the short-rate volatility used for futures convexity;
  - FX: spot by currency pair, forward points and outright forwards by
    tenor, non-deliverable forward outrights;
  - equity: spot, cash and proportional discrete dividends (ex-date,
    payment date, announced or projected), continuous dividend yields valid
    up to a date, implied dividend yields by maturity, explicit forwards,
    repo or borrow spreads, listed option prices (expiry, call or put,
    European or American, strike) with the spot observed with them;
  - commodity: spot, futures by delivery month, convenience yields;
  - inflation: index fixings by reference month, zero-coupon inflation swap
    rates, zero-coupon and year-on-year cap and floor prices or volatilities;
  - volatility: equity and commodity volatilities (flat, term structure or
    smile, optionally call- or put-specific), historical volatilities, FX
    at-the-money, risk-reversal and butterfly quotes by expiry and delta,
    cap/floor volatilities (maturity, strike or at-the-money, index tenor),
    swaption volatilities (expiry, underlying tenor, absolute strike or
    spread to at-the-money, payer or receiver), each tagged Black, normal or
    shifted Black with its shift;
  - credit: CDS par spreads and upfronts by tenor (with standard coupon),
    recovery rates, generic credit spreads, funding spreads over a floating
    index, fixed funding rates, spread volatilities;
  - correlations between any two risk factors (equity, FX pair, short rate,
    commodity, inflation, credit);
  - bonds: clean price, dirty price, yield, spread, by bond identifier;
  - user-defined numeric items keyed by a free string.
- M1.2 Identifier canonicalisation: each identifier has one canonical text
  form; a correlation identifier is order-independent (the two factors are
  sorted canonically); when an FX pair appears inverted in a correlation, the
  stored correlation changes sign accordingly; an optional curve-set label on
  rate quotes separates alternative quote sets for the same instrument (for
  example single-curve and multi-curve sets).
- M1.3 Quote record: identifier, value, unit, optional observation
  timestamp, optional source and a list of warnings (for example "value
  could not be parsed, treated as missing").
- M1.4 As-of-dated snapshot: an immutable collection of quote records for
  one as-of date with lookup by identifier (typed "missing quote" error),
  queries by kind and by filter, unions of queries, and validation that flags
  expired dated quotes (expiry or maturity on or before the as-of date) and
  duplicates.
- M1.5 Selection filters used by calibrations: cap and swaption subsets (at
  the money only, away from the money only, by expiry range, by underlying
  tenor, co-terminal set, the set surrounding a given maturity with a minimum
  count), call or put only, strike range, and removal of a whole family
  (dividends, credit spreads).
- M1.6 FX lookups: direct and inverse pairs (the inverse is the reciprocal),
  identical currencies give one; triangulation through a pivot currency only
  when explicitly enabled.
- M1.7 Past fixings store: dated time series of published index fixings
  (interest-rate indices, overnight indices, FX fixings, equity closes,
  inflation index levels), queried by index and date, with a typed "fixing
  missing" error, used to value already-fixed observations.
- M1.8 Quote file format designed for kontract (see Behaviour M1) with
  loading, validation and writing, plus a lossless serde form of a whole
  snapshot.
- M1.9 Bumps: absolute or relative shifts per quote, per kind or per bucket
  (a tenor, expiry or strike range), with quote-kind-aware rules: futures are
  shifted in rate space (price moves opposite to the rate), zero-coupon and
  bond quotes by a parallel shift of their continuously compounded yield,
  volatilities in the units of their own quote type and floored at zero,
  correlations clamped to [-1, 1], integer-valued items (for example a
  credit index default count) never shifted.
- M1.10 Scenario algebra: an elementary scenario applies a set of bump rules
  simultaneously; scenarios combine in parallel (independent results),
  in sequence (cumulative, each step recorded), as a Cartesian product, split
  by a partition rule, filtered, or as the identity; every changed quote
  records the rule that changed it; a rule may refer to the previous value
  and to the base-date value.
- M1.11 Projected market data: dated anchor points per quote joined by linear
  or compounded interpolation, with anchor dates given as dates or tenors,
  an empty step repeating the previous one, quotes without a rule kept
  constant, a later rule for the same quote overriding an earlier one, and
  optional flat extension after the horizon.

### M2 — Yield curves, bootstrapping and multi-curve (J04-P2)

- M2.1 Discount curve abstraction: discount factor P(t), log discount factor,
  zero rate under continuous compounding and under a requested compounding
  frequency and day count, the derivative of the zero rate, instantaneous
  forward f(t), and simple forward rate between two dates under a day count.
- M2.2 Interpolation schemes on curve pillars: log-linear discount factors
  (piecewise-flat instantaneous forwards); linear zero rates;
  piecewise-linear instantaneous forwards, with an option where
  the forward moves linearly over a configurable fraction of each interval
  and is flat over the rest; natural cubic on zero rates; monotone convex
  (Hagan and West 2006); with flat-forward or flat-zero extrapolation
  selectable at each end. Pillars are the instrument maturities, optionally
  augmented with interval mid-points or with all schedule dates.
- M2.3 Bootstrap instruments: deposits; FRAs; interest-rate futures with a
  convexity adjustment (none, Ho-Lee closed form, Hull-White one-factor
  closed form, or a quoted adjustment); par swaps; OIS; tenor basis swaps;
  cross-currency basis swaps (fixed-fixed, float-float, constant or
  resetting notional, deliverable or not); FX forward points and outrights;
  non-deliverable forwards; zero-coupon quotes; a flat rate. Futures and FRAs
  are filtered by their underlying tenor; maturity windows per instrument
  family (for example futures up to a cut-off date, swaps beyond) are
  configurable.
- M2.4 Sequential bootstrap in maturity order: an instrument whose value
  depends on a single unknown discount factor is solved in closed form;
  otherwise a bracketed root search (numerics.md N5) with a floor on implied
  forward rates; for interpolation schemes that are not local (cubic,
  monotone convex), an outer fixed-point or global Newton iteration until all
  instruments reprice. Quotes with a duplicated maturity or that cannot be
  solved are dropped and reported with the reason.
- M2.5 Multi-curve: one discount curve per collateral currency (overnight
  index curve by default, or a term-rate curve for legacy single-curve
  setups) and one projection curve per index tenor (for example one, three,
  six and twelve months), each projection curve stored either as its own
  curve or as a spread over the discount curve; tenor basis swaps assigned to
  the projection curve of their target tenor; the projection curve of a tenor
  without its own curve falls back to the nearest available tenor, with a
  warning.
- M2.6 Curves implied through FX: a currency's discount curve built from a
  reference currency's curves and cross-currency instruments (forward points
  up to a cut-off date, cross-currency basis swaps beyond), optionally
  adjusting its projection curves; chains of reference currencies (for example
  an offshore currency built through a non-deliverable reference); a
  multi-curve cross-currency set without multi-curve enabled is a typed error.
- M2.7 Spread curves: additive spreads on zero rates (linear interpolation,
  flat extrapolation) and piecewise-constant forward spreads
  (numerics.md N7.6) on top of a base curve.
- M2.8 Curve algebra: sum and difference of curves, adding a piecewise
  constant forward spread, parallel shift, constant curve, curve from a list
  of discount factors.
- M2.9 Sensitivities: parallel shift of all curves or of one currency's
  curves; bucketed sensitivities to each input quote by re-bootstrap; the
  Jacobian of pillar zero rates with respect to input quotes and its inverse
  (for converting between quote and zero-rate risk).
- M2.10 Diagnostics: for a built curve, the instruments used, the quotes
  dropped with reasons, and the repricing error of each instrument.
- M2.11 Inflation index forward curve from zero-coupon inflation swaps on
  top of a nominal curve, with index lag and optional seasonality (consumed
  by plan unit J10-P3).

### M3 — Rate instruments and bond analytics (J04-P3)

- M3.1 Cash-flow and leg model: dated cash flows (payment date, currency,
  notional, accrual start and end, year fraction, fixed rate or index with
  fixing dates, spread, gearing); fixed legs; floating legs on a term index
  or on an overnight index (compounded in arrears, averaged, with lookback or
  observation shift, time.md T7.6); several resets compounded within one
  period (basis swap short-tenor leg); hooks for capped and floored coupons
  priced by the volatility layer.
- M3.2 Instruments: deposit, FRA (with the market settlement formula paying
  the discounted difference at the FRA start), interest-rate future (price =
  100 × (1 - rate) plus convexity), par swap, OIS, tenor basis swap,
  cross-currency swap with initial and final notional exchange, resetting
  (mark-to-market) notional and non-deliverable settlement, zero-coupon swap;
  each instrument converts its quote into cash flows worth zero at par, used
  by the bootstrap and by Monte-Carlo repricing tests.
- M3.3 Swap analytics: present value per leg, par rate, par spread, annuity
  (PV01) and DV01, forward-starting swaps.
- M3.4 Amortising, accreting and custom notional schedules on any leg.
- M3.5 Bonds: fixed-coupon, zero-coupon and floating-rate bonds built from
  schedules (time.md T7) with the government-market conventions of
  time.md T4.3; accrued interest (with ex-coupon periods); clean and dirty
  price; yield to maturity under the street convention (compounding at the
  coupon frequency, money-market yield in the last period as an option) and
  price from yield; Macaulay and modified duration, convexity, DV01.
- M3.6 Bond spreads: Z-spread (constant spread over the zero curve that
  reprices the dirty price), I-spread, par-par asset-swap spread, discount
  margin over a floating index, the equivalent fixed coupon, a credit spread
  over the discount curve and the CDS-bond basis against a credit curve
  (M5); any one of clean price, dirty price, spread or equivalent coupon may
  be the input and the others are derived; the price as a function of spread
  and rate shocks.
- M3.7 Funding-implied Z-spreads: derive a Z-spread for funding-adjusted
  discounting from a funding spread over a floating index or from a fixed
  funding rate.

### M4 — Volatility (J04-P4)

- M4.1 Vanilla quoting formulas: Black (1976) on a forward, Bachelier
  (normal) and shifted Black (displaced lognormal with a shift), for calls,
  puts, caplets, floorlets and swaptions (annuity × formula on the forward
  swap rate), single-curve or with separate discount and projection curves;
  sensitivities delta, gamma, vega, volga, vanna, theta and rho.
- M4.2 Implied-volatility inversion for Black (rational initial guess with
  Householder iterations to machine precision, after Jäckel's "Let's be
  rational", or an equivalent with the same accuracy and robustness),
  Bachelier (Jäckel's closed-form-plus-correction method or an equivalent)
  and shifted Black; the result distinguishes a regular volatility, a price
  below intrinsic value (no solution) and a price at or above the upper bound
  (infinite volatility).
- M4.3 Quote conversions between Black, normal and shifted Black (any two
  shifts) at the same strike and expiry by exact price matching; a
  conversion that has no solution (for example a normal quote on a negative
  forward converted to Black) is a typed result; fast closed-form
  approximations (Hagan's normal-lognormal relation) as an explicit option.
- M4.4 Displaced-diffusion smile with piecewise-constant parameters: a
  volatility and a blend parameter between normal and lognormal dynamics,
  calibrated to an at-the-money volatility and skew, to an at-the-money and
  one other volatility, or to two strikes, with bounds on the blend; the pure
  normal case is the continuous limit of the general formula.
- M4.5 FX smile conventions: spot delta, forward delta and their
  premium-adjusted variants; at-the-money as forward or as delta-neutral
  straddle (premium-adjusted or not); construction of the smile from
  at-the-money, risk-reversal and butterfly quotes at standard deltas under
  the broker (smile) strangle and the market strangle definitions; delta to
  strike and strike to delta under each convention (premium-adjusted cases by
  root search with a documented fallback); smile interpolation in delta or
  in strike; inversion of the currency pair (reciprocal strikes, calls
  becoming puts, identical volatilities).
- M4.6 Volatility surfaces on expiry × strike with strikes expressed as
  absolute, percent of forward, percent of spot, log-moneyness or delta;
  interpolation in total implied variance along expiries (linear in total
  variance at fixed moneyness) and with numerics.md N7 interpolators along
  strikes; sticky-strike and sticky-moneyness behaviour on spot moves;
  term-structure-only fallback when a slice has too few strikes.
- M4.7 SVI per expiry (raw parametrisation, Gatheral 2004) with a
  quasi-explicit calibration (De Marco and Martini) and SSVI across expiries
  (Gatheral and Jacquier 2014); no-arbitrage checks: butterfly (Durrleman's
  condition on total variance, equivalently non-negative implied density),
  calendar (total variance non-decreasing in expiry at fixed log-moneyness)
  and the SSVI parameter conditions; a fit that cannot satisfy them is
  reported, not silently accepted.
- M4.8 Quote cleaning per expiry: removal of flat extrapolated wings,
  detection of outliers whose discrete second difference across strikes is
  large relative to the slice's range, keeping of large consistent zones and
  removal of isolated spikes, and collapse of a fully flat slice to its
  at-the-money point; each removal reported.
- M4.9 From option prices to volatilities: per expiry, the implied discount
  factor and forward from put-call parity on call and put pairs (exact
  pairs, regression, or a supplied curve), then one implied volatility per
  strike balancing call and put errors; American option prices inverted with
  a binomial tree (Cox-Ross-Rubinstein or Leisen-Reimer) into implied
  volatilities and implied dividend yields; results rolled to the as-of date
  under sticky strike or sticky delta.
- M4.10 Cap and floor volatility matrices (maturity × strike, Black, normal or
  shifted) with the at-the-money cap strike equal to the forward swap rate of
  the cap's caplets; caplet stripping: at-the-money by vega-weighted
  bootstrapping (Alexander 2003) and smiled by bootstrapping per strike with
  the caplet volatility interpolated linearly in cap volatility, linearly in
  cap price, or held piecewise constant between cap maturities; stripped
  caplets reprice the input caps.
- M4.11 Swaption matrices (expiry × underlying tenor, at the money) and cubes
  (strike as spread to at-the-money); conversion of spread-to-at-the-money
  quotes into absolute volatilities; per (expiry, tenor) SABR fit with the
  Hagan et al. (2002) expansion or Obłój's (2008) refinement, shifted SABR
  with fixed or calibrated shift, conversion from an at-the-money volatility
  to the SABR alpha (a typed error when no positive root exists), and
  interpolation of prices and Greeks across the cube; fitted parameters
  serialisable and reloadable.

### M5 — Credit curves and CDS (J04-P5)

- M5.1 Survival curve: survival probability Q(t), piecewise-flat hazard
  rates (piecewise-constant function of time, numerics.md N7.6), default
  density, conditional survival between two dates, and survival-weighted
  discount factors.
- M5.2 CDS under the ISDA Standard Model conventions: premium leg with
  accrual on default, protection leg from the step-in date, standard
  quarterly schedule (time.md T7.10), the interest-rate curve built and
  interpolated as the standard model prescribes (piecewise-flat forwards),
  standard coupons, cash settlement; outputs: clean and dirty price in
  percent of notional, upfront amount, accrued days and accrued premium,
  cash-settlement amount, present value, par spread, risky annuity, protection
  and premium leg values, CS01 and recovery sensitivity.
- M5.3 Hazard-curve bootstrap from par spreads or from upfront quotes (an
  upfront quote requires a standard coupon; missing coupon is a typed error),
  with optional spread shift, rate shift and a quanto (FX-default) factor per
  currency.
- M5.4 Conversions: quoted (conventional) spread to upfront and back through
  a flat hazard curve; par spread to hazard and back; a linear spread curve
  to hazard rates and back through the recovery-of-par relation; standard
  coupon and par coupon from a curve.
- M5.5 Credit-risky discounting from either a CDS-implied hazard curve, a
  flat generic spread, a term structure of generic spreads, or bond and
  funding implied Z-spreads (M3.7); when several sources are available the
  choice is explicit, with a documented default precedence; an unsupported
  combination is a typed error.
- M5.6 Forward CDS spread and forward risky annuity, with and without
  front-end protection (consumed by CDS option pricing in plan unit J10-P1).

### M6 — Equity forwards, dividends and repo; FX and commodity forwards (J04-P6)

- M6.1 Equity forward curve from spot, the discount curve, continuous
  dividend yields, discrete cash and proportional dividends, repo or borrow
  spreads, implied dividend yields, a forward growth-rate override, or
  explicit forwards (which take precedence over the other inputs up to their
  last date).
- M6.2 Discrete dividends: cash amounts with ex-date and payment date
  (discounted from the payment date), proportional dividends, announced
  versus projected dividends, an option to treat all dividends as
  proportional (or projected ones only, beyond a cut-off date), and
  index-level synthetic dividends for price-return versus total-return
  indices.
- M6.3 Continuous carry items (dividend yields, repo spreads) as dated items
  each valid up to its date, plus an optional open-ended item that applies
  after the last dated item; the net carry function (repo spread minus
  dividend yield) as a piecewise-constant function of time.
- M6.4 Implied dividends: implied dividend yields by maturity reproduced by
  the forward curve; dividend amounts implied from forwards, from dividend
  futures, or from put-call parity on listed options.
- M6.5 Dividend-adjusted spot process: forward-based quantities for the
  dividend-free ("pure") process, used to convert option quotes between the
  market process and a dividend-free process (consumed by plan unit J08-P1).
- M6.6 FX forwards from forward points (with the pair's pip scaling) and
  outrights; covered interest parity against the two discount curves;
  non-deliverable forwards settled in the settlement currency.
- M6.7 Commodity forward curve from futures by delivery month (interpolation
  between delivery dates, spot as the front), implied convenience yield, and
  the spot-to-futures relation used by commodity models (plan unit J10-P2).

## Behaviour

### M1 — Market-data store and quote file format

- Snapshot building from a list of records rejects duplicate identifiers
  with a typed error listing them; a merge operation lets the later record
  win explicitly.
- Lookups never panic; a missing quote is a typed error naming the
  identifier.
- The quote file format is UTF-8 CSV (RFC 4180, comma separator, double-quote
  escaping) with a mandatory header line and these columns, kontract's own
  design:

```text
asof,id,value,unit,source,label
<date>,RATE/DEPO/EUR/3M,<number>,decimal,,
<date>,RATE/SWAP/USD/SOFR/10Y,<number>,percent,<source>,<label>
<date>,FX/SPOT/EURUSD,<number>,price,,
<date>,VOL/SWPT/EUR/1Y/5Y/ATM/NORMAL,<number>,decimal,,
```

  `asof` is an ISO date; `id` is a slash-separated identifier whose first
  segments name the asset class and the quote kind and whose remaining
  segments are kind-specific (currency, index, tenor, expiry, strike, delta,
  pair, name, date); volatility identifiers end with their quote type
  (BLACK, NORMAL, or SHIFTED with its shift); `unit` is one of decimal,
  percent, bp or price; `source` and `label` (curve-set label) may be empty.
  The angle-bracket fields above are placeholders, not data.
  Lines starting with `#` are comments; blank lines are ignored. A separate
  file with columns `index,date,value` holds past fixings.
- Loading reports every invalid line (line number, column, reason) in one
  error value rather than stopping at the first; an unparsable value can be
  loaded as "missing" with a warning only when the caller asks for lenient
  mode. Writing then reading a snapshot returns an identical snapshot.
- A bump rule targets quotes by kind and identifier pattern; relative bumps
  multiply, absolute bumps add, both in the quote's own unit and volatility
  type (a normal volatility is bumped in normal points, a Black volatility in
  Black points); targeting the same quote twice within one elementary
  scenario is a typed error (kontract's own choice: no silent double bump).

### M2 — Yield curves

- Every bootstrap instrument reprices to its quote within a tight tolerance
  on the built curve (repricing error below the root-finder tolerance).
- P(0) = 1 and the curve is defined from the as-of date; asking for a date
  before the as-of date is a typed error.
- Log-linear discount-factor interpolation gives a constant instantaneous
  forward on each pillar interval; linear zero-rate interpolation is
  continuous in the zero rate; monotone convex keeps positive forwards
  positive and is local, as stated by Hagan and West.
- Deposits use their own day count and spot lag and quote simple interest;
  curve time uses the curve's own day count.
- Futures: the forward rate equals 1 - price / 100 minus the convexity
  adjustment; with zero short-rate volatility the adjustment is zero.
- Projection forwards for a tenor are read from that tenor's curve; the
  discount curve is never used as a projection curve unless explicitly
  configured (single-curve mode).

### M3 — Rate instruments and bonds

- A par swap built from the curve's own par rate has zero present value;
  the annuity is the sum of discounted year fractions of the fixed leg; in
  single-curve mode the par rate equals (P(t₀) - P(tₙ)) / annuity.
- Cross-currency swaps value each leg in its currency and convert at spot;
  with zero basis spread and consistent curves their value is zero.
- Bond yield and price are inverse functions on the price domain; clean =
  dirty - accrued; accrued is zero on a coupon date and, in the ex-coupon
  period, negative as the market convention requires; a clean price input
  without the information needed to compute accrued interest is a typed
  error.
- Z-spread, discount margin and asset-swap spread are found by bracketed root
  search and reprice the input price exactly.

### M4 — Volatility

- Formulas satisfy put-call parity exactly in their own model; Black with
  zero volatility returns the discounted intrinsic value; Bachelier accepts
  negative forwards and strikes; shifted Black requires forward + shift > 0
  and strike + shift > 0 (typed error otherwise).
- Implied volatility inversion reaches a relative accuracy close to machine
  precision in volatility wherever the price carries that information, and
  never loops indefinitely.
- At-the-money volatility of an FX smile is reproduced exactly; the
  constructed smile reprices the risk-reversal quote and the strangle quote
  under the chosen strangle definition.
- A surface never returns a negative total variance; calendar-arbitrage
  violations in the input are reported per expiry pair.
- Caplet stripping returns caplet volatilities whose caps reprice the input
  cap prices within tolerance; the at-the-money cap strike excludes the first
  (already fixed) caplet, per market convention.
- SABR fits are performed in the quote's own volatility type (normal or
  Black); fitted smiles can be reported in either type through M4.3.

### M5 — Credit

- With a flat hazard rate λ, survival is exp(-λt); the bootstrapped curve
  reprices every input CDS to its quote (par spread or upfront).
- Under the ISDA Standard Model the upfront of a contract whose standard
  coupon equals the par spread is zero (up to accrual conventions); the
  accrued premium covers the days from the accrual start date up to and
  including the trade date (the step-in date excluded), as the model
  specifies.
- Survival probabilities are non-increasing in time; negative hazard rates
  produced by inconsistent quotes are reported as a typed error (or floored
  at zero on explicit request).

### M6 — Forwards, dividends and repo

- Equity forward, defined by growth between ex-dates and jumps at ex-dates
  (deterministic rates):

```math
F(0) = S_0, \qquad F(t) = F(s)\,\frac{P(s)}{P(t)}\,\exp\!\Big(-\int_s^t \big(q(u) + b(u)\big)\,du\Big) \quad \text{for } t_i \le s \le t < t_{i+1}
```

```math
F(t_i^{+}) = (1 - \delta_i)\,F(t_i^{-}) - D_i\,\frac{P(t_i^{pay})}{P(t_i)}
```

  with ex-dates tᵢ, cash dividends Dᵢ paid at their payment dates,
  proportional dividends δᵢ, continuous dividend yield q and repo or borrow
  spread b; the forward for an ex-date T already includes the dividend going
  ex on T; when a cash and a proportional dividend share an ex-date the
  proportional one applies first (kontract's own convention, documented).
- Dated carry items apply up to and including their date; an open-ended item
  applies after the last dated item; without an open-ended item, carry is
  zero after the last date; repo and dividend items add.
- FX forward: F = S × P_foreign(T) / P_domestic(T) with spot-date
  conventions of time.md T4.3; forward points = (F - S) / pip size.

## Edge cases

- Quote file: missing header, unknown column, unknown quote kind, unknown
  unit, malformed number, duplicate identifiers, empty file, comment-only
  file, Windows line endings, an `asof` differing between lines (typed error).
- Identifiers: correlation of a factor with itself (must be one or
  rejected), inverted FX pairs, identifiers with lower-case segments
  (canonicalised).
- Fixings: a fixing date that is a holiday, a fixing requested after the
  as-of date (not a past fixing: typed error), duplicate fixings.
- Bumps: relative bump of a zero quote, vol bump below zero (floored),
  correlation bump beyond one (clamped), bump of an absent quote (no-op,
  reported).
- Curves: a single instrument; instruments with equal maturities (one is
  dropped and reported); negative rates; inverted curves; very high rates;
  futures-heavy front ends; a requested date beyond the last pillar
  (extrapolation rule); a date equal to the as-of date (P = 1).
- Multi-curve: a projection tenor without its own instruments (fallback with
  warning); a cross-currency set requiring multi-curve while multi-curve is
  disabled (typed error).
- Bonds: settlement on a coupon date, in the ex-coupon period, in a short or
  long first coupon period, a zero-coupon bond, a price implying a negative
  yield.
- Volatility: zero and extremely small or large volatilities; deep in- and
  out-of-the-money options where the price is below the representable time
  value (inversion must return a typed result, not NaN); zero time to
  expiry; negative strikes with Bachelier; normal-to-Black conversion with no
  solution; FX premium-adjusted delta where the delta-to-strike map is not
  monotone (two solutions: the documented one is returned); SVI parameters at
  the edge of the admissible set; a smile slice with one point; a fully flat
  slice.
- Caplet stripping with the first caplet already fixed; a cap matrix with a
  missing strike column.
- Credit: very low and very high spreads, a negative notional (sign handled
  consistently), trade dates around a roll date, protection starting after
  the trade date, an upfront quote without a standard coupon (typed error),
  a zero recovery and a recovery close to one.
- Dividends: an ex-date on the as-of date, on the forward date, a dividend
  larger than the forward (typed error), no open-ended carry item, only
  projected dividends beyond the cut-off.

## Test scenarios

Each scenario states its independent justification; no scenario reuses data
produced by the reference library. Tests use small synthetic fixtures written
for kontract.

### M1 — tests/market_data.rs

- S-M1.1 Round trip (identity): write then read a snapshot covering every
  quote kind; the result equals the original; the serde form round-trips too.
- S-M1.2 Validation (definition): each malformed line of a hand-written file
  yields an error with the right line number and reason, and all errors are
  reported in one pass.
- S-M1.3 Query equivalence (property-based): filtering through the indexed
  query path returns the same records as a naive scan over random queries and
  random snapshots.
- S-M1.4 Canonicalisation (identity): a correlation stored for (A, B) is
  returned for (B, A); inverting one FX leg flips its sign; FX inverse is the
  reciprocal; identical currencies give one.
- S-M1.5 Bumps and scenarios (hand computation): absolute and relative bumps,
  futures in rate space, yield shift of a zero-coupon price, floors and
  clamps, sequential and product combinations, against results computed by
  hand in the test.
- S-M1.6 Projected paths (hand computation): linear and compounded
  interpolation between anchors, repeated steps, overriding rules and flat
  extension.

### M2 — tests/market_curves.rs

- S-M2.1 Repricing (identity): every input instrument (deposits, FRAs,
  futures, swaps, OIS, basis and cross-currency swaps) reprices to its quote
  on the built curve, for single-curve and multi-curve set-ups, for
  normal, inverted, high-rate and negative-rate synthetic markets.
- S-M2.2 Deposits only (closed form): each discount factor equals
  1 / (1 + r·τ) at the deposit maturity under the deposit's day count and spot
  lag.
- S-M2.3 Flat curve (identity): with every quote consistent with a flat
  continuously compounded rate r, P(t) = exp(-r·t), all forwards equal the
  flat forward and zero rates equal r, for every interpolation scheme.
- S-M2.4 Interpolation properties (published and identity): log-linear
  discount factors give piecewise-constant forwards; the monotone convex
  scheme reproduces the worked example and the stated properties of Hagan and
  West (2006, 2008).
- S-M2.5 Futures convexity (closed form): the Ho-Lee and Hull-White
  adjustments match the closed forms in Hull's *Options, Futures and Other
  Derivatives* and in Kirikos and Novak (1997); zero volatility gives zero
  adjustment.
- S-M2.6 Cross-currency (identity): every cross-currency, basis and plain
  swap reprices with either currency as reference, with and without forward
  points up to a cut-off; covered interest parity holds at the short end.
- S-M2.7 Jacobian (independent): the analytic or automatic Jacobian of zero
  rates with respect to quotes matches central finite differences of a
  re-bootstrap; the Jacobian times its inverse is the identity.
- S-M2.8 Monte-Carlo consistency (independent): discount factors computed
  by simulating a deterministic short rate on the curve equal the curve's
  discount factors.

### M3 — tests/market_instruments.rs

- S-M3.1 Par swaps (identity): par rate gives zero PV; PV is linear in the
  fixed rate with slope minus the annuity; single-curve par rate equals the
  discount-factor formula.
- S-M3.2 Textbook bonds (published): the price, yield, accrued interest,
  Macaulay and modified duration and convexity examples of Fabozzi's *Bond
  Markets, Analysis and Strategies* and of Hull's textbook.
- S-M3.3 Round trips (identity): price → yield → price and price → Z-spread
  → price; the Z-spread on a curve built from the bond's own discounting is
  zero; duration equals minus the derivative of log price with respect to
  yield (finite difference in the test).
- S-M3.4 FRA (closed form): FRA settlement amount from the ISDA formula
  against a hand computation.
- S-M3.5 Cross-currency swap (hand computation): constant and resetting
  notional legs valued by an independent cash-flow sum in the test; zero
  spread gives zero value on consistent curves.
- S-M3.6 Amortisation (identity): an amortising swap equals the sum of
  bullet swaps on the notional decrements.

### M4 — tests/market_volatility.rs

- S-M4.1 Formulas (published and identity): Black, Bachelier and shifted Black
  match the examples of Haug's *The Complete Guide to Option Pricing Formulas*
  and Hull's textbook; put-call parity; Bachelier limit of Black for small
  volatility relative to forward; Monte-Carlo check of Bachelier with a
  stated standard-error tolerance.
- S-M4.2 Inversion round trips (identity): implied volatility of the formula
  price recovers the input volatility over a wide grid of moneyness,
  expiries and volatilities, including the tails, for Black, Bachelier and
  shifted Black; prices below intrinsic and above the upper bound give the
  typed results.
- S-M4.3 Conversions (identity): Black → normal → Black returns the input;
  converted quotes reprice identically; calibrating a model (in later units)
  to Black or to converted normal quotes gives the same parameters.
- S-M4.4 FX smile (published): the smile construction and delta conventions
  match the worked examples of Clark's *Foreign Exchange Option Pricing*,
  Reiswich and Wystup (2010) and Castagna and Mercurio (2007); inverting the
  pair gives reciprocal strikes and identical volatilities.
- S-M4.5 SVI and SSVI (published and identity): the SVI parametrisations and
  arbitrage examples of Gatheral and Jacquier (2014) are reproduced (the
  arbitrable example is detected, the repaired one passes); a synthetic
  surface generated from known SVI parameters is fitted back to them.
- S-M4.6 Calendar and butterfly detection (identity): surfaces built in the
  test with a crossing total variance or a negative density are flagged; a
  surface from a single Black volatility is arbitrage-free.
- S-M4.7 Implied forward (identity): on synthetic call and put prices built
  from a known forward and discount factor, the put-call parity regression
  recovers them; American prices from a fine binomial tree in the test are
  inverted back to the generating volatility.
- S-M4.8 Caplet stripping (identity): stripped caplets reprice the input caps;
  the three smiled interpolation schemes give the same cap volatilities at the
  input maturities; the ATM vega-weighted method of Alexander (2003)
  reproduces flat caplet volatilities from a flat cap matrix.
- S-M4.9 SABR (published and identity): the Hagan et al. (2002) and Obłój
  (2008) formulas reproduce the reference examples given in those papers;
  the at-the-money volatility to alpha conversion round-trips; with zero
  vol-of-vol and β = 1 SABR reduces to Black.

### M5 — tests/market_credit.rs

- S-M5.1 ISDA Standard Model (published): the upfront, clean and dirty price,
  accrued premium and cash-settlement amount of the public test cases
  distributed with the ISDA CDS Standard Model and its documentation,
  including trade dates just before, on and after a roll date and spreads
  above, at and below the coupon.
- S-M5.2 Repricing (identity): the bootstrapped hazard curve reprices every
  input par spread and every input upfront.
- S-M5.3 Flat hazard (closed form): with flat hazard λ, constant recovery R
  and continuous premium payment, the par spread is λ(1 - R) (the credit
  triangle); survival is exp(-λt); the risky annuity matches a closed-form
  integral computed in the test.

```math
s_{\text{par}} = \lambda\,(1 - R) \quad\text{(continuous premium, flat hazard, flat rates)}
```

- S-M5.4 Conversions (identity): spread → upfront → spread returns the
  input; a contract whose coupon equals its par spread has zero upfront
  (accrual aside); conventional-spread conversions agree with the ISDA
  converter conventions.
- S-M5.5 Monotonicity (identity): survival is non-increasing; upfront
  increases with spread; CS01 has the expected sign.

### M6 — tests/market_equity.rs

- S-M6.1 Cost of carry (identity): with a flat rate and continuous yield
  only, F = S·exp((r - q - b)T); with one cash dividend, the forward drops by
  the dividend value at the ex-date across the ex-date, and equals the
  textbook escrowed-dividend forward (Hull); with proportional dividends, it
  drops by the proportion; mixed cases follow the recursion of Behaviour M6,
  checked against a hand computation in the test.
- S-M6.2 Implied dividends (identity, property-based): forwards built from
  implied dividend yields reproduce those yields; dividends implied from
  forwards rebuild the same forwards.
- S-M6.3 Carry semantics (definition): dated items apply up to and including
  their date, the open-ended item afterwards, zero carry after the last date
  without one, and repo and dividend items add.
- S-M6.4 Monte-Carlo consistency (independent, later units): the simulated
  expected spot equals the deterministic forward within standard error with
  continuous, discrete and mixed dividends; a Margrabe exchange option with
  dividend yields matches its closed form.
- S-M6.5 FX forwards (identity): covered interest parity; forward points
  round-trip with outrights.

## References

- J. C. Hull, *Options, Futures, and Other Derivatives*, 10th edition,
  Pearson, 2017 (forwards, futures convexity, bonds, Black's model, CDS).
- F. Black, "The pricing of commodity contracts", *Journal of Financial
  Economics* 3, 1976; L. Bachelier, *Théorie de la spéculation*, 1900.
- E. G. Haug, *The Complete Guide to Option Pricing Formulas*, 2nd edition,
  McGraw-Hill, 2007.
- P. Jäckel, "Let's be rational", *Wilmott* 2015; P. Jäckel, "Implied normal
  volatility", *Wilmott* 2017.
- P. S. Hagan and G. West, "Interpolation methods for curve construction",
  *Applied Mathematical Finance* 13, 2006; "Methods for constructing a yield
  curve", *Wilmott* 2008.
- M. Henrard, *Interest Rate Modelling in the Multi-curve Framework*,
  Palgrave Macmillan, 2014; F. M. Ametrano and M. Bianchetti, "Everything you
  always wanted to know about multiple interest rate curve bootstrapping but
  were afraid to ask", SSRN 2219548, 2013.
- OpenGamma, *Interest Rate Instruments and Market Conventions Guide*, 2013.
- G. Kirikos and D. Novak, "Convexity conundrums", *Risk* 10, 1997.
- L. Andersen and V. Piterbarg, *Interest Rate Modeling*, Atlantic Financial
  Press, 2010 (curve construction, futures convexity, vanilla volatility
  structures).
- F. J. Fabozzi, *Bond Markets, Analysis and Strategies*, 9th edition,
  Pearson, 2015; F. J. Fabozzi (ed.), *The Handbook of Fixed Income
  Securities*, McGraw-Hill.
- ISDA, *2006 ISDA Definitions* (FRA discounting, floating rate options);
  ISDA, *2021 ISDA Interest Rate Derivatives Definitions*.
- J. Gatheral, "A parsimonious arbitrage-free implied volatility
  parameterization with application to the valuation of volatility
  derivatives", Global Derivatives, 2004; J. Gatheral, *The Volatility
  Surface*, Wiley, 2006; J. Gatheral and A. Jacquier, "Arbitrage-free SVI
  volatility surfaces", *Quantitative Finance* 14, 2014.
- S. De Marco and C. Martini, "Quasi-explicit calibration of Gatheral's SVI
  model", Zeliade white paper, 2009.
- B. Dupire, "Pricing with a smile", *Risk* 7, 1994 (density and arbitrage
  conditions, used here for checks only).
- I. Clark, *Foreign Exchange Option Pricing: A Practitioner's Guide*,
  Wiley, 2011; D. Reiswich and U. Wystup, "A guide to FX options quoting
  conventions", *Journal of Derivatives* 18, 2010; A. Castagna and F.
  Mercurio, "The vanna-volga method for implied volatilities", *Risk*, 2007.
- C. Alexander, "Common correlation and calibrating the lognormal forward
  rate model", *Wilmott* 2003 (caplet stripping).
- P. S. Hagan, D. Kumar, A. S. Lesniewski and D. E. Woodward, "Managing smile
  risk", *Wilmott* 2002; J. Obłój, "Fine-tune your smile: correction to Hagan
  et al.", *Wilmott* 2008.
- J. C. Cox, S. A. Ross and M. Rubinstein, "Option pricing: a simplified
  approach", *Journal of Financial Economics* 7, 1979; D. Leisen and M.
  Reimer, "Binomial models for option valuation — examining and improving
  convergence", *Applied Mathematical Finance* 3, 1996.
- M. Rubinstein, "Displaced diffusion option pricing", *Journal of Finance*
  38, 1983.
- ISDA CDS Standard Model, source code documentation and test cases
  (cdsmodel.com, public); ISDA, *Standard North American Corporate CDS
  Contract Specification*; Markit, *The CDS Big Bang* and *CDS Small Bang*
  documentation, 2009.
- D. O'Kane, *Modelling Single-name and Multi-name Credit Derivatives*,
  Wiley, 2008 (hazard curves, CDS legs, risky annuity, credit triangle).
- H. Bühler, "Volatility and dividends — volatility modelling with cash
  dividends and simple credit risk", SSRN 1141877, 2010; H.-P. Overhaus et
  al., *Equity Hybrid Derivatives*, Wiley, 2007 (dividend modelling).
- W. Margrabe, "The value of an option to exchange one asset for another",
  *Journal of Finance* 33, 1978.
- RFC 4180, *Common Format and MIME Type for Comma-Separated Values (CSV)
  Files*, 2005.
