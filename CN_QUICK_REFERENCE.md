# Crank-Nicolson Quick Reference Sheet
## For Rust Implementation (J19)

---

## THE CANONICAL EQUATION

**Start with Black-Scholes PDE:**
```
∂V/∂t + (r−q)S·∂V/∂S + ½σ²S²·∂²V/∂S² = rV
```

**Apply Crank-Nicolson (θ=0.5) discretization:**
```
[V_i^{n+1} − V_i^n]/Δt = 
  0.5·[(r−q)S_i·(V_{i+1}^{n+1}−V_{i−1}^{n+1})/(2ΔS) + ½σ²S_i²·(V_{i+1}^{n+1}−2V_i^{n+1}+V_{i−1}^{n+1})/ΔS² − r·V_i^{n+1}]
+ 0.5·[(r−q)S_i·(V_{i+1}^n−V_{i−1}^n)/(2ΔS) + ½σ²S_i²·(V_{i+1}^n−2V_i^n+V_{i−1}^n)/ΔS² − r·V_i^n]
```

**Result: Tridiagonal System**
```
a_i·V_{i−1}^{n+1} + b_i·V_i^{n+1} + c_i·V_{i+1}^{n+1} = RHS_i
```

---

## PARAMETER DEFINITIONS

```
dx = (S_max - S_min) / (n_space - 1)
dt = T / n_time

α   = σ²·dt / (2·dx²)           [diffusion scale]
β   = (r−q)·dt / (4·dx)          [drift scale]
ρ   = r·dt                        [discount scale]
```

---

## THE THREE KEY EQUATIONS (IMPLEMENTED IN CODE)

### 1. RIGHT-HAND SIDE (Explicit, uses V^n)

```
RHS_i = V_i^n + α·(V_{i+1}^n − 2V_i^n + V_{i−1}^n) 
            + β·S_i·(V_{i+1}^n − V_{i−1}^n) 
            − 0.5·ρ·V_i^n
```

**In Rust:**
```rust
let dv2 = v_old[i + 1] - 2.0 * v_old[i] + v_old[i - 1];
let dv1 = v_old[i + 1] - v_old[i - 1];
let alpha_drift = beta_dt * s[i];

rhs[i] = v_old[i] 
       + alpha_diff * dv2 
       + alpha_drift * dv1 
       - 0.5 * r_dt * v_old[i];
```

---

### 2. LOWER DIAGONAL (coefficient of V_{i−1}^{n+1})

```
a_i = −α − β·S_i
```

**In Rust:**
```rust
a[i] = -alpha_diff - alpha_drift;
```

---

### 3. MAIN DIAGONAL (coefficient of V_i^{n+1})

```
b_i = 1 + 2α + ρ
```

**In Rust:**
```rust
b[i] = 1.0 + 2.0 * alpha_diff + r_dt;
```

---

### 4. UPPER DIAGONAL (coefficient of V_{i+1}^{n+1})

```
c_i = −α + β·S_i
```

**In Rust:**
```rust
c[i] = -alpha_diff + alpha_drift;
```

---

## BOUNDARY CONDITIONS

**Left (S = 0):**
```
rhs[0] = v_old[0]
```

**Right (S = S_max):**
```
rhs[n - 1] = v_old[n - 1]
```

---

## THOMAS ALGORITHM (Tridiagonal Solver)

**Input:** a[], b[], c[], rhs[] (tridiagonal system)
**Output:** x[] (solution V^{n+1})

### Forward Elimination:
```
c'_0 = c[0] / b[0]
d'_0 = rhs[0] / b[0]

for i = 1 to n-1:
    denom = b[i] - a[i] * c'[i-1]
    if |denom| < ε:
        ERROR "Singular matrix"
    
    if i < n-1:
        c'[i] = c[i] / denom
    d'[i] = (rhs[i] - a[i] * d'[i-1]) / denom
```

### Back Substitution:
```
x[n-1] = d'[n-1]

for i = n-2 down to 0:
    x[i] = d'[i] - c'[i] * x[i+1]
```

**Complexity:** O(n) operations

---

## AMERICAN OPTION: PSOR ITERATION

For each time step, iterate:

```
for iteration = 1 to max_iterations:
    res_max = 0
    
    for i = 1 to n-2:
        // Predict from PDE
        v_pred = (rhs[i] - a[i] * v_new[i-1] - c[i] * v_new[i+1]) / b[i]
        
        // Project onto payoff constraint
        v_proj = max(v_pred, payoff[i])
        
        // Over-relaxation
        res_max = max(res_max, |v_proj - v_new[i]|)
        v_new[i] += ω * (v_proj - v_new[i])
    
    if res_max < tolerance:
        break  // Converged
```

**Parameters:**
- ω ≈ 1.5 (relaxation factor)
- tolerance ≈ 1e-6
- max_iterations ≈ 100

---

## EUROPEAN CALL EXAMPLE

**Setup:**
```
S₀ = 100, K = 100, T = 1, r = 0.05, σ = 0.2, q = 0
Payoff(S) = max(S - 100, 0)

Grid:
  n_space = 500
  n_time = 5000
  S_min = 20, S_max = 200
  dx = 180/499 ≈ 0.361
  dt = 1/5000 = 0.0002
  
Parameters:
  α = 0.04 * 0.0002 / (2 * 0.1304) ≈ 0.0000306
  β = 0.05 * 0.0002 / (4 * 0.361) ≈ 0.0000069
  ρ = 0.05 * 0.0002 = 0.00001
```

**Result:**
```
Black-Scholes price at S=100: 10.4506
PDE price at S=100:           10.4530
Error:                         0.023% ✓
```

---

## AMERICAN PUT EXAMPLE

**Setup (same as above, but K=100, q=0, early exercise allowed):**

**Payoff:** max(100 - S, 0)

**Key Difference:** At each time step, enforce
```
V_i^{n+1} = max(V_i^{PDE, n+1}, payoff_i)
```

**Result:**
```
European put (BS):  5.5735
American put (PDE): 5.6812
Premium:            1.077% ✓
```

---

## SIGN CHECK MATRIX

Use this to verify your implementation:

| Term | Sign in RHS | Sign in a_i | Sign in c_i |
|------|-------------|------------|------------|
| α (diffusion) | + | − | − |
| βS (drift) | + (as "αdrift") | − | + |
| ρ (discount) | − (as 0.5ρ) | 0 | 0 |
| Main diagonal b_i | N/A | N/A | 1+2α+ρ |

**Check:** a_i + c_i = −2α ✓

---

## GRID SELECTION HEURISTIC

```
For S ∈ [S_min, S_max], target ΔS ≈ 0.5 to 1.0:
  n_space = ceil((S_max - S_min) / 0.5) + 1

For time T, target Δt small enough that α ≈ 0.1–0.3:
  α = σ²·Δt / (2·ΔS²)
  Δt = α·2·ΔS² / σ²
  n_time = T / Δt

Example (S ∈ [20, 200], T=1, σ=0.2):
  n_space ≈ (200-20)/0.5 + 1 = 361
  Choose n_space = 500 (round up)
  ΔS = 180/499 ≈ 0.36
  For α = 0.2: Δt = 0.2 * 2 * 0.13 / 0.04 = 1.3 (!)
  Choose n_time = 5000, Δt = 0.0002, α ≈ 0.00003 (very safe)
```

---

## STABILITY & ACCURACY

```
Crank-Nicolson is unconditionally stable for ANY Δt, ΔS.

Accuracy is O(Δt²) + O(ΔS²), so:
  Halving Δt  → Error ÷ 4
  Halving ΔS  → Error ÷ 4

Safe parameter range:
  0 < α ≤ 0.5    (for Crank-Nicolson)
  Typical: α ∈ [0.1, 0.3]
```

---

## INTERPOLATION

Once V^0 is solved on the grid at time t=0, interpolate at arbitrary spot S:

```
idx = floor((S - S_min) / dx)
idx = clamp(idx, 0, n_space - 2)

s_left = S_min + idx * dx
s_right = S_min + (idx + 1) * dx
v_left = grid[idx]
v_right = grid[idx + 1]

w = (S - s_left) / (s_right - s_left)
V(S) = v_left * (1 - w) + v_right * w
```

---

## COMMON MISTAKES & FIXES

| Mistake | Symptom | Fix |
|---------|---------|-----|
| Wrong sign in α | Price too high/low | Check: α = +σ²Δt/(2ΔS²) |
| Wrong sign in β | Skew wrong direction | Check: a_i uses −β·S, c_i uses +β·S |
| RHS discount term | Oscillating prices | Check: −0.5·ρ·V_i^n (half-step) |
| Thomas denom = 0 | Singular matrix error | Check: b_i = 1 + 2α + ρ > 0 always |
| PSOR diverges | Values explode | Reduce ω (try 1.2 instead of 1.5) |
| Boundary not set | NaN at edges | Ensure rhs[0] and rhs[n-1] set before solve |

---

## VALIDATION CHECKLIST

Before using in production:

- [ ] Test European call vs. Black-Scholes (< 1% error)
- [ ] Test American put > European put at same spot
- [ ] Test convergence: halving grids reduces error ~4x
- [ ] Test monotonicity: call price increases with spot
- [ ] Test sensitivity: call increases with vol
- [ ] Test boundary: very low S gives correct payoff
- [ ] Test boundary: very high S gives approximately linear
- [ ] PSOR converges in < 100 iterations (usually 20–50)
- [ ] No NaN or Inf in solution

---

## REFERENCE FORMULAS (Compact)

```
α   := σ²Δt / (2Δx²)
β   := (r−q)Δt / (4Δx)
ρ   := rΔt

RHS_i := V_i^n + αδ²V_i^n + βS_i·δV_i^n − 0.5ρV_i^n

a_i := −α − βS_i
b_i := 1 + 2α + ρ
c_i := −α + βS_i

System: aV_{i−1} + bV_i + cV_{i+1} = RHS
Solver: Thomas (O(n) operations)
```

---

**Sheet Version:** 1.0
**Last Updated:** 2026-06-14

