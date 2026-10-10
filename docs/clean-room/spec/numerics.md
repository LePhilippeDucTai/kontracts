# Functional specification — numerical foundations

Status: dirty-room functional specification for the `clean-room-core` plan,
unit J01-P2. It states **what** kontract v0.3 must provide in its numerical
layer, never **how** a reference implementation does it. Implementers follow
`docs/clean-room/PROTOCOL.md` and choose their own names, structure and
algorithms among the public methods cited here.

Scope: special functions and probability distributions, descriptive
statistics, dense and banded linear algebra including symmetric
eigen-decomposition, random and low-discrepancy number generation with the
Brownian bridge, root finding, adaptive quadrature, interpolation and
piecewise-constant functions of time, and optimisation (bounded least squares,
global stochastic search, quasi-Newton, quadratic and linear programming).

Global conventions, applying to every capability below:

- All real arithmetic is IEEE-754 double precision (`f64`).
- Every operation that can fail on its input returns a typed error (a variant
  of the crate-wide error type) instead of panicking or returning a silent
  NaN. A NaN or infinite input is an input error unless the capability states
  otherwise.
- Stochastic capabilities are deterministic given their seed, independent of
  the number of worker threads.
- No C, C++ or Fortran code and no vendor maths library: pure Rust only. An
  optional accelerated backend is out of scope.
- Exporting data to external statistics or plotting tools is out of scope.

Mapping onto the plan:

| Group | Capability group | Plan unit |
|-------|------------------|-----------|
| N1 | Special functions and distributions | J02-P1 |
| N2 | Descriptive statistics and risk quantiles | J02-P1 |
| N3 | Linear algebra | J02-P2 |
| N4 | Random numbers, low-discrepancy sequences, Brownian bridge, Poisson sampling | J02-P3 |
| N5 | Root finding | J02-P4 |
| N6 | Quadrature | J02-P4 |
| N7 | Interpolation and piecewise-constant functions of time | J02-P4 |
| N8 | Optimisation and calibration driver | J02-P5 |

## Capabilities

### N1 — Special functions and distributions (J02-P1)

- N1.1 Error function and complementary error function, with full relative
  accuracy of the complementary function in the far tail (no cancellation for
  large positive arguments).
- N1.2 Standard normal density, cumulative distribution function and its
  inverse (quantile function), all at machine precision over the usable
  double range.
- N1.3 A fast, lower-accuracy normal cumulative distribution function for hot
  Monte-Carlo loops, with a documented maximum absolute error taken from the
  published approximation it implements.
- N1.4 Gamma function, log-gamma function, lower and upper regularised
  incomplete gamma functions, and the unregularised lower incomplete gamma
  integral.
- N1.5 Chi-square distribution: central cumulative distribution function;
  non-central cumulative distribution function and density, for the range of
  non-centrality parameters met by square-root diffusions (CIR-type models).
- N1.6 Student t cumulative distribution function (and quantile) for real
  degrees of freedom, through the regularised incomplete beta function.
- N1.7 Poisson distribution: probability mass, cumulative distribution and
  quantile function, where the quantile of level p is the smallest integer k
  with cumulative probability at least p.
- N1.8 Poisson bridge: sample the count of a Poisson process at an
  intermediate time given its counts at two surrounding times.
- N1.9 Bivariate standard normal cumulative distribution function for any
  correlation in the closed interval from minus one to one.
- N1.10 Gaussian quadrature nodes and weights: Gauss-Hermite (weight
  exp(-x²) on the real line), Gauss-Laguerre (weight exp(-x) on the positive
  half-line) and Gauss-Legendre on an arbitrary finite interval, for any node
  count from one up to at least sixty-four, plus a convenience that applies a
  Gauss-Legendre rule to a function on [a, b].
- N1.11 Real roots of quadratic and cubic polynomials with real coefficients,
  degenerate leading coefficients included.
- N1.12 Binomial enumeration: fold a user function over the binomial
  coefficients C(n, k) for k from zero to n, or up to an optional maximum k,
  without overflow for large n (coefficients produced as floating values by a
  stable recurrence).
- N1.13 Integer apportionment: split a non-negative integer total into n
  integer parts, each equal to the floor or the ceiling of total / n, summing
  exactly to the total, with the larger parts spread as evenly as possible
  (error-accumulation scheme of Bresenham's line algorithm).
- N1.14 Simple ordinary least-squares line fit y ≈ a·x + b.
- N1.15 Removable-singularity guard: evaluate a parametric closed form f(p, x)
  that is ill-conditioned or undefined as the parameter p tends to a limit
  (typically zero, e.g. (1 - exp(-p·t)) / p), by switching, below a
  user-given threshold on |p|, to the linear interpolation between the limit
  value supplied by the caller and the value at the threshold. Variants: one
  scalar parameter with scalar output, one scalar parameter with vector
  output, and two parameters with separate thresholds (using the four corner
  limits supplied by the caller).

### N2 — Descriptive statistics and risk quantiles (J02-P1)

- N2.1 Sample moments: mean, variance (population and unbiased), standard
  deviation, skewness, excess kurtosis, covariance and Pearson correlation of
  equal-length samples; minimum, maximum, sum, product and dot product of
  vectors.
- N2.2 Correlation of two dated series: align on common dates, compute
  correlation of log-returns (or of increments, selectable) between
  consecutive common dates, require a minimum number of common points, and
  check that consecutive common dates are regularly spaced; options to
  tolerate quasi-regular spacing and to pair a monthly series with the next
  available observation of a denser series.
- N2.3 Rolling historical volatility: from a dated price history (assumed
  business-day sampled and sorted), produce the history of annualised
  volatility of log-returns over a sliding window of given size, returning the
  most recent requested number of points, or a typed error when the history
  is too short.
- N2.4 Empirical quantiles under the sample-quantile definitions 1, 2, 3, 7
  and 8 of Hyndman and Fan, with definition 8 as the default, optional sample
  weights, and the lower-tail integral of the quantile function up to the
  requested level (from which the expected shortfall is the integral divided
  by the level). A form that pre-sorts the data once and answers many levels
  is required.
- N2.5 Historical value-at-risk and expected shortfall from a profit-and-loss
  sample at a confidence level c: the loss quantile at tail position n·(1 - c)
  by linear interpolation between the two bracketing order statistics, the
  average loss over the tail, the two bracketing samples, and a flag raised
  when the tail position had to be clamped to the first or last sample.

### N3 — Linear algebra (J02-P2)

- N3.1 Dense vectors and matrices (row-major), with element-wise arithmetic,
  scaling, shifting, transcendental maps, reductions (sum, product, min, max,
  standard deviation), dot products, transposition (out-of-place and in-place
  for square matrices), matrix-vector and matrix-matrix products with
  optional transposition of either operand, and rank-3 arrays for grid
  storage.
- N3.2 Sorting helpers: in-place sort of reals, a stable index permutation
  (argsort) suitable for large arrays in parallel, and binary search in a
  sorted sub-range with the usual variants (first index whose value is
  greater or equal, first strictly greater, last lower or equal, last
  strictly lower, exact match).
- N3.3 Cholesky factorisation of a symmetric positive-definite matrix into a
  lower-triangular factor, reporting failure (with the failing pivot index)
  instead of returning garbage on a non-positive-definite input; a repair
  option for positive semi-definite inputs (diagonal jitter or spectral
  clipping, selectable).
- N3.4 LU factorisation with partial pivoting: solve for one or several
  right-hand sides, determinant, inverse; typed error when the matrix is
  numerically singular. A robust solver that never fails: it uses the LU
  solution when the matrix is well conditioned and otherwise the minimum-norm
  least-squares (pseudo-inverse) solution, and tells the caller which branch
  produced the answer.
- N3.5 Symmetric eigen-decomposition: all eigenvalues in increasing order and
  an orthonormal matrix of eigenvectors, using only the lower triangle of the
  input.
- N3.6 Spectral square root of a correlation matrix: given the lower triangle
  of a correlation matrix (unit diagonal implied), return a factor B with
  B·Bᵀ equal to the input when it is positive definite and no condition is
  set, and otherwise close to it with unit diagonal restored by row
  rescaling. Conditions: an eigenvalue floor (eigenvalues below the floor are
  set to zero) or a maximal rank r (only the r largest eigenvalues are kept),
  with an option to keep the full column count instead of reducing B to its
  rank. Also report the rank and the root-mean-square error between the lower
  triangles of the input and of B·Bᵀ.
- N3.7 Nearest correlation matrix in the Frobenius norm by alternating
  projections with Dykstra's correction (Higham 2002), as an alternative to
  spectral clipping.
- N3.8 Inverse and pseudo-inverse of a symmetric positive semi-definite
  matrix through its eigen-decomposition, with an eigenvalue cut-off below
  which eigenvalues are treated as zero.
- N3.9 Principal component analysis of a data matrix (items × features):
  per-feature empirical mean and standard deviation, standardised data,
  eigenvalues (increasing order) and eigenvectors of the correlation matrix,
  and the projection of the data onto its k leading components, returned in
  the original units and shape.
- N3.10 Tridiagonal systems: storage of a square tridiagonal matrix by rows,
  matrix-vector product, direct solve (Thomas algorithm) and solve with the
  transposed matrix, the right-hand side left untouched.
- N3.11 Five-band (pentadiagonal) sparse matrices whose five non-zeros per
  row sit at arbitrary column offsets: row assignment, the product
  y ← α·A·x + β·y, and a direct banded solve.
- N3.12 Preconditioned stabilised bi-conjugate gradient solver (BiCGSTAB)
  for non-symmetric sparse systems, with a Jacobi or incomplete-LU
  preconditioner, tolerance, iteration cap and a convergence report.

### N4 — Random numbers, low-discrepancy sequences and Brownian bridge (J02-P3)

- N4.1 Mersenne Twister MT19937 pseudo-random generator, seedable both from a
  single integer and from an integer array, whose raw 32-bit output stream is
  bit-identical to the authors' published reference implementation.
- N4.2 Uniform deviates in the open interval (0, 1) (never exactly 0 or 1, so
  that the inverse normal is always finite) with at least 53 random bits
  where the generator allows it, and standard Gaussian deviates obtained by
  inverting the normal distribution function (not by rejection or
  Box-Muller, so that the mapping from uniforms to Gaussians is monotone and
  preserves low-discrepancy structure).
- N4.3 Sobol low-discrepancy sequence with the published Joe-Kuo direction
  numbers, in Gray-code order, supporting the full published dimension range;
  random-access to point index i (skip-ahead) for parallel chunking; options
  to skip leading points, to apply a seeded digital (random XOR) shift or an
  Owen-type scrambling, and to apply a seeded randomisation of the free
  initial direction-number bits in the spirit of Jäckel's regularity-breaking
  initialisation. Above the largest supported dimension, the remaining
  coordinates are filled from the pseudo-random generator (hybrid sequence).
- N4.4 Block generation: fill a paths × dimensions matrix of uniforms or
  Gaussians from a selected generator kind, in successive calls that continue
  the stream; internal buffering must not change the values produced.
- N4.5 Reproducible parallel streams: a stream is addressed by a seed and a
  substream (or chunk) index, so that a rayon-parallel simulation produces the
  same numbers for any thread count and any chunk scheduling.
- N4.6 Correlated Gaussian vectors from a correlation or covariance matrix
  through its Cholesky factor or spectral square root (N3.3, N3.6).
- N4.7 Brownian bridge construction on an arbitrary increasing time grid
  t₁ < … < tₙ (t₀ = 0 implied): the terminal point is drawn first, then the
  grid is filled by successive conditional (bridge) samples in a bisection
  order, so that the first input deviates carry the largest share of the
  path variance. Two maps are required, each acting row-wise or column-wise
  on a matrix of inputs: (a) deviates to Wiener path values W(tᵢ); (b) the
  increments W(tᵢ) - W(tᵢ₋₁) that the bridge path implies, i.e. the
  N(0, tᵢ - tᵢ₋₁) deviates an Euler scheme would need to reproduce the same
  path.
- N4.8 Poisson process sampling on a time grid: jump counts per interval and
  jump times (uniform order statistics given the count), from either uniform
  inputs (inverse distribution function) or the generator directly, with
  the Poisson bridge (N1.8) for refinement.

### N5 — Root finding (J02-P4)

- N5.1 Brent's bracketed method on [x_lo, x_hi] with an absolute tolerance on
  the root and an iteration cap; the error distinguishes "function below zero
  at both ends" from "function above zero at both ends"; an optional
  protected mode that tries to widen the bracket a bounded number of times
  before failing.
- N5.2 Secant method from a starting point, returning "no solution" (not an
  error) when it does not converge within the iteration cap.
- N5.3 Safeguarded Newton-Raphson combining Newton steps with bisection inside
  a bracket, with a user-supplied derivative.
- N5.4 Multidimensional Newton for square nonlinear systems with an analytic
  or finite-difference Jacobian and a backtracking line search.
- N5.5 Batched bracketed root finding: solve many independent bracketed
  problems at once when the expensive part is evaluating the functions
  (for example one pricing call evaluates every pending point). At each round
  the solver hands the caller the list of (problem id, abscissa) pairs to
  evaluate and receives, per pair, either a value with an opaque payload or
  "cannot evaluate"; each problem ends with its root, function value and
  payload, or with its own error (not bracketed below, not bracketed above,
  cannot evaluate at a given abscissa). An input with x_hi ≤ x_lo is an input
  error.

### N6 — Quadrature (J02-P4)

- N6.1 Gauss-Kronrod rule pairs with 15, 21, 31, 41, 51 and 61 Kronrod points
  on a finite interval, each returning the integral estimate, an absolute
  error estimate, the integral of |f| and the integral of |f - mean of f|
  (the two auxiliary quantities used by the QUADPACK error heuristics).
- N6.2 Globally adaptive integration of a smooth function on a finite
  interval with a selectable rule, a maximum number of subintervals, an
  absolute and a relative tolerance.
- N6.3 Adaptive integration with extrapolation (Wynn epsilon algorithm) for
  integrands with integrable end-point singularities, using the 21-point rule.
- N6.4 Integration over (-∞, b), (a, +∞) and (-∞, +∞) by mapping to the unit
  interval and applying the extrapolating adaptive scheme with the 15-point
  rule.
- N6.5 Every adaptive integrator returns either success with the value, the
  absolute error estimate and the number of function evaluations, or a
  typed failure: subdivision limit reached, round-off detected, bad
  integrand behaviour, divergence or slow convergence, invalid input
  (tolerances unattainable or non-positive limit).

### N7 — Interpolation and piecewise-constant functions of time (J02-P4)

- N7.1 One-dimensional interpolators on strictly increasing abscissas:
  piecewise linear, linear on the logarithm of the ordinates (log-linear),
  natural and clamped cubic splines, and a monotone-preserving cubic
  (Fritsch-Carlson, with the Hyman filter as an option). Left and right
  extrapolation are chosen independently between flat (constant end value)
  and extension of the end piece.
- N7.2 Local least-squares quadratic interpolation: on each interval
  [xᵢ, xᵢ₊₁] a quadratic that passes through both end nodes and minimises the
  squared misfit at the two neighbouring nodes xᵢ₋₁ and xᵢ₊₂; at the two ends
  missing neighbours are replaced by ghost nodes obtained by linear
  extrapolation. A variant evaluates directly on a grid given as two arrays
  (domain and image) for use inside PDE solvers.
- N7.3 Three-point (quadratic Lagrange) interpolation and one-sided
  extrapolation on non-uniform nodes, used for boundary values and
  derivatives on PDE grids.
- N7.4 Two-dimensional interpolation on a rectangular grid: bilinear and
  bicubic (or bicubic spline), with the same flat-extrapolation choices per
  axis.
- N7.5 Adaptive piecewise-linear knot placement: given a function f on
  [min, max] and an initial step, return knots such that the piecewise-linear
  interpolant of f at the knots stays within a tolerance of f, together with
  a support point separating the put-like and call-like parts for static
  replication of a payoff by vanilla options (Carr-Madan decomposition).
- N7.6 Piecewise-constant functions of time (used for volatilities,
  continuous dividend yields, repo rates and hazard rates): defined by a
  first value and a list of (date, value) steps in increasing date order; a
  list form where the pair (tᵢ, vᵢ) means "value vᵢ on (tᵢ₋₁, tᵢ]" with flat
  extension after the last date; evaluation at a date; point-wise sum,
  difference, product and mapping by a scalar function; exact integral and
  integral of the square over [d₁, d₂]; integral of the function weighted by
  the derivative of a caller-given primitive; the discount-like quantity
  exp(-∫ f) over [d₁, d₂]; extraction of the constant when the function is
  constant (typed error otherwise); round-trip between the step form and the
  list form.

### N8 — Optimisation and calibration driver (J02-P5)

- N8.1 Bounded Levenberg-Marquardt non-linear least squares in the MINPACK
  tradition (Moré 1978, with the bound handling described by Markwardt
  2009): residuals computed by a caller function from the
  parameter vector, an analytic Jacobian or forward/central finite
  differences, each parameter fixed or free with optional lower and upper
  bounds, residuals relative to target values (default) or absolute, and
  tolerances on the sum of squares, on the parameters and on the
  gradient-orthogonality, plus caps on iterations and function evaluations.
  The outcome reports one of: converged on sum of squares, on parameters, on
  both, on orthogonality, iteration cap reached, or one of the three
  tolerances too small for further progress; or a typed error: invalid input,
  NaN returned by the function, function failure, fewer residuals than free
  parameters, no free parameter, initial value outside its bounds,
  inconsistent bounds, invalid settings.
- N8.2 Global stochastic search on a box: differential evolution after Storn
  and Price, whose mutant mixes the "rand/1/bin" and "best/1/bin" strategies
  with a mixing weight redrawn at every generation (population size and
  crossover rate configurable, sensible defaults per dimension); and particle
  swarm optimisation with inertia, cognitive and social coefficients and the
  regrouping strategy of Evers and Ben Ghalia against stagnation. Both
  respect the box, accept a progress callback (iteration, best value), stop
  on the first of: iteration cap, evaluation cap, target value reached, and
  return the best point, its value, the evaluation count and the stopping
  reason. Deterministic under a fixed seed.
- N8.3 Calibration driver: fit model parameters to market quotes by a global
  stage (N8.2) followed by a local stage (N8.1). Each named parameter is
  either fixed at a value or optimised within a range with an initial value;
  the pricer fills a vector of model values for a parameter vector and may
  signal that a point is infeasible (the driver then treats the point as
  rejected, not as an error). Distances are relative by default and absolute
  on request; a feedback hook receives progress. The result carries the
  fitted parameters (in the caller's own parameter type), the root-mean-square
  error, the quotes and the fitted model values.
- N8.4 Unconstrained quasi-Newton minimisation by limited-memory BFGS with a
  line search satisfying the Wolfe conditions, from a function and its
  gradient; bound-constrained L-BFGS-B, Nelder-Mead and CMA-ES behind the same
  small interface (plan unit J02-P5).
- N8.5 Convex quadratic programming: minimise ½·xᵀCx + dᵀx with C symmetric
  positive definite, subject to a given number of linear equality
  constraints, linear inequality constraints of the form aⱼ·x + bⱼ ≥ 0, and
  lower and upper bounds on x. Return the optimum and the Lagrange
  multipliers (constraints first, then bounds), or a typed error: iteration
  cap, insufficient accuracy, inconsistent constraints (with the index of the
  offending constraint), other failure.
- N8.6 Constrained linear least squares built on N8.5: given samples
  (xₙ, vₙ) and basis functions f₁ … f_K, find coefficients a minimising
  Σₙ (vₙ - Σₖ aₖ·fₖ(xₙ))² subject to bounds on a and optional linear equality
  (E·a + e = 0) and inequality (G·a + g ≥ 0) constraints, with an optional
  ridge penalty Σₖ λₖ·(aₖ - αₖ)² towards a prior; return the coefficients,
  the residuals and the fitted function.
- N8.7 Linear programming: maximise or minimise a linear objective under
  linear equality and "greater than or equal to zero" constraints written as
  sparse (coefficient, variable index) lists in which index zero denotes the
  constant term; variables are non-negative unless an explicit lower bound is
  given; optional upper bounds; outcomes: optimal (objective value, variable
  values, constraint values), infeasible, unbounded, degenerate, numerical
  failure. Also: reading a problem from the standard MPS text format, and a
  check telling whether swapping two given variables leaves the problem
  invariant (used to exploit symmetric problems).

## Behaviour

### N1 — Special functions and distributions

- Accuracy targets (relative error, unless stated): erf and erfc, normal
  density and cumulative function, gamma and log-gamma within a small
  multiple of machine epsilon over their usable range; the normal quantile
  within a small multiple of machine epsilon in p over (0, 1), including the
  tails down to the smallest normal double; incomplete gamma, chi-square,
  Student t and the bivariate normal at least to 1e-12 absolute; the
  non-central chi-square to at least 1e-10 absolute on the supported range.
  These targets are kontract's own design choices.
- The normal cumulative function is computed through erfc for negative
  arguments so that tail probabilities keep full relative accuracy; it
  returns exactly 0 only where the true value underflows and exactly 1 only
  where 1 - Φ(x) is below half an ulp of 1.
- The inverse normal maps 0 to -∞ and 1 to +∞; arguments outside [0, 1] or
  NaN are input errors.
- Gamma is defined for all real arguments except non-positive integers (pole:
  input error); log-gamma returns ln |Γ(x)|.
- Regularised incomplete gamma P(s, x) and Q(s, x) = 1 - P(s, x) are computed
  by the series for x < s + 1 and by the continued fraction otherwise, each
  evaluated directly (never as one minus the other in the cancelling region).
- The non-central chi-square distribution with k degrees of freedom and
  non-centrality λ is the Poisson(λ/2)-weighted mixture of central
  chi-square distributions with k + 2j degrees of freedom; the
  implementation sums the mixture from its mode outward until the remaining
  Poisson mass is below tolerance, and its density uses the modified Bessel
  function representation. A non-centrality beyond the supported range is a
  typed error, not a silently inaccurate value.
- The Student t with ν degrees of freedom is evaluated through the
  regularised incomplete beta function, with ν real and positive.
- The Poisson quantile is the smallest k with F(k) ≥ p; p = 0 returns 0, p = 1
  is an input error (infinite quantile), λ = 0 returns 0 for every p < 1.
- Poisson bridge: given N(t₀) = a and N(t₁) = b with a ≤ b and
  t₀ ≤ t ≤ t₁, N(t) - a is binomial with b - a trials and success probability
  (t - t₀)/(t₁ - t₀); a uniform input selects the sample by inversion.
- Bivariate normal: an algorithm of the Drezner-Wesolowsky / Genz family
  (Gauss-Legendre integration of the correlation integral, with the
  transformation recommended by Genz for |ρ| close to one), or an equivalent
  meeting the accuracy target; ρ = ±1 are handled exactly.
- Gaussian quadrature nodes are returned in increasing order with positive
  weights; Gauss-Laguerre and Gauss-Hermite weights are offered both raw and
  multiplied by the inverse weight function (so that the rule applies to f
  directly), the choice being explicit in the API.
- Cubic roots: the result lists the distinct real roots in increasing order;
  a zero leading coefficient degrades gracefully to the quadratic, linear or
  constant case (an identically zero polynomial is an input error).
- Removable-singularity guard: for |p| ≥ threshold the closed form is
  returned unchanged; for |p| < threshold the value is the linear
  interpolation in p between the caller's limit value at p = 0 and the closed
  form at p = ±threshold (sign of p), so the result is continuous at the
  threshold and exact at zero.

### N2 — Descriptive statistics and risk quantiles

- Variance uses a numerically stable one-pass or two-pass algorithm (Welford
  or compensated two-pass). Empty inputs are input errors; a single point has
  zero population variance and an undefined (error) unbiased variance.
- Dated-series correlation uses only dates present in both series, assumes
  (and documents) increasing date order, and fails with a typed error when
  fewer than the required minimum of common points remain or when the spacing
  of common dates is irregular beyond the allowed tolerance.
- Rolling volatility: the log-return series is r_j = ln(S_j / S_{j-1}); each
  output point is the sample standard deviation of the last w returns times
  the square root of an annualisation factor (observations per year, a
  parameter whose default corresponds to business-day sampling), dated at
  the window's last observation.
- Hyndman-Fan definitions follow their 1996 paper exactly, including their
  boundary rules; with weights, the empirical distribution function is the
  normalised cumulative weight.
- VaR is reported as a positive loss number (the negative of the interpolated
  profit-and-loss quantile); the clamping flag is raised when n·(1 - c) falls
  below one or reaches n.

### N3 — Linear algebra

- Matrices are dense, row-major, `f64`; dimension mismatches are input
  errors, detected before any computation.
- Cholesky returns the lower factor L with L·Lᵀ = A; positive definiteness is
  tested on the pivots, and the error carries the first failing index.
- LU with partial pivoting satisfies P·A = L·U; the determinant is the signed
  product of U's diagonal; "numerically singular" means a pivot below a
  relative threshold of the matrix norm (threshold documented).
- Symmetric eigen-decomposition (Jacobi rotations, or Householder
  tridiagonalisation followed by implicit QL) returns eigenvalues sorted
  increasingly and eigenvectors as orthonormal columns with a deterministic
  sign convention (for example, largest-magnitude component positive).
- The spectral square root (Jäckel 2002, chapter 6) clips eigenvalues below
  the floor (or beyond the rank) to zero, forms B = V·√Λ, then rescales each
  row of B to unit Euclidean norm so that B·Bᵀ has unit diagonal; rows of
  zero norm are an error.
- The robust linear solver never returns an error for finite input; the
  pseudo-inverse branch uses the singular value or symmetric
  eigen-decomposition with a documented cut-off.
- Tridiagonal and five-band solvers run in O(n) and O(n·bandwidth²)
  respectively and do not modify the right-hand side.
- BiCGSTAB stops when the residual 2-norm relative to the right-hand side's
  norm is below tolerance, and reports the iterations used, the final
  residual and whether it converged; breakdown (a vanishing inner product)
  is reported, not hidden.

### N4 — Random numbers, low-discrepancy sequences and Brownian bridge

- MT19937 follows Matsumoto and Nishimura (1998) and the authors' 2002
  initialisation by array; the 32-bit output, the [0, 1) and the (0, 1)
  real conversions match the authors' reference program for the same seed.
- Gaussian deviates are Φ⁻¹(u) for the uniform u, using N1.2.
- Sobol: point i of dimension d is the XOR of the direction numbers selected
  by the bits of the Gray code of i; the first coordinate uses the van der
  Corput base-2 sequence; the published Joe-Kuo primitive polynomials and
  initial direction numbers are the source data (shipped as a data file with
  its licence notice, not retyped by hand). Skip-ahead to index i must give
  the same point as sequential generation.
- A point of a low-discrepancy sequence is one path: in a paths × dimensions
  block each row is one multi-dimensional point, and the column index is the
  dimension, which callers map to (time step, risk factor) pairs; the Brownian
  bridge (N4.7) is the recommended way to assign the best dimensions to the
  largest variance components.
- Bridge construction: with a deviate vector z (Gaussian, or uniform when the
  caller asks the bridge to apply Φ⁻¹), W(tₙ) = √tₙ·z₁, and each later point
  is W(t_m) = w_l·W(t_l) + w_r·W(t_r) + σ_m·z_k with the standard conditional
  Gaussian weights, l and r being the nearest already-built neighbours. The
  construction order and the weights depend only on the grid and are
  precomputed once.
- Parallel streams: the value of a given (seed, stream, index) triple is
  fixed; work splitting across threads must address streams or Sobol indices
  explicitly, never share a mutable generator.

### N5 — Root finding

- Brent requires x_lo < x_hi (input error otherwise) and f(x_lo)·f(x_hi) ≤ 0;
  a root exactly at an end point is returned immediately; the absolute
  tolerance applies to the abscissa.
- Secant uses a small relative perturbation of the start point as its second
  point and returns "no solution" on non-convergence or on a zero
  denominator.
- Batched root finding is equivalent, problem by problem, to running Brent
  independently; batching changes only how evaluations are grouped, never
  the result.

### N6 — Quadrature

- The Gauss-Kronrod nodes and weights are those published with QUADPACK
  (Piessens et al. 1983), shipped as data with their provenance.
- The error estimate of a single rule follows the QUADPACK formula combining
  the Gauss-Kronrod difference and the two auxiliary integrals; the adaptive
  schemes bisect the subinterval with the largest error estimate until the
  global estimate satisfies max(epsabs, epsrel·|I|).
- The infinite-interval mapping is x = a + (1 - t)/t on (0, 1] (and the
  symmetric form for the doubly infinite case).

### N7 — Interpolation and piecewise-constant functions of time

- Every interpolator validates that abscissas are finite and strictly
  increasing and that log-linear ordinates are positive; fewer than two nodes
  is an input error, except for constant interpolation which accepts one.
- Interpolators reproduce the node values exactly (up to rounding).
- Flat extrapolation returns the first (respectively last) node value for
  every abscissa beyond the left (respectively right) node.
- The monotone cubic preserves monotonicity of monotone data on every
  interval and has a continuous first derivative.
- Piecewise-constant functions of time: a value attached to the date tᵢ in
  the list form holds on the left-open, right-closed interval (tᵢ₋₁, tᵢ];
  evaluation at exactly tᵢ returns vᵢ; time is measured in years with the
  ACT/365 fixed convention of time.md unless the caller supplies another
  year-fraction function; integration is exact (sum over steps of value ×
  length).

### N8 — Optimisation and calibration driver

- All optimisers minimise; maximisation is the caller's responsibility
  (negated objective) except in linear programming, which accepts both senses.
- Parameter vectors passed to callbacks always respect the bounds (bound
  handling by projection or by a smooth change of variables, documented).
- Stochastic optimisers draw every random number from an explicitly seeded
  N4 stream; two runs with the same seed, inputs and thread count produce
  identical results, and results must not depend on the thread count when
  population evaluation is parallelised.
- The calibration driver skips the global stage when its iteration budget is
  zero and then starts the local stage from the caller's initial values.
- The quadratic programme requires C positive definite (Goldfarb-Idnani dual
  active-set method or an equivalent with the same contract); at the optimum
  the multipliers of inequalities and bounds are non-negative.
- Linear programming uses a simplex method (primal, dual or criss-cross,
  selectable, with a deterministic anti-cycling rule).

## Edge cases

- Normal functions at ±0, at the largest finite doubles, at ±∞ (Φ(-∞) = 0,
  Φ(+∞) = 1, φ(±∞) = 0) and at NaN (input error).
- Inverse normal at p = 0, p = 1, at the smallest positive subnormal, at
  p = 1 - ulp, and for p just around the central/tail switch points of the
  rational approximation (continuity across the switch).
- Gamma near poles (non-positive integers), at large arguments (overflow
  reported as +∞ for gamma, finite for log-gamma), and at half-integers.
- Incomplete gamma with x = 0, with x very large, and with s at the
  series/continued-fraction switch.
- Non-central chi-square with λ = 0 (must equal the central distribution),
  at the upper end of the supported λ range, and its density near x = 0 with
  fewer than two degrees of freedom (the density is unbounded there and must
  return +∞ or a large finite value, never NaN).
- Student t with ν = 1 and ν = 2 (closed forms) and very large ν.
- Bivariate normal with ρ = -1, 0 and 1 and with infinite arguments.
- Poisson quantile with λ = 0, with p at the exact cumulative values, and
  with very large λ.
- Cubic with a triple root, a double root, a zero leading coefficient, and
  all-zero coefficients.
- Binomial fold with n = 0, with maximum k above n, with a negative maximum
  k (the fold still visits k = 0 once), and with n large enough that integer
  coefficients would overflow.
- Apportionment with total = 0, with n = 1, with total < n, and with n = 0
  (input error).
- Statistics on an empty sample, a constant sample (zero variance, undefined
  correlation reported as an error), a single point, and NaN entries.
- Dated correlation with no common dates, with duplicated dates, and with a
  missing month in a monthly series.
- Quantiles at levels 0 and 1, with a sample of size 1, with ties, and with
  all weights zero (input error).
- Cholesky of a semi-definite and of an indefinite matrix; LU of a singular,
  of a nearly singular (Hilbert) and of a 1 × 1 matrix; determinant of a
  matrix containing NaN (propagates NaN or errors, documented).
- Eigen-decomposition with repeated eigenvalues (orthonormality must still
  hold), of the zero matrix, and of an input whose strict upper triangle holds
  NaN (only the lower triangle may be read).
- Reductions of empty vectors: the sum is 0 and the product is 1.
- Spectral square root of a non-positive-semi-definite "correlation" matrix
  (typical after independent pairwise estimation) and with a rank cap of one.
- Tridiagonal systems of size 1 and 2; zero pivot without pivoting (error);
  storage whose two unused corner cells hold NaN (they must never be read);
  repeated solves with the same factorisation leave the inputs unchanged.
- BiCGSTAB with a zero right-hand side (returns zero immediately) and a
  singular operator (reports non-convergence).
- Sobol with dimension 1, at the largest supported dimension, beyond it
  (hybrid fill), and the point of index zero (all zeros: skipped or mapped
  away so that Φ⁻¹ stays finite).
- MT19937 seeded with zero and with an array of length one.
- Brownian bridge on a single date, on non-uniform grids whose step sizes
  span many orders of magnitude, on grids containing repeated dates (a
  zero-length step has a zero increment, never NaN), and on a grid that does
  not start at zero (the first increment then starts at the grid's origin,
  documented); row-wise and column-wise layouts give bit-identical paths.
- Brent with a root exactly at an end point, with a discontinuous sign change
  (must converge to the jump), with a bracket that also contains an even
  (double) root without sign change, with a flat triple root such as x³ at
  zero, and with an unbracketed interval on each side.
- Quadrature of a constant, of an odd function on a symmetric interval, of a
  function with an end-point singularity, of a divergent integral (must report
  divergence or round-off rather than a finite "success"), with limit = 1, and
  with both tolerances zero (invalid input).
- Interpolation exactly at nodes, outside the range on both sides, with two
  nodes only, and with duplicated abscissas (input error).
- Piecewise-constant function with no steps (constant), with a step exactly
  at the integration bounds, with d₂ < d₁ (integral changes sign) and with
  d₁ = d₂ (zero).
- Optimisers with zero free parameters, with lower bound equal to upper
  bound, with an initial value outside the bounds, with an objective
  returning NaN, and with a pricer that marks every point infeasible.
- Quadratic programme with inconsistent equality constraints, with redundant
  constraints, and with a bound-active optimum.
- Linear programme that is infeasible, unbounded, degenerate, or has no
  constraints.

## Test scenarios

Each scenario names its independent justification. "Identity" means the
expected value follows from the stated mathematical relation; "published"
means it comes from the cited public reference; "independent" means a second
method computed inside the test. No scenario uses data produced by the
reference library.

### N1 — Special functions and distributions (tests/math_special.rs)

- S-N1.1 Symmetry and complement (identity), over a dense grid and in the
  tails:

```math
\Phi(x) + \Phi(-x) = 1, \qquad \operatorname{erf}(x) = 2\,\Phi(x\sqrt{2}) - 1, \qquad \operatorname{erfc}(x) = 2\,\Phi(-x\sqrt{2})
```

- S-N1.2 Round trip (identity): Φ(Φ⁻¹(p)) = p to a few ulps over p on a
  log-spaced grid from the smallest normal double to 1 - ulp, and
  Φ⁻¹(Φ(x)) = x for |x| up to the tail limit.
- S-N1.3 Density is the derivative of the distribution (identity): a
  high-order central finite difference of Φ matches φ; φ integrates to one by
  adaptive quadrature (N6).
- S-N1.4 Tabulated values (published): erf, erfc, Φ, Γ and the incomplete
  gamma at the arguments tabulated in Abramowitz and Stegun (chapters 6, 7
  and 26) and in the NIST DLMF, to the precision of the table.
- S-N1.5 Fast normal function (published): its maximum absolute deviation
  from the accurate Φ over a dense grid is below the error bound published
  for the approximation it implements.
- S-N1.6 Gamma identities: Γ(n + 1) = n! for integers, Γ(1/2) = √π,
  Γ(x + 1) = x·Γ(x), and the reflection formula Γ(x)·Γ(1 - x) = π / sin(πx);
  ln Γ agrees with ln |Γ| where both are finite.
- S-N1.7 Incomplete gamma identities: P(s, x) + Q(s, x) = 1; P(1, x) =
  1 - exp(-x); P(1/2, x) = erf(√x).
- S-N1.8 Chi-square (identity): with two degrees of freedom the central
  distribution function is 1 - exp(-x/2); the non-central distribution with
  λ = 0 equals the central one; the non-central density integrates to one and
  its mean is k + λ (quadrature, N6); the non-central distribution function is
  increasing in x and decreasing in λ.
- S-N1.9 Student t (identity): ν = 1 gives the Cauchy distribution
  F(t) = 1/2 + arctan(t)/π; ν = 2 gives F(t) = 1/2 + t / (2·√(t² + 2)); for
  large ν the distribution approaches Φ; F(-t) = 1 - F(t).
- S-N1.10 Poisson quantile (definition): for every tested (λ, p) with
  0 < p < 1, F(q - 1) < p ≤ F(q), where F is computed by direct summation of
  the mass function in the test.
- S-N1.11 Poisson bridge (identity): over many uniform inputs, the empirical
  distribution of the intermediate count matches the binomial law with
  b - a trials and probability (t - t₀)/(t₁ - t₀) (chi-square goodness of fit
  at a stated significance); t = t₀ returns a and t = t₁ returns b.
- S-N1.12 Bivariate normal (identity): ρ = 0 gives Φ(x)·Φ(y); ρ = 1 gives
  Φ(min(x, y)); ρ = -1 gives max(0, Φ(x) + Φ(y) - 1); at the origin
  Sheppard's formula 1/4 + arcsin(ρ)/(2π) holds; symmetry in (x, y); a
  two-dimensional quadrature in the test (independent) agrees on a grid;
  published values in Genz (2004).
- S-N1.13 Gaussian quadrature exactness (identity): for n nodes the rules
  integrate polynomials of degree up to 2n - 1 exactly:

```math
\int_{0}^{\infty} x^{k} e^{-x}\,dx = k!, \qquad \int_{-\infty}^{\infty} x^{2k} e^{-x^2}\,dx = \Gamma\!\left(k + \tfrac{1}{2}\right), \qquad \int_{a}^{b} x^{k}\,dx = \frac{b^{k+1} - a^{k+1}}{k+1}
```

  and the weights sum to √π (Hermite), 1 (Laguerre) and b - a (Legendre).
- S-N1.14 Cubic roots (identity): each returned root makes the polynomial
  vanish to a relative tolerance; Vieta's relations hold when three real
  roots exist; polynomials built from chosen roots (simple, double, triple)
  return those roots.
- S-N1.15 Binomial fold (identity): the sum over k of C(n, k) equals 2ⁿ, the
  alternating sum equals 0 for n ≥ 1, and C(n, k) = C(n, n - k).
- S-N1.16 Apportionment (definition): parts sum to the total, each part is
  the floor or the ceiling of total / n, and every partial sum of the first j
  parts stays within one of j·total / n.
- S-N1.17 Least-squares line (identity): exact recovery of (a, b) on
  noise-free linear data; the residuals are orthogonal to the regressors.
- S-N1.18 Removable-singularity guard (identity): for f(p, t) =
  (1 - exp(-p·t)) / p with limit t at p = 0, the guarded value is
  continuous across the threshold, equals the limit at p = 0 and equals the
  closed form above the threshold; the same for the two-parameter variant.

### N2 — Descriptive statistics (tests/math_special.rs)

- S-N2.1 Moments (hand computation documented in the test) on small integer
  samples; invariance of variance and correlation under shifts, and scaling
  laws under multiplication.
- S-N2.2 Dated correlation (identity): a series with itself gives 1; with its
  reciprocal (log-returns negated) gives -1; an affine transform of the
  log-returns does not change it; fewer common points than the minimum gives
  the typed error.
- S-N2.3 Rolling volatility (identity): a price series with constant
  log-return gives zero volatility; a series whose log-returns alternate
  between +a and -a gives a closed-form sample standard deviation times the
  square root of the annualisation factor; the output length equals the
  requested result size.
- S-N2.4 Quantiles (published): results of each of the five definitions on
  the illustrative samples discussed by Hyndman and Fan (1996); monotonicity
  in the level; definition 1 returns sample points only.
- S-N2.5 VaR and expected shortfall (hand computation): on the sample
  {1, 2, …, n} the quantile and tail mean follow from the interpolation rule
  in closed form; the clamping flag is raised exactly in the documented
  boundary cases.

### N3 — Linear algebra (tests/math_linalg.rs)

- S-N3.1 Reconstruction identities on random well-conditioned matrices
  (property-based, seeded): L·Lᵀ = A for Cholesky; P·A = L·U for LU;
  A·V = V·Λ and Vᵀ·V = I for the eigen-decomposition; A·A⁻¹ = I.
- S-N3.2 Determinant (independent): LU determinant equals the Leibniz
  expansion for 2 × 2 and 3 × 3 matrices and the closed-form Vandermonde
  determinant for larger ones.
- S-N3.3 Known spectra (published): the symmetric tridiagonal Toeplitz matrix
  with diagonal a and off-diagonal b has eigenvalues a + 2b·cos(kπ/(n + 1)),
  k = 1 … n; a rank-one matrix u·uᵀ has a single non-zero eigenvalue |u|².
- S-N3.4 Conditioning (published): LU on Hilbert matrices of increasing size
  either solves with an error consistent with the known condition number or
  reports singularity; the robust solver always returns and flags the
  pseudo-inverse branch on singular input; the pseudo-inverse satisfies the
  four Moore-Penrose conditions.
- S-N3.5 Correlation square root (identity): for a positive-definite
  correlation matrix B·Bᵀ reproduces it; for an indefinite input the output
  has unit diagonal and is positive semi-definite, and the reported error
  matches the error recomputed in the test; with a rank cap r the product
  has rank r.
- S-N3.6 Nearest correlation (published): the worked examples in Higham
  (2002) converge to a matrix with unit diagonal, non-negative spectrum, and a
  Frobenius distance consistent with the paper.
- S-N3.7 PCA (identity and independent): eigenvalues of the correlation
  matrix sum to the number of features; projecting on all components
  reproduces the data; projecting on k components gives the best rank-k
  approximation of the standardised data (Eckart-Young), verified against a
  direct eigen computation; on Fisher's public Iris data set (log-transformed)
  the component variances agree with an independent eigen computation in the
  test.
- S-N3.8 Tridiagonal and five-band solvers (independent): solutions agree
  with dense LU on random diagonally dominant systems; the discrete second
  difference matrix with a right-hand side built from a chosen solution
  returns that solution; transposed solve agrees with solving the explicit
  transpose.
- S-N3.9 BiCGSTAB (independent): agrees with the dense solve on random
  non-symmetric diagonally dominant systems; convergence report consistent
  with the residual recomputed in the test.
- S-N3.10 Sorting and search (property-based): argsort yields a permutation
  that sorts; each binary-search variant agrees with a linear scan, including
  on arrays with repeated values and on empty sub-ranges.

### N4 — Random numbers and Brownian bridge (tests/math_random.rs)

- S-N4.1 MT19937 (published): the first outputs for the authors' documented
  array seed match the reference output file distributed with the authors'
  implementation (both the integer and the real outputs).
- S-N4.2 Sobol (published and identity): the first points of the low
  dimensions match those tabulated by Bratley and Fox (1988) and Joe and Kuo
  (2008); for dimension one the first 2ᵐ points are a permutation of
  {k / 2ᵐ}; the first 2ᵐ points of each low-dimensional pair form a
  (t, m, 2)-net with the t-value implied by the direction numbers;
  skip-ahead equals sequential generation.
- S-N4.3 Uniformity (statistical): Kolmogorov-Smirnov statistics for large
  samples of uniforms and of Φ applied to the Gaussians are below the
  critical value at a stated significance; sample mean and variance of the
  Gaussians are within the stated multiple of their standard errors.
- S-N4.4 Reproducibility (identity): a seeded simulation run with 1, 2 and 8
  threads produces bit-identical blocks; buffering choices do not change the
  stream.
- S-N4.5 Correlated vectors (statistical): the sample correlation of
  generated vectors matches the target matrix within the stated standard
  errors.
- S-N4.6 Brownian bridge covariance (identity): the bridge map M from
  deviates to path values satisfies M·Mᵀ = [min(tᵢ, tⱼ)] exactly (up to
  rounding), on uniform and non-uniform grids; the terminal value depends
  only on the first deviate.
- S-N4.7 Bridge increments (identity): the increments map applied to a
  bridge path, cumulated, gives back the path; the increments are
  independent N(0, tᵢ - tᵢ₋₁) (statistical check).
- S-N4.8 Poisson sampling (statistical and identity): the first four raw
  moments of the counts, for several intensities and with both pseudo-random
  and Sobol inputs, match the Poisson closed forms within standard errors;
  given the count, jump times are uniform (KS test).

### N5–N7 — Roots, quadrature, interpolation (tests/math_solvers.rs)

- S-N5.1 Roots (identity): √2 from x² - 2, the Dottie number from cos x - x,
  Lambert-W values from x·eˣ - c checked by substitution; Brent's own test
  functions from Brent (1973) converge within the iteration cap.
- S-N5.2 Errors (definition): an interval where f is positive at both ends
  yields the "above" error, negative at both ends the "below" error; the
  secant method on a function without a real root returns "no solution".
- S-N5.3 Batched roots (independent): results equal those of independent
  Brent runs, with "cannot evaluate" reported for the affected problems only.
- S-N5.4 Multidimensional Newton (identity): solves a small polynomial system
  whose roots are chosen in the test, with quadratic convergence observed on
  the residual norms.
- S-N6.1 Rule exactness (identity): each Gauss-Kronrod pair integrates
  polynomials up to its Kronrod degree exactly; the returned error estimate
  is not smaller than the true error on a set of smooth test integrands.
- S-N6.2 Adaptive integration (published): the QUADPACK book's test problems
  (Piessens et al. 1983), and integrals with closed forms such as

```math
\int_0^1 \frac{dx}{\sqrt{x}} = 2, \qquad \int_0^1 \ln x\,dx = -1, \qquad \int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}, \qquad \int_0^{\infty} \frac{dx}{1+x^2} = \frac{\pi}{2}
```

  agree within the returned error estimate, and the true error is below the
  requested tolerance.
- S-N6.3 Failure modes (definition): the integral of 1/x on (0, 1] reports a
  failure status; a subdivision limit of one on an oscillatory integrand
  reports the limit status.
- S-N7.1 Interpolation exactness (identity): linear interpolation reproduces
  affine data; the clamped cubic spline with exact end slopes reproduces any
  cubic; the local least-squares quadratic reproduces any quadratic; all
  interpolators return node values at nodes.
- S-N7.2 Monotonicity (published): on the monotone data sets of Fritsch and
  Carlson (1980) the monotone cubic is monotone on a dense evaluation grid,
  while the natural cubic spline is shown to overshoot on the same data.
- S-N7.3 Extrapolation (definition): flat extrapolation returns the end
  values; extension of the end piece agrees with the end polynomial.
- S-N7.4 Two-dimensional (identity): bilinear reproduces functions of the
  form a + b·x + c·y + d·x·y; bicubic reproduces bicubic polynomials.
- S-N7.5 Knot placement (identity): for a convex payoff the piecewise-linear
  interpolant stays within tolerance on a dense grid, and for a piecewise
  linear payoff the kinks are among the knots.
- S-N7.6 Piecewise-constant functions (identity): the integral of a constant
  c over [d₁, d₂] is c·τ(d₁, d₂); additivity over adjacent intervals;
  exp(-∫ f) of a sum is the product of the factors; point-wise algebra
  commutes with evaluation; the list form round-trips.

### N8 — Optimisation (tests/math_optimize.rs)

- S-N8.1 Classical test functions (published, Moré, Garbow and Hillstrom
  1981): Rosenbrock (minimum at (1, 1)), Powell singular (minimum at the
  origin), Freudenstein-Roth and Beale reach their known minimisers with
  L-BFGS, Nelder-Mead and Levenberg-Marquardt within tolerance.
- S-N8.2 Least squares (published): the NIST Statistical Reference Datasets
  for non-linear regression (lower and average difficulty problems) reach the
  certified parameter values to the precision expected for each problem,
  from both NIST starting points.
- S-N8.3 Bounds (identity): when the unconstrained minimiser lies outside the
  box, the bounded solver returns the box point predicted by the KKT
  conditions; a fixed parameter is returned unchanged.
- S-N8.4 Global search (published and deterministic): differential evolution
  and particle swarm reach the global minimum of Rastrigin and Ackley
  functions in low dimension within a stated budget; two runs with the same
  seed are identical; each termination reason is produced by a scenario built
  for it.
- S-N8.5 Calibration driver (identity): quotes generated in the test from a
  known parameter vector are fitted back to it (relative and absolute
  distance), with zero RMSE up to the optimiser tolerance; infeasible points
  are skipped.
- S-N8.6 Quadratic programme (identity): small problems solved by hand
  through their KKT system in the test; non-negative multipliers and
  complementary slackness at the optimum; multiplying C and d by a positive
  factor leaves the solution unchanged and multiplies the multipliers by the
  same factor; inconsistent constraints yield the typed error with the
  offending index.
- S-N8.7 Constrained least squares (independent): without constraints the
  coefficients equal the normal-equation solution; with an active bound the
  solution matches the KKT solution computed in the test.
- S-N8.8 Linear programmes (published): textbook examples from Chvátal
  (1983) and Dantzig (1963) give their known optimum; infeasible and
  unbounded examples from the same sources are detected; the linear
  programme for minimum conditional value-at-risk of Rockafellar and Uryasev
  (2000), with a variable unbounded below, matches the CVaR computed directly
  from the scenarios in the test; the symmetry check is true for a problem
  built symmetric in two variables and false after perturbing one
  coefficient.

## References

- M. Abramowitz and I. A. Stegun, *Handbook of Mathematical Functions*,
  National Bureau of Standards, 1964 (chapters 6, 7 and 26; formula 7.1.26).
- NIST Digital Library of Mathematical Functions, https://dlmf.nist.gov
  (chapters 5, 7, 8, 18 and 26).
- W. J. Cody, "Rational Chebyshev approximations for the error function",
  *Mathematics of Computation* 23, 1969.
- M. J. Wichura, "Algorithm AS 241: the percentage points of the normal
  distribution", *Applied Statistics* 37, 1988.
- C. Lanczos, "A precision approximation of the gamma function", *SIAM
  Journal on Numerical Analysis* B1, 1964.
- C. G. Ding, "Algorithm AS 275: computing the non-central chi-squared
  distribution function", *Applied Statistics* 41, 1992.
- A. Genz, "Numerical computation of rectangular bivariate and trivariate
  normal and t probabilities", *Statistics and Computing* 14, 2004;
  Z. Drezner and G. O. Wesolowsky, "On the computation of the bivariate
  normal integral", *Journal of Statistical Computation and Simulation* 35,
  1990.
- G. H. Golub and J. H. Welsch, "Calculation of Gauss quadrature rules",
  *Mathematics of Computation* 23, 1969.
- J. E. Bresenham, "Algorithm for computer control of a digital plotter",
  *IBM Systems Journal* 4, 1965.
- W. H. Press, S. A. Teukolsky, W. T. Vetterling and B. P. Flannery,
  *Numerical Recipes*, 3rd edition, Cambridge University Press, 2007
  (cubic roots, incomplete gamma and beta, safeguarded Newton, Brent).
- R. J. Hyndman and Y. Fan, "Sample quantiles in statistical packages", *The
  American Statistician* 50, 1996.
- B. P. Welford, "Note on a method for calculating corrected sums of squares
  and products", *Technometrics* 4, 1962.
- G. H. Golub and C. F. Van Loan, *Matrix Computations*, 4th edition, Johns
  Hopkins University Press, 2013 (Cholesky, LU, symmetric eigenproblem,
  banded systems).
- N. J. Higham, "Computing the nearest correlation matrix — a problem from
  finance", *IMA Journal of Numerical Analysis* 22, 2002.
- P. Jäckel, *Monte Carlo Methods in Finance*, Wiley, 2002 (spectral
  decomposition of correlation matrices, Sobol initialisation, Brownian
  bridge).
- H. A. van der Vorst, "Bi-CGSTAB: a fast and smoothly converging variant of
  Bi-CG", *SIAM Journal on Scientific and Statistical Computing* 13, 1992.
- M. Matsumoto and T. Nishimura, "Mersenne Twister: a 623-dimensionally
  equidistributed uniform pseudo-random number generator", *ACM TOMACS* 8,
  1998, and the authors' reference implementation with its published output.
- I. M. Sobol', "On the distribution of points in a cube and the approximate
  evaluation of integrals", *USSR Computational Mathematics and Mathematical
  Physics* 7, 1967.
- S. Joe and F. Y. Kuo, "Constructing Sobol sequences with better
  two-dimensional projections", *SIAM Journal on Scientific Computing* 30,
  2008, with the published direction numbers.
- P. Bratley and B. L. Fox, "Algorithm 659: implementing Sobol's
  quasirandom sequence generator", *ACM TOMS* 14, 1988.
- A. B. Owen, "Randomly permuted (t, m, s)-nets and (t, s)-sequences", in
  *Monte Carlo and Quasi-Monte Carlo Methods in Scientific Computing*,
  Springer, 1995.
- P. Glasserman, *Monte Carlo Methods in Financial Engineering*, Springer,
  2003 (Brownian bridge, Poisson processes, correlated normals).
- R. P. Brent, *Algorithms for Minimization without Derivatives*,
  Prentice-Hall, 1973.
- R. Piessens, E. de Doncker-Kapenga, C. W. Überhuber and D. K. Kahaner,
  *QUADPACK: a subroutine package for automatic integration*, Springer, 1983.
- P. Wynn, "On a device for computing the eₘ(Sₙ) transformation", *MTAC* 10,
  1956.
- F. N. Fritsch and R. E. Carlson, "Monotone piecewise cubic interpolation",
  *SIAM Journal on Numerical Analysis* 17, 1980; J. M. Hyman, "Accurate
  monotonicity preserving cubic interpolation", *SIAM Journal on Scientific
  and Statistical Computing* 4, 1983.
- C. de Boor, *A Practical Guide to Splines*, Springer, 1978.
- P. Carr and D. Madan, "Optimal positioning in derivative securities",
  *Quantitative Finance* 1, 2001 (static replication by vanilla options).
- J. J. Moré, "The Levenberg-Marquardt algorithm: implementation and theory",
  *Lecture Notes in Mathematics* 630, Springer, 1978; C. B. Markwardt,
  "Non-linear least-squares fitting in IDL with MPFIT", *ASP Conference
  Series* 411, 2009.
- J. J. Moré, B. S. Garbow and K. E. Hillstrom, "Testing unconstrained
  optimization software", *ACM TOMS* 7, 1981.
- NIST Statistical Reference Datasets, non-linear regression,
  https://www.itl.nist.gov/div898/strd/.
- R. Storn and K. Price, "Differential evolution — a simple and efficient
  heuristic for global optimization over continuous spaces", *Journal of
  Global Optimization* 11, 1997.
- J. Kennedy and R. Eberhart, "Particle swarm optimization", *Proceedings of
  the IEEE International Conference on Neural Networks*, 1995; G. I. Evers
  and M. Ben Ghalia, "Regrouping particle swarm optimization: a new global
  optimization algorithm with improved performance consistency across
  benchmarks", *IEEE SMC*, 2009.
- J. Nocedal, "Updating quasi-Newton matrices with limited storage",
  *Mathematics of Computation* 35, 1980; D. C. Liu and J. Nocedal, "On the
  limited memory BFGS method for large scale optimization", *Mathematical
  Programming* 45, 1989; R. H. Byrd, P. Lu, J. Nocedal and C. Zhu, "A limited
  memory algorithm for bound constrained optimization", *SIAM Journal on
  Scientific Computing* 16, 1995.
- J. A. Nelder and R. Mead, "A simplex method for function minimization",
  *The Computer Journal* 7, 1965; N. Hansen, "The CMA evolution strategy: a
  tutorial", arXiv:1604.00772, 2016.
- D. Goldfarb and A. Idnani, "A numerically stable dual method for solving
  strictly convex quadratic programs", *Mathematical Programming* 27, 1983.
- V. Chvátal, *Linear Programming*, W. H. Freeman, 1983; G. B. Dantzig,
  *Linear Programming and Extensions*, Princeton University Press, 1963;
  IBM, MPS file format description (public documentation).
- R. T. Rockafellar and S. Uryasev, "Optimization of conditional
  value-at-risk", *Journal of Risk* 2, 2000.
- R. A. Fisher, "The use of multiple measurements in taxonomic problems",
  *Annals of Eugenics* 7, 1936 (Iris data set, public domain).
- C. Eckart and G. Young, "The approximation of one matrix by another of
  lower rank", *Psychometrika* 1, 1936.
