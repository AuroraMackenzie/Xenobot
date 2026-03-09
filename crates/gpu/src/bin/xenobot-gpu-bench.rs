//! Minimal GPU benchmark entrypoint for Apple Silicon Metal/MPS verification.
//!
//! This binary measures square matrix multiplication throughput on CPU and GPU
//! and reports a compact summary for regression tracking.

use std::time::Instant;
use xenobot_gpu::linalg::MatrixOps;
use xenobot_gpu::metal::MetalDevice;

#[derive(Debug, Clone, Copy)]
enum OutputFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, Copy)]
struct BenchOptions {
    size: usize,
    iterations: usize,
    format: OutputFormat,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchReport {
    size: usize,
    iterations: usize,
    cpu_avg_ms: f64,
    cpu_gflops: f64,
    gpu_available: bool,
    gpu_device: Option<String>,
    gpu_avg_ms: Option<f64>,
    gpu_gflops: Option<f64>,
    max_abs_diff: Option<f32>,
    error: Option<String>,
}

fn parse_args() -> Result<BenchOptions, String> {
    let mut size = 256usize;
    let mut iterations = 8usize;
    let mut format = OutputFormat::Text;
    let mut args = std::env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--size" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--size requires a value".to_string())?;
                size = value
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --size value '{}': {}", value, e))?;
            }
            "--iters" | "--iterations" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--iters requires a value".to_string())?;
                iterations = value
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --iters value '{}': {}", value, e))?;
            }
            "--format" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--format requires a value".to_string())?;
                format = match value.as_str() {
                    "text" => OutputFormat::Text,
                    "json" => OutputFormat::Json,
                    _ => return Err(format!("invalid --format '{}', expected text|json", value)),
                };
            }
            "--help" | "-h" => {
                println!(
                    "Usage: xenobot-gpu-bench [--size N] [--iters N] [--format text|json]\n\
                     Defaults: --size 256 --iters 8 --format text"
                );
                std::process::exit(0);
            }
            unknown => return Err(format!("unknown argument '{}'", unknown)),
        }
    }

    size = size.clamp(16, 2048);
    iterations = iterations.clamp(1, 128);

    Ok(BenchOptions {
        size,
        iterations,
        format,
    })
}

fn build_input(size: usize) -> (Vec<f32>, Vec<f32>) {
    let mut a = vec![0.0f32; size * size];
    let mut b = vec![0.0f32; size * size];
    for i in 0..size {
        for j in 0..size {
            let idx = i * size + j;
            a[idx] = ((i * 31 + j * 17) % 97) as f32 / 97.0;
            b[idx] = ((i * 13 + j * 29) % 89) as f32 / 89.0;
        }
    }
    (a, b)
}

fn cpu_matmul(a: &[f32], b: &[f32], n: usize) -> Vec<f32> {
    let mut c = vec![0.0f32; n * n];
    for i in 0..n {
        for k in 0..n {
            let aik = a[i * n + k];
            if aik == 0.0 {
                continue;
            }
            for j in 0..n {
                c[i * n + j] += aik * b[k * n + j];
            }
        }
    }
    c
}

fn avg_ms(started: Instant, iterations: usize) -> f64 {
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    elapsed / iterations as f64
}

fn gflops_for_square(n: usize, avg_ms: f64) -> f64 {
    if avg_ms <= 0.0 {
        return 0.0;
    }
    let ops = 2.0 * (n as f64).powi(3);
    let sec = avg_ms / 1000.0;
    (ops / sec) / 1e9
}

fn max_abs_diff(a: &[f32], b: &[f32]) -> f32 {
    a.iter()
        .zip(b.iter())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max)
}

fn print_report(report: &BenchReport, format: OutputFormat) -> Result<(), String> {
    match format {
        OutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(report)
                    .map_err(|e| format!("failed to serialize report: {}", e))?
            );
        }
        OutputFormat::Text => {
            println!("xenobot gpu benchmark");
            println!("size: {}", report.size);
            println!("iterations: {}", report.iterations);
            println!("cpu avg(ms): {:.3}", report.cpu_avg_ms);
            println!("cpu gflops: {:.3}", report.cpu_gflops);
            println!("gpu available: {}", report.gpu_available);
            if let Some(name) = report.gpu_device.as_deref() {
                println!("gpu device: {}", name);
            }
            if let Some(v) = report.gpu_avg_ms {
                println!("gpu avg(ms): {:.3}", v);
            }
            if let Some(v) = report.gpu_gflops {
                println!("gpu gflops: {:.3}", v);
            }
            if let Some(v) = report.max_abs_diff {
                println!("max abs diff: {:.6}", v);
            }
            if let Some(err) = report.error.as_deref() {
                println!("error: {}", err);
            }
        }
    }
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("xenobot-gpu-bench: {}", err);
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let options = parse_args()?;
    let (a, b) = build_input(options.size);

    let mut cpu_out = Vec::new();
    let cpu_started = Instant::now();
    for _ in 0..options.iterations {
        cpu_out = cpu_matmul(&a, &b, options.size);
    }
    let cpu_avg_ms = avg_ms(cpu_started, options.iterations);
    let cpu_gflops = gflops_for_square(options.size, cpu_avg_ms);

    let mut report = BenchReport {
        size: options.size,
        iterations: options.iterations,
        cpu_avg_ms,
        cpu_gflops,
        gpu_available: false,
        gpu_device: None,
        gpu_avg_ms: None,
        gpu_gflops: None,
        max_abs_diff: None,
        error: None,
    };

    match MetalDevice::new() {
        Ok(device) => {
            report.gpu_available = true;
            report.gpu_device = Some(device.name().to_string());
            let matrix_ops = MatrixOps::new(device);
            match matrix_ops.matmul(&a, &b, options.size, options.size, options.size) {
                Ok(first_gpu) => {
                    let gpu_started = Instant::now();
                    let mut gpu_out = first_gpu;
                    for _ in 1..options.iterations {
                        gpu_out = matrix_ops
                            .matmul(&a, &b, options.size, options.size, options.size)
                            .map_err(|e| e.to_string())?;
                    }
                    let gpu_avg_ms = avg_ms(gpu_started, options.iterations.max(1));
                    report.gpu_avg_ms = Some(gpu_avg_ms);
                    report.gpu_gflops = Some(gflops_for_square(options.size, gpu_avg_ms));
                    report.max_abs_diff = Some(max_abs_diff(&cpu_out, &gpu_out));
                }
                Err(err) => {
                    report.error = Some(err.to_string());
                }
            }
        }
        Err(err) => {
            report.error = Some(err.to_string());
        }
    }

    print_report(&report, options.format)
}
