//! Benchmarks Criterion : pricing par lot (J9c) et calibration rapide (J21-fast).
//!
//! `cargo bench` mesure les chemins chauds dont les jalons revendiquent la perf
//! (« 100 contrats en < 500 ms », calibration « < 1 sec »), pour suivre les
//! régressions dans le temps.

use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;

use kontract::pricer::McConfig;
use kontract::products::{european_call, european_put, straddle};
use kontract::{
    calibration::FastCalibrationConfig, fit_gbm_volatility, price_batch_gbm, Contract, Gbm,
};

fn bench_batch_pricing(c: &mut Criterion) {
    // 100 contrats vanille variés, simulation unique partagée (J9c).
    let contracts: Vec<Contract> = (0..100)
        .map(|i| {
            let k = 80.0 + i as f64;
            match i % 3 {
                0 => european_call("S", k, 1.0, "USD"),
                1 => european_put("S", k, 1.0, "USD"),
                _ => straddle("S", k, 1.0, "USD"),
            }
        })
        .collect();
    let model = Gbm::new("S", 100.0, 0.05, 0.2);
    let cfg = McConfig {
        n_paths: 50_000,
        seed: 42,
        steps_per_year: 50,
        rate: 0.05,
        variance_reduction: None,
    };

    c.bench_function("batch_pricing_100_contracts", |b| {
        b.iter(|| price_batch_gbm(black_box(&contracts), &model, &cfg).unwrap());
    });
}

fn bench_calibration(c: &mut Criterion) {
    let call = european_call("S", 100.0, 1.0, "USD");
    let market = vec![(100.0, 10.45)];
    let cfg = FastCalibrationConfig {
        n_paths: 2000,
        ..Default::default()
    };

    c.bench_function("fit_gbm_volatility", |b| {
        b.iter(|| fit_gbm_volatility(black_box(&call), &[1.0], &market, 0.05, &cfg).unwrap());
    });
}

criterion_group!(benches, bench_batch_pricing, bench_calibration);
criterion_main!(benches);
