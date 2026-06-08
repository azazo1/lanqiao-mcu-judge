use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use criterion::{Criterion, criterion_group, criterion_main};
use std::hint::black_box;
use stcjudge::{BenchHarness, Simulator, WaveCaptureOptions, run_script_source};

const TOGGLE_P34_LOOP: &[u8] = &[0xB2, 0xB4, 0x80, 0xFC];
static FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

fn toggle_p34_harness() -> BenchHarness {
    BenchHarness::from_code(TOGGLE_P34_LOOP.to_vec())
}

fn unique_temp_path(label: &str, ext: &str) -> PathBuf {
    let id = FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("stcjudge-bench-{label}-{id}.{ext}"))
}

fn bench_core_exec(c: &mut Criterion) {
    let mut group = c.benchmark_group("core_exec");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("construct_nop", |b| {
        b.iter(|| {
            let sim = BenchHarness::nop();
            black_box(sim.sim_time_ns());
        });
    });

    group.bench_function("construct_toggle_p34_loop", |b| {
        b.iter(|| {
            let sim = toggle_p34_harness();
            black_box(sim.sim_time_ns());
        });
    });

    group.bench_function("run_nop_1ms", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut sim = BenchHarness::nop();
            for _ in 0..iters {
                sim.reset().expect("reset nop simulator");
                let start = Instant::now();
                sim.run_ms(1).expect("run nop simulator");
                black_box(sim.sim_time_ns());
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("run_toggle_p34_1ms", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut sim = toggle_p34_harness();
            for _ in 0..iters {
                sim.reset().expect("reset toggle loop");
                let start = Instant::now();
                sim.run_ms(1).expect("run toggle loop");
                black_box(sim.sim_time_ns());
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("run_to_p34_flip", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut sim = toggle_p34_harness();
            for _ in 0..iters {
                sim.reset().expect("reset toggle loop");
                let start = Instant::now();
                let elapsed_ns = sim.run_to_pin_flip(3, 4).expect("wait for P3.4 flip");
                black_box(elapsed_ns);
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

fn bench_input_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_ops");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("set_key_s7_pair", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut sim = BenchHarness::nop();
            for _ in 0..iters {
                let start = Instant::now();
                sim.set_key("S7", true).expect("press key");
                sim.set_key("S7", false).expect("release key");
                black_box(sim.sim_time_ns());
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("tap_key_s7_hold_20ms", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut sim = BenchHarness::nop();
            for _ in 0..iters {
                sim.reset().expect("reset nop simulator");
                let start = Instant::now();
                sim.tap_key("S7", 20).expect("tap key");
                black_box(sim.sim_time_ns());
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("set_voltage_ain1", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut sim = BenchHarness::nop();
            for _ in 0..iters {
                let start = Instant::now();
                sim.set_voltage("AIN1", 2.75).expect("set AIN1");
                black_box(sim.sim_time_ns());
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("uart_write_32bytes", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut sim = BenchHarness::nop();
            let payload = [0x55; 32];
            for _ in 0..iters {
                let start = Instant::now();
                sim.uart_write(&payload).expect("inject uart bytes");
                black_box(sim.sim_time_ns());
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

fn bench_observe_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("observe_ops");
    group.sample_size(20);
    group.measurement_time(Duration::from_secs(8));

    group.bench_function("snapshot_text_nop", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let sim = BenchHarness::nop();
            for _ in 0..iters {
                let start = Instant::now();
                black_box(sim.snapshot_text());
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("snapshot_text_toggle_after_1ms", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let mut sim = toggle_p34_harness();
            for _ in 0..iters {
                sim.reset().expect("reset toggle loop");
                sim.run_ms(1).expect("warm up toggle loop");
                let start = Instant::now();
                black_box(sim.snapshot_text());
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

fn bench_script_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("script_ops");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    let script = r#"
        set_frequency_hz(2000);
        let rounds = 200;
        let total = 0;
        for i in 0..rounds {
            let next_ns = sim_time_ns() + 1000;
            total += run_to(|| sim_time_ns() >= next_ns, 10_000);
        }
        print(total);
    "#;

    group.bench_function("run_to_predicate_200_rounds", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let sim = Simulator::nop(false);
                let start = Instant::now();
                run_script_source(sim, "bench:run_to_callback", script)
                    .expect("run callback benchmark");
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

fn bench_wave_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("wave_ops");
    group.sample_size(10);
    group.measurement_time(Duration::from_secs(10));

    group.bench_function("wave_json_toggle_1ms", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let path = unique_temp_path("wave-json", "json");
                let options = WaveCaptureOptions {
                    json_path: Some(path.clone()),
                    ..WaveCaptureOptions::default()
                };
                let start = Instant::now();
                {
                    let mut sim = BenchHarness::from_code_with_options(
                        TOGGLE_P34_LOOP.to_vec(),
                        options,
                    );
                    sim.run_ms(1).expect("run wave json benchmark");
                    black_box(sim.sim_time_ns());
                }
                let file_size = fs::metadata(&path).expect("wave json metadata").len();
                black_box(file_size);
                let _ = fs::remove_file(&path);
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("wave_msgpack_toggle_1ms", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let path = unique_temp_path("wave-msgpack", "msgpack");
                let options = WaveCaptureOptions {
                    msgpack_path: Some(path.clone()),
                    ..WaveCaptureOptions::default()
                };
                let start = Instant::now();
                {
                    let mut sim = BenchHarness::from_code_with_options(
                        TOGGLE_P34_LOOP.to_vec(),
                        options,
                    );
                    sim.run_ms(1).expect("run wave msgpack benchmark");
                    black_box(sim.sim_time_ns());
                }
                let file_size = fs::metadata(&path).expect("wave msgpack metadata").len();
                black_box(file_size);
                let _ = fs::remove_file(&path);
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("wave_html_toggle_1ms", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let path = unique_temp_path("wave-html", "html");
                let options = WaveCaptureOptions {
                    html_path: Some(path.clone()),
                    ..WaveCaptureOptions::default()
                };
                let start = Instant::now();
                {
                    let mut sim = BenchHarness::from_code_with_options(
                        TOGGLE_P34_LOOP.to_vec(),
                        options,
                    );
                    sim.run_ms(1).expect("run wave html benchmark");
                    black_box(sim.sim_time_ns());
                }
                let file_size = fs::metadata(&path).expect("wave html metadata").len();
                black_box(file_size);
                let _ = fs::remove_file(&path);
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("wave_uart_json_32bytes", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            let payload = [0x55; 32];
            for _ in 0..iters {
                let path = unique_temp_path("wave-uart-json", "json");
                let options = WaveCaptureOptions {
                    json_path: Some(path.clone()),
                    ..WaveCaptureOptions::default()
                };
                let start = Instant::now();
                {
                    let mut sim = BenchHarness::from_code_with_options(vec![0x00], options);
                    sim.uart_write(&payload).expect("inject uart bytes");
                    sim.run_us(100).expect("advance uart wave");
                    black_box(sim.sim_time_ns());
                }
                let file_size = fs::metadata(&path).expect("wave uart json metadata").len();
                black_box(file_size);
                let _ = fs::remove_file(&path);
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("wave_i2c_json_write", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let path = unique_temp_path("wave-i2c-json", "json");
                let options = WaveCaptureOptions {
                    json_path: Some(path.clone()),
                    ..WaveCaptureOptions::default()
                };
                let start = Instant::now();
                {
                    let mut sim = BenchHarness::from_code_with_options(vec![0x00], options);
                    sim.i2c_idle().expect("i2c idle");
                    sim.i2c_start().expect("i2c start");
                    sim.i2c_write_byte(0x90).expect("i2c addr");
                    sim.i2c_write_byte(0x40).expect("i2c control");
                    sim.i2c_write_byte(0x7F).expect("i2c data");
                    sim.i2c_stop().expect("i2c stop");
                    black_box(sim.sim_time_ns());
                }
                let file_size = fs::metadata(&path).expect("wave i2c json metadata").len();
                black_box(file_size);
                let _ = fs::remove_file(&path);
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("wave_onewire_json_reset_skiprom", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let path = unique_temp_path("wave-onewire-json", "json");
                let options = WaveCaptureOptions {
                    json_path: Some(path.clone()),
                    ..WaveCaptureOptions::default()
                };
                let start = Instant::now();
                {
                    let mut sim = BenchHarness::from_code_with_options(vec![0x00], options);
                    sim.onewire_idle().expect("onewire idle");
                    sim.onewire_reset().expect("onewire reset");
                    sim.onewire_write_byte(0xCC).expect("skip rom");
                    sim.onewire_write_byte(0x44).expect("convert t");
                    black_box(sim.sim_time_ns());
                }
                let file_size =
                    fs::metadata(&path).expect("wave onewire json metadata").len();
                black_box(file_size);
                let _ = fs::remove_file(&path);
                total += start.elapsed();
            }
            total
        });
    });

    group.bench_function("wave_ds1302_json_write_seconds", |b| {
        b.iter_custom(|iters| {
            let mut total = Duration::ZERO;
            for _ in 0..iters {
                let path = unique_temp_path("wave-ds1302-json", "json");
                let options = WaveCaptureOptions {
                    json_path: Some(path.clone()),
                    ..WaveCaptureOptions::default()
                };
                let start = Instant::now();
                {
                    let mut sim = BenchHarness::from_code_with_options(vec![0x00], options);
                    sim.ds1302_idle().expect("ds1302 idle");
                    sim.ds1302_begin().expect("ds1302 begin");
                    sim.ds1302_write_byte(0x80).expect("seconds write cmd");
                    sim.ds1302_write_byte(0x25).expect("seconds value");
                    sim.ds1302_end().expect("ds1302 end");
                    black_box(sim.sim_time_ns());
                }
                let file_size =
                    fs::metadata(&path).expect("wave ds1302 json metadata").len();
                black_box(file_size);
                let _ = fs::remove_file(&path);
                total += start.elapsed();
            }
            total
        });
    });

    group.finish();
}

criterion_group!(
    sim,
    bench_core_exec,
    bench_input_ops,
    bench_observe_ops,
    bench_script_ops,
    bench_wave_ops
);
criterion_main!(sim);
