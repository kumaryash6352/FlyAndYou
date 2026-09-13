//! Throwaway full-graph compute spike. Not a game worker or RNG implementation.
use candle_core::{
    CpuStorage, CustomOp1, DType, Device, Layout, MetalStorage, Result, Shape, Tensor,
    backend::BackendStorage,
};
use candle_metal_kernels::metal::{Buffer, ComputePipeline};
use objc2_metal::{MTLCompileOptions, MTLSize};
use rayon::prelude::*;
use serde_json::json;
use std::{fs, path::Path, sync::Arc, time::Instant};

fn f32s(p: &Path) -> Vec<f32> {
    fs::read(p)
        .unwrap()
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}
fn u32s(p: &Path) -> Vec<u32> {
    fs::read(p)
        .unwrap()
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}
fn save(p: &Path, v: &[f32]) {
    fs::write(
        p,
        v.iter().flat_map(|x| x.to_le_bytes()).collect::<Vec<_>>(),
    )
    .unwrap();
}
fn stats(v: &[f64]) -> serde_json::Value {
    let mut x = v.to_vec();
    x.sort_by(f64::total_cmp);
    let pct = |q: f64| {
        let k = q * (x.len() - 1) as f64;
        let a = k.floor() as usize;
        x[a] + (x[k.ceil() as usize] - x[a]) * (k - a as f64)
    };
    json!({"n":x.len(),"median_ms":pct(0.5),"p95_ms":pct(0.95),"min_ms":x[0],"max_ms":x[x.len()-1]})
}
struct Graph {
    rows: Vec<u32>,
    cols: Vec<u32>,
    weights: Vec<f32>,
}
impl Graph {
    fn step(&self, a: &[f32], input: &[f32], noise: &[f32], out: &mut [f32], parallel: bool) {
        let update = |row: usize, o: &mut f32| {
            let mut sum = 0f32;
            for j in self.rows[row] as usize..self.rows[row + 1] as usize {
                sum += self.weights[j] * a[self.cols[j] as usize];
            }
            let drive = (0.9f32 * sum + input[row]) + noise[row];
            *o = 0.7f32 * a[row] + 0.3f32 * drive.max(0.).tanh();
        };
        if parallel {
            out.par_iter_mut()
                .enumerate()
                .for_each(|(r, o)| update(r, o));
        } else {
            out.iter_mut().enumerate().for_each(|(r, o)| update(r, o));
        }
    }
}
struct CsrStep {
    n: usize,
    rows: Arc<Buffer>,
    cols: Arc<Buffer>,
    weights: Arc<Buffer>,
    input: Arc<Buffer>,
    noise: Arc<Buffer>,
    pipeline: ComputePipeline,
    simd: bool,
}
impl CustomOp1 for CsrStep {
    fn name(&self) -> &'static str {
        "experimental-csr-recurrence"
    }
    fn cpu_fwd(&self, _: &CpuStorage, _: &Layout) -> Result<(CpuStorage, Shape)> {
        candle_core::bail!("GPU-only probe")
    }
    fn metal_fwd(&self, a: &MetalStorage, l: &Layout) -> Result<(MetalStorage, Shape)> {
        assert_eq!(l.start_offset(), 0);
        assert!(l.is_contiguous());
        assert_eq!(a.dtype(), DType::F32);
        let dev = a.device();
        let out = dev.new_buffer(self.n, DType::F32, "csr-out")?;
        let guard = dev.command_encoder()?;
        let encoder = guard.as_ref();
        encoder.set_compute_pipeline_state(&self.pipeline);
        candle_metal_kernels::set_params!(
            encoder,
            (
                self.n as u32,
                self.rows.as_ref(),
                self.cols.as_ref(),
                self.weights.as_ref(),
                a.buffer(),
                self.input.as_ref(),
                self.noise.as_ref(),
                candle_metal_kernels::Output::new(out.as_ref())
            )
        );
        let threads = if self.simd { self.n * 32 } else { self.n };
        encoder.dispatch_thread_groups(
            MTLSize {
                width: threads.div_ceil(256),
                height: 1,
                depth: 1,
            },
            MTLSize {
                width: 256,
                height: 1,
                depth: 1,
            },
        );
        Ok((
            MetalStorage::new(out, dev.clone(), self.n, DType::F32),
            Shape::from(self.n),
        ))
    }
}
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let root = Path::new(&args[1]);
    let mode = &args[2];
    let graph = Graph {
        rows: u32s(&root.join("rows.u32")),
        cols: u32s(&root.join("columns.u32")),
        weights: f32s(&root.join("values.f32")),
    };
    let initial = f32s(&root.join("initial.f32"));
    let n = initial.len();
    let inputs = f32s(&root.join("inputs.f32"));
    let noise = f32s(&root.join("noise.f32"));
    let reference = f32s(&root.join("reference.f32"));
    let count = inputs.len() / n;
    assert_eq!(noise.len(), count * 5 * n);
    assert_eq!(reference.len(), count * n);
    let mut latencies = Vec::new();
    let mut states = Vec::new();
    if mode.starts_with("cpu") {
        let threads = if mode == "cpu1" { 1 } else { 5 };
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(threads)
            .build()
            .unwrap();
        // Warm the loop/pool without advancing the measured trajectory.
        let mut scratch = vec![0.; n];
        pool.install(|| {
            graph.step(
                &initial,
                &inputs[..n],
                &noise[..n],
                &mut scratch,
                threads > 1,
            )
        });
        let mut a = initial.clone();
        let mut out = vec![0.; n];
        for d in 0..count {
            let t = Instant::now();
            pool.install(|| {
                for k in 0..5 {
                    graph.step(
                        &a,
                        &inputs[d * n..(d + 1) * n],
                        &noise[(d * 5 + k) * n..(d * 5 + k + 1) * n],
                        &mut out,
                        threads > 1,
                    );
                    std::mem::swap(&mut a, &mut out);
                }
            });
            latencies.push(t.elapsed().as_secs_f64() * 1000.);
            states.extend_from_slice(&a);
        }
    } else {
        let dev = Device::new_metal(0)?;
        let md = dev.as_metal_device()?;
        let opts = MTLCompileOptions::new();
        #[allow(deprecated)]
        opts.setFastMathEnabled(false);
        let library = md
            .metal_device()
            .new_library_with_source(include_str!("step.metal"), Some(&opts))
            .map_err(candle_core::Error::msg)?;
        let simd = mode == "metal-simd";
        let function = library
            .get_function(if simd { "csr_simd" } else { "csr_serial" }, None)
            .map_err(candle_core::Error::msg)?;
        let pipeline = md
            .metal_device()
            .new_compute_pipeline_state_with_function(&function)
            .map_err(candle_core::Error::msg)?;
        let rows = md.new_buffer_with_data(&graph.rows)?;
        let cols = md.new_buffer_with_data(&graph.cols)?;
        let weights = md.new_buffer_with_data(&graph.weights)?;
        let mut a = Tensor::from_slice(&initial, n, &dev)?;
        // Warm pipeline and memory before resetting the measured trajectory.
        let warm = CsrStep {
            n,
            rows: rows.clone(),
            cols: cols.clone(),
            weights: weights.clone(),
            input: md.new_buffer_with_data(&inputs[..n])?,
            noise: md.new_buffer_with_data(&noise[..n])?,
            pipeline: pipeline.clone(),
            simd,
        };
        let _ = a.apply_op1_no_bwd(&warm)?.to_vec1::<f32>()?;
        for d in 0..count {
            let t = Instant::now();
            let input = md.new_buffer_with_data(&inputs[d * n..(d + 1) * n])?;
            // Hold all host-uploaded noise buffers through completed readback.
            let noises: Vec<_> = (0..5)
                .map(|k| md.new_buffer_with_data(&noise[(d * 5 + k) * n..(d * 5 + k + 1) * n]))
                .collect::<Result<_>>()?;
            let mut intermediates = Vec::new();
            for noise in &noises {
                let op = CsrStep {
                    n,
                    rows: rows.clone(),
                    cols: cols.clone(),
                    weights: weights.clone(),
                    input: input.clone(),
                    noise: noise.clone(),
                    pipeline: pipeline.clone(),
                    simd,
                };
                let next = a.apply_op1_no_bwd(&op)?;
                intermediates.push(a);
                a = next;
            }
            let output = a.to_vec1::<f32>()?; // Synchronizes: do not time enqueue alone.
            latencies.push(t.elapsed().as_secs_f64() * 1000.);
            states.extend_from_slice(&output);
        }
    }
    let max_error = states
        .iter()
        .zip(&reference)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    let rmse = (states
        .iter()
        .zip(&reference)
        .map(|(a, b)| (*a as f64 - *b as f64).powi(2))
        .sum::<f64>()
        / states.len() as f64)
        .sqrt();
    assert!(states.iter().all(|v| v.is_finite() && *v >= 0. && *v <= 1.));
    save(&root.join(format!("{mode}-states.f32")), &states);
    let report = json!({"backend":mode,"nodes":n,"edges":graph.weights.len(),"timing_scope":"five recurrent steps, precomputed noise; Metal includes host input/noise upload and full activity readback; excludes loading, RNG generation, image encoding, motor and socket","latency":stats(&latencies),"max_abs_error":max_error,"rmse":rmse});
    fs::write(
        root.join(format!("{mode}.json")),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
    Ok(())
}
