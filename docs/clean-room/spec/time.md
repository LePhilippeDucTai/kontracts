# Functional specification — dates, calendars, schedules and currencies

Status: dirty-room functional specification for the `clean-room-core` plan,
unit J01-P2. It states **what** kontract v0.3 must provide for dates and
market conventions, never **how** a reference implementation does it.
Implementers follow `docs/clean-room/PROTOCOL.md`, choose their own names and
structure, and take every convention, holiday rule and per-currency default
from the public sources cited here (ISDA definitions, ISO standards, central
bank and exchange publications), with a citation in the doc comment.

Scope: civil dates and date-times, date arithmetic, parsing and formatting,
tenors and periods, day-count conventions, business-day conventions and date
offsets, rule-based holiday calendars for a list of financial centres, joint
calendars, schedule generation (stubs, rolls, IMM, CDS and futures dates,
fixing and payment lags), and currencies with their market conventions.

Global conventions:

- Dates are proleptic Gregorian civil dates without time zone. The supported
  range is at least 1900-01-01 to 2199-12-31; constructing a date outside
  the range, or an invalid civil date (for example 30 February), is a typed
  error, never a silent normalisation.
- Every fallible operation returns a typed error of the crate-wide error
  type. No operation panics on user input.
- All types in this spec (dates, tenors, conventions, calendars, schedules,
  currencies) are serialisable and deserialisable losslessly (serde), and
  have a canonical text form that round-trips.
- Rule-based data (holiday rules, per-currency conventions) live in data
  tables with a cited source per entry, so that a change of market practice
  is a data change, not a code change.

Mapping onto the plan:

| Group | Capability group | Plan unit |
|-------|------------------|-----------|
| T1 | Civil dates, date-times, arithmetic, parsing and formatting | J03-P1 |
| T2 | Tenors, periods and frequencies | J03-P1 |
| T3 | Day-count conventions | J03-P1 |
| T4 | Currencies and market conventions | J03-P1 |
| T5 | Holiday calendars and joint calendars | J03-P2 |
| T6 | Business-day conventions, adjustment and offsets | J03-P2 |
| T7 | Schedule generation and standard dates | J03-P3 |

## Capabilities

### T1 — Civil dates, date-times, arithmetic, parsing and formatting (J03-P1)

- T1.1 Civil date type with year, month (1 to 12) and day accessors, total
  ordering, hashing, and a day serial number with a documented epoch
  (kontract's own choice) converting both ways.
- T1.2 Date-time type at minute resolution for intraday events (fixing times,
  exercise cut-offs), naive with respect to time zones: adding hours never
  applies daylight saving; conversion to and from Unix-epoch milliseconds;
  normalisation of a date-time to the start of its day.
- T1.3 Calendar facts: leap-year test (Gregorian rule), days in month, last
  day of month, "is month end", push to month end, weekday (Monday to Sunday)
  and ISO-8601 weekday number, day of year, ISO week number.
- T1.4 Arithmetic: add or subtract days; difference in days (signed integer
  for dates, fractional for date-times); add or subtract months with the day
  capped at the end of the target month; add years as twelve months; an
  optional end-of-month rule under which a month-end start date always maps
  to the target month end.
- T1.5 Splitting an interval into steps of a given tenor with the end date
  always included as last point, and enumerating every day of an inclusive
  range.
- T1.6 Text forms: ISO 8601 calendar dates (YYYY-MM-DD) and date-times
  (YYYY-MM-DDThh:mm[:ss], with a space accepted instead of T) as the canonical
  format; formatting through a template language with tokens for day, month
  number, two- and four-digit year, English weekday name and English month
  name (full and three-letter); English three-letter upper-case month codes.
- T1.7 Tolerant parsing of human-entered dates, kontract's own grammar built
  on these accepted forms: ISO; month name, day (optional English ordinal
  suffix), four-digit year; day, month name, two- or four-digit year; numeric
  day, month, year (day first by default, with an explicit month-first mode);
  numeric year, month, day. Separators: space, comma, dot, slash, hyphen.
  Month names are case-insensitive in English (full and abbreviated), French
  and Dutch. Two-digit years follow a documented pivot rule. A mode that
  extracts every date found in a free text is optional.

### T2 — Tenors, periods and frequencies (J03-P1)

- T2.1 Tenor value: a signed count of days, weeks, months or years, plus the
  money-market special tenors overnight, tomorrow-next and spot-next.
  Compound tenors (for example one year and six months) are supported.
- T2.2 Tenor text form: an optional sign, an integer and a unit letter (D, W,
  M, Y, case-insensitive), concatenable for compound tenors, plus the codes
  ON, TN and SN; zero tenor; parsing a field that may hold either an explicit
  ISO date or a tenor relative to a base date.
- T2.3 Tenor arithmetic: date plus tenor (days and weeks exactly, months and
  years with the T1.4 rules), tenor negation, tenor multiplication by an
  integer, normalisation (twelve months equals one year, seven days equals one
  week where the caller asks), comparison only when unambiguous (a typed error
  otherwise, for example comparing one month with thirty days).
- T2.4 Bracketing a span by standard tenors: given a span between two dates,
  find the standard week or month tenors immediately below and above it,
  used to interpolate stub rates linearly between two index tenors (ISDA 2006
  Definitions, section 8.3); spans outside the supported tenor range are a
  typed error.
- T2.5 Period value (start date, end date with start ≤ end) and conversion
  of a strictly increasing date list into consecutive periods.
- T2.6 Frequency: every n days, every n weeks, every n months (with the usual
  named cases: annual, semi-annual, quarterly, monthly), or a single period
  for the whole term ("zero coupon" or "term"); conversion between frequency
  and number of periods per year where defined.

### T3 — Day-count conventions (J03-P1)

- T3.1 Year fraction between two dates for each of these conventions, named
  as in the ISDA 2006 Definitions section 4.16 or the cited market source:
  ACT/360; ACT/365 Fixed; ACT/364; ACT/365L (ICMA rule 251, leap-year-sensitive
  denominator); ACT/ACT ISDA; ACT/ACT ICMA (ICMA rule 251, with regular
  reference period and frequency); ACT/ACT AFB (French banking federation
  convention); 30/360 Bond Basis (ISDA 4.16(f)); 30/360 US (SIA rule, with the
  February end-of-month adjustments); 30E/360 Eurobond (ISDA 4.16(g)); 30E/360
  ISDA (ISDA 4.16(h), February end treated as day 30 except for the
  termination date); 30E+/360; NL/365 and NL/360 (actual days excluding each
  29 February in the period, over 365 or 360); actual days over the length of
  the calendar year containing the end date, and actual days over the length
  of the one-year period ending on the end date (both found in older European
  conventions); BUS/252 (business days of a given calendar over 252, used in
  Brazil); 1/1; and a user-supplied convention through a trait.
- T3.2 Day-count options: by default the start date is included and the end
  date excluded; options to include both ends, or to add one day.
- T3.3 Per convention: the day count numerator (days in the period under the
  convention), the year fraction, and the accrual ratio used for accrued
  interest (the fraction of the current coupon period elapsed, which for ACT/ACT
  ICMA is days elapsed over days in the reference period).
- T3.4 Each convention has a canonical name and accepted aliases (for example
  ACT/365F, A365F, Actual/365 (Fixed)) parsing to it; the canonical name
  round-trips.

### T4 — Currencies and market conventions (J03-P1)

- T4.1 Currency type for the ISO 4217 alphabetic codes in current use, the
  legacy European currencies replaced by the euro (for historical data), the
  precious-metal codes (gold, silver, platinum, palladium), the offshore
  Chinese yuan, sub-unit pseudo-currencies quoted as a fixed fraction of a
  main currency (for example pence for sterling, with its factor), and an
  open extension for arbitrary asset units; ISO numeric code and minor units
  (decimals) from ISO 4217; text round trip.
- T4.2 Amount rounding: up, down, half-up and half-even to a number of
  decimals, and the per-currency rounding of settlement amounts (ISO 4217
  minor units by default, with the ISDA 2006 section 8.1 rounding rule).
- T4.3 Per-currency market conventions, stored as cited data, for at least
  USD, EUR, GBP, JPY, CHF, CAD, AUD, NZD, HKD, SGD, SEK, NOK, DKK, KRW, CNY and
  CNH where the market exists:
  - money market deposits: calendar(s), spot lag in business days, day count,
    business-day convention, end-of-month rule;
  - interest-rate futures: calendar, contract months, last-trading-day rule,
    settlement lag, underlying tenor;
  - interest-rate swaps (main IBOR or risk-free-rate swap): calendar(s), spot
    lag, business-day convention, fixed-leg day count and frequency,
    floating-leg index, day count and frequency, with exceptions by maturity
    range where the market uses them;
  - overnight index swaps: index (for example SOFR, €STR, SONIA, TONA, SARON),
    day count, payment frequency, payment lag, rate cut-off;
  - tenor basis swaps and cross-currency swaps: per-leg conventions, which leg
    carries the spread, notional exchange kind (constant, mark-to-market
    resetting, non-deliverable);
  - default discounting choice for collateralised instruments (overnight
    curve or term-rate curve) per instrument family;
  - rate indices: overnight and term (IBOR-type) indices by tenor with fixing
    calendar, fixing lag, value-date rules and the rounding of published
    fixings; CMS index definition (underlying swap conventions);
  - FX pairs: market quotation order of a pair (which currency is the base),
    spot lag, calendars of both currencies plus the settlement-currency rule,
    pip size, premium currency and default delta and at-the-money conventions
    used by volatility quotes (consumed by market.md);
  - government bond markets (at least US Treasury, UK Gilt, German Bund and
    Bobl, French OAT): coupon frequency, day count, settlement lag,
    business-day convention, calendar, ex-coupon period where it exists, as
    currently practised (not as historically practised).
- T4.4 Default calendar of a currency (for example USD to New York, EUR to
  TARGET), overridable per instrument.

### T5 — Holiday calendars and joint calendars (J03-P2)

- T5.1 Calendar value: a named built-in centre, an explicit list of holiday
  dates with a weekend definition, or a joint calendar of several calendars.
  A day is a business day of a joint calendar only when it is a business day
  of every member (union of holidays). An "open in at least one member"
  combination (intersection of holidays) is also provided.
- T5.2 Calendar text form, kontract's own design: centre codes combined with
  "+" for a joint calendar, and an explicit-dates form, both round-tripping.
  Unknown names are a typed error. An empty calendar (weekends only, or no
  holiday at all, selectable) exists.
- T5.3 Business-day queries: is business day, is holiday, next and previous
  business day, list of business days in an inclusive range, count of
  business days between two dates (start included, end excluded by default),
  and the list of holidays in a range.
- T5.4 Rule families from which centre calendars are built:
  - weekend days per centre (Saturday-Sunday by default; other weekends
    possible);
  - fixed day of month, with year-validity ranges;
  - observance rules for a fixed holiday falling on a weekend: none; move to
    the next business day; move to the nearest weekday (Saturday to Friday,
    Sunday to Monday); move only when on Sunday; and cascading substitutes,
    so that two consecutive holidays both on the weekend take two distinct
    substitute days (for example the British Christmas and Boxing Day rule);
  - n-th given weekday of a month and last given weekday of a month;
  - Easter-relative days (Maundy Thursday, Good Friday, Easter Monday,
    Ascension Thursday, Whit Monday, Corpus Christi) using the Gregorian
    computus;
  - East-Asian lunisolar holidays (Lunar New Year with its multi-day rules,
    Buddha's Birthday, Dragon Boat (Tuen Ng) festival, Mid-Autumn festival and
    the day after, Chung Yeung, Korean Chuseok and Seollal) from a published
    conversion table or algorithm with its cited source and its supported
    year range;
  - solar-term holidays (Ching Ming / Qingming) and the Japanese vernal and
    autumnal equinox days, from the official published dates or the
    astronomical formula cited by the official source;
  - weekday-dependent national rules: the Japanese "Happy Monday" system, the
    Japanese substitute holiday and the citizens' holiday between two
    holidays, the Swedish Midsummer Eve, the last business day of the year
    when a market closes on it;
  - rules that changed at a given year (applied by validity ranges);
  - one-off closures (royal events, national days of mourning, elections,
    extreme-weather closures, exchange closures) as dated entries with
    their source.
- T5.5 Built-in financial centres, each with its official holiday source:
  TARGET (euro area payment system), London, New York (banking days; the
  exchange calendar as a separate centre), Tokyo, Zurich, Stockholm, Oslo,
  Copenhagen, Sydney, Toronto, Hong Kong, Singapore, Seoul, Taipei, Paris,
  Frankfurt, Amsterdam (the exchanges of Paris and Amsterdam follow the
  TARGET closing days; the Frankfurt exchange has its own rules), and
  Wellington. The registry is extensible at run time by user-defined
  calendars.
- T5.6 Rule calendars are materialised into holiday sets for a horizon that
  covers at least the supported date range; queries outside the materialised
  horizon are a typed error, not a silent "business day".

### T6 — Business-day conventions, adjustment and offsets (J03-P2)

- T6.1 Business-day conventions: unadjusted, following, modified following,
  preceding, modified preceding (ISDA 2006 Definitions, business-day
  conventions), and the end-of-month variants used by money-market and bond
  schedules.
- T6.2 Adjustment rule = convention plus a calendar (joint if several are
  given); adjusting a date under a rule.
- T6.3 Offsets: a signed number of business days or of calendar days, the
  calendar-day variant optionally followed by an adjustment; the two
  compositions of an adjustment with an offset (adjustment first, or offset
  first); adding n
  business days, where n = 0 leaves the date unchanged even when it is a
  holiday (the caller adjusts explicitly if needed).
- T6.4 Business days in a range with each holiday replaced by the preceding
  business day (the "previous good business day" view used for fixings).

### T7 — Schedule generation and standard dates (J03-P3)

- T7.1 Periodic schedule from an effective date to a termination date with a
  frequency, generated forward from the effective date or backward from the
  termination date, every unadjusted date being computed from the anchor as
  anchor ± i × period (never by iterating month additions, so that day capping
  does not drift).
- T7.2 Stubs: none, short or long at the front, short or long at the back,
  specified either by a stub type or by an explicit first regular period start
  date and/or last regular period end date; stubs appear only at the ends; a
  termination date inconsistent with the frequency and no stub allowed is a
  typed error.
- T7.3 Roll conventions: a day-of-month roll (1 to 30), end-of-month roll,
  IMM roll (third Wednesday), CDS roll (the 20th of the quarter months), and
  a no-roll mode that takes the day from the anchor; end-of-month roll is
  allowed only when the anchor is a month end.
- T7.4 Adjustment: unadjusted and adjusted dates are both returned;
  intermediate dates use the period adjustment rule while the effective,
  termination and stub dates may use their own rules; both the unadjusted and
  the adjusted sequences are strictly increasing (a collapse after adjustment
  is a typed error).
- T7.5 Payment dates relative to period start or end, with a business-day lag
  and their own adjustment rule; compounding schedules where several
  calculation periods map to one payment.
- T7.6 Reset and fixing dates: fixing in advance or in arrears, fixing lag in
  business days on the fixing calendar, several resets per period for
  compounded or averaged legs, and the observation-shift and lookback
  conventions of overnight-rate compounding (ISDA 2021 Definitions).
- T7.7 Index accrual periods: for a term (IBOR-type) index, the fixing date is
  the reset date minus the spot lag in business days and the index end date
  is the value date plus the index tenor, adjusted; for an overnight index,
  the accrual is one business day of the index calendar; for a CMS index, the
  underlying swap's periods (start, end and payment dates) follow the
  conventions of the CMS index (T4.3).
- T7.8 Money-market dates: spot date = trade date plus the spot lag in
  business days (zero for overnight, one for tomorrow-next); overnight,
  tomorrow-next and spot-next mature one business day after their start;
  week tenors mature at spot plus seven days times n, month and year tenors
  at spot plus the tenor under the modified-following and end-of-month rules
  of the currency (T4.3).
- T7.9 IMM dates: third Wednesday of a month; next and previous IMM date
  from a date (quarterly or monthly cycle); IMM codes (month letter and year
  digit) both ways; futures last trading day as a number of business days
  before the IMM date and settlement as a lag after it.
- T7.10 CDS standard dates: the quarterly 20 March, 20 June, 20 September and
  20 December roll dates; next roll date strictly after a date; the standard
  maturity for a tenor, including the semi-annual roll of standard maturities
  in force since December 2015; step-in date (trade date plus one calendar
  day), accrual start (previous roll date, adjusted), cash-settlement date
  (three business days after trade date) under the ISDA Standard CDS
  conventions.
- T7.11 Futures and commodity last-trading-day rule language, covering the
  rule families found in exchange rulebooks: n-th day or n-th weekday of the
  contract month or of a month offset from it, n business days before a given
  day or before the end of the contract or preceding month, start or end of
  month, following or preceding adjustment, with exceptions per month.
- T7.12 Schedule output: for each period, unadjusted and adjusted start and
  end, payment date, fixing dates, day-count fraction under a supplied
  convention, and a flag for stub periods; serialisable.

## Behaviour

### T1 — Dates

- Adding months: compute target year and month, then use min(day, days in
  target month). This operation is neither associative nor invertible
  (adding one month to 31 January then subtracting one month gives
  28 or 29 January); the end-of-month rule is opt-in and documented.
- Difference of two dates is the difference of their serial numbers.
- Formatting then parsing in the same template returns the same date.
- The tolerant parser never returns a wrong date silently: in day-first mode,
  a first number above 12 is a day and a second number above 12 forces the
  month-first reading; an impossible reading is skipped, and an input with no
  valid reading is a typed error (in extraction mode it yields nothing).

### T2 — Tenors

- "1M" parses to one month; a bare unit letter means one unit; "0" (or
  "0D") is the zero tenor; unit letters are case-insensitive; ON, TN and SN
  are only valid where a money-market start rule is defined (T7.8).
- Adding a tenor to a date and then its negation does not in general give the
  original date for month tenors (T1.4); for day and week tenors it does.

### T3 — Day counts

- Each convention follows the cited definition exactly, including all
  end-of-month and February rules; the definitions of ISDA 2006 section 4.16,
  ICMA rule 251 and the ISDA 1999 memo "EMU and market conventions: recent
  developments" (ACT/ACT comparison) are normative.
- ACT/ACT ISDA splits the period at year boundaries: days in a leap year
  over 366 plus days in a non-leap year over 365.
- ACT/ACT ICMA requires a regular reference period (or a frequency, from
  which regular periods are derived from the schedule); for a stub it sums
  the fractions of the notional regular periods it overlaps. Without
  reference dates and frequency the convention is a typed error.
- ACT/365L uses 366 when 29 February falls in the period (annual coupon) or
  when the period end is in a leap year (other frequencies), per ICMA.
- NL conventions exclude every 29 February in the period and divide by the
  denominator named in the convention.
- BUS/252 counts business days of the attached calendar in [start, end).
- 1/1 returns one for any non-empty period (and zero for an empty one).
- A start date after the end date is a typed error, except in a separate
  signed form that returns the negative of the reversed fraction.

### T4 — Currencies

- Currency codes are three upper-case letters; parsing is case-insensitive;
  unknown codes are a typed error unless the extension mechanism registers
  them.
- A sub-unit pseudo-currency converts amounts to its main currency by its
  fixed factor and shares its calendar and conventions.

### T5 — Calendars

- A built-in centre's holiday set for a year equals the official list
  published for that year by its cited source (central bank, government or
  exchange), including substitute days.
- Gregorian computus (Meeus/Jones/Butcher algorithm) gives Easter Sunday for
  every year of the supported range.
- Lunisolar holidays are defined only within the range of the cited table or
  algorithm; outside it the calendar reports a typed error (no guessed
  dates).
- A joint calendar's holidays are exactly the union of its members'.
- Business-day counting between d and d is zero; between d and the next
  business day is one when d is a business day.

### T6 — Adjustment

- Following: the first business day on or after the date. Preceding: the
  last business day on or before. Modified following: following unless that
  moves into the next month, then preceding. Modified preceding: preceding
  unless that moves into the previous month, then following.
- A business day is left unchanged by every convention.
- An adjustment rule with a convention other than "unadjusted" and no
  calendar is a typed error; an "unadjusted" rule ignores its calendar and
  says so in its text form (kontract's own choice, avoiding silent
  misconfiguration).
- Adding n > 0 business days to a holiday first moves to the next business
  day and counts from there, so the result is a business day; negative n is
  symmetric.

### T7 — Schedules

- Backward generation (the market default for swaps) puts any stub at the
  front; forward generation puts it at the back.
- With an end-of-month roll, every unadjusted date that is not the effective
  or termination date is a month end.
- The CDS standard schedule is generated backward from the maturity on the
  20th of the quarter months, with a short front stub by default, payment
  dates adjusted following, the last accrual period ending on the maturity
  date included (one extra day), per the ISDA Standard CDS conventions.
- Futures settlement dates and the IMM dates used by curve building agree.
- Every generated schedule is serialisable, and generating twice from the
  same inputs gives identical output.

## Edge cases

- 29 February: adding one year gives 28 February; adding four years gives
  29 February; ACT/ACT ISDA and ACT/365L over periods containing it; NL
  conventions over periods starting or ending on it.
- Month ends: 31 January plus one month (28 or 29 February); 30 April plus
  one month with and without end-of-month rule; 28 February in a non-leap
  year treated as month end under end-of-month rules and under 30E/360 ISDA
  and 30/360 US.
- Day 31 in 30/360 variants for start, end and both.
- Zero-length periods (year fraction zero for every convention) and one-day
  periods.
- Long spans (several decades) and spans crossing century years that are not
  leap years (1900, 2100).
- Weekends: trade dates on Saturday or Sunday; business-day adjustment at a
  month boundary under modified following and modified preceding; offsets of
  zero business days from a holiday; negative offsets across a year end.
- Holidays falling on weekends (substitute days) and two consecutive
  weekend holidays (cascading substitutes); New Year on Saturday under the
  "Sunday only" rule (no substitute).
- Joint calendars where one member is closed and the other open; a joint
  calendar of a centre with itself (identical to the centre).
- Easter at its earliest and latest possible dates.
- Lunisolar holidays at the edges of the supported table range.
- Rules whose definition changed in a given year (both sides of the change).
- Schedules whose termination date is a month end while the effective date
  is not; schedules with a front and a back stub at once; schedules whose
  adjusted dates collide (error); schedules of one period; zero-coupon
  frequency.
- CDS dates for trade dates the day before, on and the day after a roll
  date, and around the semi-annual maturity roll.
- IMM date when the input date is itself an IMM date (next is strictly
  later).
- Parsing: invalid day for a month, two-digit years around the pivot,
  month names with different case, ordinal suffixes, ambiguous numeric dates.
- Tenor parsing: empty string, unknown unit, overflow, negative tenors.

## Test scenarios

Tests live in tests/time_daycount.rs (T1–T4), tests/time_calendar.rs (T5–T6)
and tests/time_schedule.rs (T7). Each scenario states its justification.

### T1–T2 — Dates and tenors

- S-T1.1 Serial round trip (identity): for every date of the supported range,
  converting to the serial number and back gives the same date, and
  consecutive dates have consecutive serial numbers; the weekday advances by
  one each day and matches the published weekday of a few anchor dates
  (ISO 8601 annex examples).
- S-T1.2 Leap years (definition): the Gregorian rule (divisible by 4, except
  centuries not divisible by 400) on century and non-century years.
- S-T1.3 Month arithmetic (hand computation): the T1.4 capping rule on all
  month ends, with and without the end-of-month rule; adding years equals
  adding twelve times as many months.
- S-T1.4 Text round trip (identity): format then parse in each template;
  the tolerant parser on a hand-written corpus covering every accepted form,
  separator and language, and rejecting impossible dates.
- S-T1.5 Unix time (identity): 1970-01-01T00:00 maps to zero milliseconds,
  and one day maps to 86 400 000 milliseconds.
- S-T2.1 Tenor parsing and printing round-trip on a hand-written list; date
  plus tenor matches hand-computed dates; day and week tenors are invertible.
- S-T2.2 Stub interpolation bracketing (ISDA 2006 section 8.3): the two
  bracketing standard tenors of a span lie on either side of it; spans equal
  to a standard tenor return that tenor on both sides.

### T3 — Day counts

- S-T3.1 Published examples (published): the worked examples of the ISDA
  1999 ACT/ACT memo for ACT/ACT ISDA, ACT/ACT ICMA and ACT/ACT AFB (regular
  periods, short and long first periods, final stubs); the examples of the
  ISDA 2006 Definitions and of the ICMA rule book for the 30/360 family; the
  30/360 US worked cases of the SIA Standard Securities Calculation Methods.
- S-T3.2 Hand-computed table (hand computation): for each convention, a table
  written in the test covering zero-length periods, single days, February in
  leap and non-leap years, day 31, end of February, year-end crossings and
  multi-year spans, each row with its arithmetic shown.
- S-T3.3 Identities: for ACT/360, ACT/365F and ACT/ACT ISDA the year fraction
  is additive over adjacent sub-periods; for ACT/ACT ISDA a full calendar year
  gives exactly one; for 30/360 Bond Basis and 30E/360 a period from day d of
  one month to day d of the next (d ≤ 28) gives exactly 1/12; for 1/1 any
  non-empty period gives one; ACT/ACT ICMA of a regular period equals one over
  the frequency.

```math
\tau(d_1, d_3) = \tau(d_1, d_2) + \tau(d_2, d_3) \quad \text{for additive conventions}
```

- S-T3.4 BUS/252 (independent): the numerator equals a direct count of
  business days of the attached calendar computed in the test.
- S-T3.5 Serialisation round trip of every convention and every alias
  parsing to its canonical value.

### T4 — Currencies

- S-T4.1 ISO 4217 (published): every code, numeric code and minor-unit count
  matches the current ISO 4217 list maintained by its maintenance agency.
- S-T4.2 Rounding (hand computation): half-up and half-even rounding on
  exact ties, and ISDA section 8.1 rounding examples.
- S-T4.3 Conventions (published): each per-currency convention entry carries
  a source; a test asserts that every entry has a non-empty citation and that
  the stated calendars, day counts and lags parse.

### T5–T6 — Calendars and adjustment

- S-T5.1 Official lists (published): for each built-in centre, the full
  holiday list of several recent years and of the current year matches the
  list published by its official source (for example the ECB for TARGET, the
  UK government bank-holiday list for London, the Federal Reserve holiday
  schedule for New York banking days, the Cabinet Office of Japan for Tokyo,
  the Hong Kong government gazette for Hong Kong).
- S-T5.2 Easter (published): Easter Sunday for a span of years matches a
  published Easter table (for example the tables of the US Naval Observatory
  or Meeus's Astronomical Algorithms).
- S-T5.3 Lunisolar (published): Lunar New Year dates over the supported range
  match the Hong Kong Observatory Gregorian-lunar conversion tables;
  converting lunar to Gregorian and back is the identity, and day counts
  between two lunar dates equal the Gregorian day counts.
- S-T5.4 Weekend observance (hand computation): each observance rule on
  holidays falling on Saturday and on Sunday, including cascading
  substitutes; rule changes on both sides of their change year.
- S-T5.5 Joint calendars (identity): the holiday set of a joint calendar is
  the union of the members' sets; the joint calendar of a centre with itself
  equals the centre; the "any open" combination is the intersection.
- S-T5.6 Adjustment truth tables (definition): for every convention, a
  table written in the test covering a business day, a weekend before a
  month end, a weekend after a month start and a holiday block, with the
  expected result derived from the T6 definitions.
- S-T5.7 Business-day arithmetic (identity): adding n then −n business days
  to a business day returns it; the business-day count between d and
  d + n business days is n; adding zero business days to a holiday returns
  the holiday; adding across a year end on a joint TARGET and London
  calendar agrees with a day-by-day count in the test.

### T7 — Schedules

- S-T7.1 Invariants (property-based): for random effective dates, tenors,
  frequencies, stub types and rolls, the unadjusted and adjusted dates are
  strictly increasing, the first and last dates are the effective and
  termination dates, every regular period has the nominal length, stubs
  appear only at the ends, and no period is empty.
- S-T7.2 Market-standard swap schedules (published): the example schedules of
  the ISDA 2006 Definitions and of public convention guides (for example the
  OpenGamma "Interest Rate Instruments and Market Conventions Guide"), with
  end-of-month rolls, fixing lags over holidays and trade dates on a weekend.
- S-T7.3 Stubs (hand computation): front and back, short and long stubs, and
  explicit first/last regular dates, against schedules written out in the
  test.
- S-T7.4 IMM (definition): every IMM date is a Wednesday between the 15th and
  the 21st; next IMM of an IMM date is the following one; IMM codes round
  trip.
- S-T7.5 CDS dates (published): the step-in, accrual start, cash-settlement
  and standard maturity dates of the worked examples in the ISDA Standard CDS
  Converter documentation and the ISDA "CDS standard model" public
  documentation, including trade dates just before, on and after a roll date
  and around the semi-annual maturity roll.
- S-T7.6 Money-market dates (definition): overnight, tomorrow-next,
  spot-next, week and month deposits from trade dates before weekends,
  before holidays and at month ends, against dates derived by hand from the
  T7.8 rules.
- S-T7.7 Overnight compounding (published): observation-shift and lookback
  date sets of the ISDA 2021 Definitions examples and of the ARRC SOFR
  conventions documentation.
- S-T7.8 Futures (published): last trading days of a few listed contracts
  computed from the published exchange rulebook rule, using the T7.11 rule
  language.

## References

- ISDA, *2006 ISDA Definitions*, in particular the definitions of business
  days, section 4.12 (business-day conventions), section 4.16 (day count
  fractions), and section 8 (rounding, linear interpolation); ISDA, *2021 ISDA Interest Rate Derivatives Definitions*
  (overnight compounding, observation shift, lookback).
- ISDA, "EMU and market conventions: recent developments", 1999 (ACT/ACT
  conventions and worked examples).
- International Capital Market Association (ICMA), *Primary Market Handbook /
  Rule Book*, rule 251 (accrued interest, ACT/ACT ICMA, ACT/365L).
- Securities Industry Association, *Standard Securities Calculation Methods*,
  1993 (30/360 US).
- ISO 8601-1:2019, *Date and time — Representations for information
  interchange*.
- ISO 4217:2015, *Codes for the representation of currencies*, and the
  current code list published by its maintenance agency.
- J. Meeus, *Astronomical Algorithms*, 2nd edition, Willmann-Bell, 1998
  (Gregorian Easter, equinoxes); E. G. Richards, "Calendars", in *Explanatory
  Supplement to the Astronomical Almanac*, 3rd edition, 2013.
- Hong Kong Observatory, Gregorian-Lunar Calendar Conversion Table
  (public); Korea Astronomy and Space Science Institute, lunar calendar
  publications.
- European Central Bank, TARGET2 / T2 closing days (public announcements).
- UK Government, bank holidays list (gov.uk); Federal Reserve Bank holiday
  schedule; Cabinet Office of Japan, national holidays; Hong Kong
  government general holidays gazette; equivalent official publications of
  the other listed centres and exchanges.
- ISDA, *Standard North American Corporate CDS Contract Specification* and
  the ISDA CDS Standard Model public documentation (cdsmodel.com), including
  the 2015 change to semi-annual roll of standard maturities.
- CME Group and ICE Futures Europe rulebooks (IMM dates, last trading day
  rules of interest-rate and commodity futures).
- Alternative Reference Rates Committee, *SOFR conventions* documents;
  Bank of England / Working Group on Sterling Risk-Free Reference Rates,
  SONIA conventions; ECB €STR methodology.
- OpenGamma, *Interest Rate Instruments and Market Conventions Guide*, 2nd
  edition, 2013 (public quantitative research paper).
- M. Henrard, *Interest Rate Modelling in the Multi-curve Framework*,
  Palgrave Macmillan, 2014 (conventions and index definitions).
