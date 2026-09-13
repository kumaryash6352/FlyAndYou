use crate::profile::Profile;
use candle_core::{
    CpuStorage, CustomOp1, DType, Device, Layout, MetalStorage, Shape, Tensor,
    backend::BackendStorage,
};
use candle_metal_kernels::metal::{Buffer, ComputePipeline};
use objc2_metal::{MTLCompileOptions, MTLSize};
use std::sync::Arc;

pub enum Compute {
    Metal(Metal),
    Cpu(Vec<f32>),
}
pub struct Metal {
    device: Device,
    activity: Tensor,
    rows: Arc<Buffer>,
    columns: Arc<Buffer>,
    weights: Arc<Buffer>,
    pipeline: ComputePipeline,
}
struct StepOp<'a> {
    metal: &'a Metal,
    n: usize,
    input: Arc<Buffer>,
    noise: Arc<Buffer>,
    retention: f32,
    recurrence: f32,
    update_gain: f32,
}
impl CustomOp1 for StepOp<'_> {
    fn name(&self) -> &'static str {
        "malecns-csr-recurrence"
    }
    fn cpu_fwd(&self, _: &CpuStorage, _: &Layout) -> candle_core::Result<(CpuStorage, Shape)> {
        candle_core::bail!("Metal operation on CPU")
    }
    fn metal_fwd(
        &self,
        a: &MetalStorage,
        l: &Layout,
    ) -> candle_core::Result<(MetalStorage, Shape)> {
        if !l.is_contiguous()
            || l.start_offset() != 0
            || l.shape().elem_count() != self.n
            || a.dtype() != DType::F32
        {
            candle_core::bail!("Invalid recurrence activity layout")
        }
        let dev = a.device();
        let out = dev.new_buffer(self.n, DType::F32, "brain-state")?;
        let guard = dev.command_encoder()?;
        let encoder = guard.as_ref();
        encoder.set_compute_pipeline_state(&self.metal.pipeline);
        candle_metal_kernels::set_params!(
            encoder,
            (
                self.n as u32,
                self.metal.rows.as_ref(),
                self.metal.columns.as_ref(),
                self.metal.weights.as_ref(),
                a.buffer(),
                self.input.as_ref(),
                self.noise.as_ref(),
                candle_metal_kernels::Output::new(out.as_ref()),
                self.retention,
                self.recurrence,
                self.update_gain
            )
        );
        encoder.dispatch_thread_groups(
            MTLSize {
                width: (self.n * 32).div_ceil(256),
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
impl Compute {
    pub fn new(p: &Profile, cpu: bool) -> Result<Self, String> {
        if cpu {
            return Ok(Self::Cpu(vec![0.; p.manifest.nodes]));
        }
        let device =
            Device::new_metal(0).map_err(|e| format!("Metal controller unavailable: {e}"))?;
        let dev = device.as_metal_device().map_err(|e| e.to_string())?;
        let options = MTLCompileOptions::new();
        #[allow(deprecated)]
        options.setFastMathEnabled(false);
        let library = dev
            .metal_device()
            .new_library_with_source(include_str!("step.metal"), Some(&options))
            .map_err(|e| e.to_string())?;
        let function = library
            .get_function("csr_step", None)
            .map_err(|e| e.to_string())?;
        let pipeline = dev
            .metal_device()
            .new_compute_pipeline_state_with_function(&function)
            .map_err(|e| e.to_string())?;
        let rows = dev
            .new_buffer_with_data(&p.rows)
            .map_err(|e| e.to_string())?;
        let columns = dev
            .new_buffer_with_data(&p.columns)
            .map_err(|e| e.to_string())?;
        let weights = dev
            .new_buffer_with_data(&p.values)
            .map_err(|e| e.to_string())?;
        let activity =
            Tensor::zeros(p.manifest.nodes, DType::F32, &device).map_err(|e| e.to_string())?;
        Ok(Self::Metal(Metal {
            device,
            activity,
            rows,
            columns,
            weights,
            pipeline,
        }))
    }
    pub fn name(&self) -> &'static str {
        match self {
            Self::Metal(_) => crate::profile::BACKEND,
            Self::Cpu(_) => "rust-csr-f32-reference-v1",
        }
    }
    pub fn restore(&mut self, a: &[f32]) -> Result<(), String> {
        match self {
            Self::Cpu(old) => *old = a.to_vec(),
            Self::Metal(m) => {
                let replacement =
                    Tensor::from_slice(a, a.len(), &m.device).map_err(|e| e.to_string())?;
                m.device.synchronize().map_err(|e| e.to_string())?;
                m.activity = replacement;
            }
        }
        Ok(())
    }
    pub fn run(
        &mut self,
        p: &Profile,
        input: &[f32],
        noise: &[Vec<f32>],
    ) -> Result<Vec<f32>, String> {
        let n = p.manifest.nodes;
        let cfg = &p.manifest.dynamics;
        if input.len() != n
            || noise.len() != wire_types::NEURAL_STEPS as usize
            || noise.iter().any(|v| v.len() != n)
        {
            return Err("Recurrence buffer dimensions mismatch".into());
        }
        match self {
            Self::Cpu(a) => {
                let mut out = vec![0.; n];
                for ns in noise {
                    for row in 0..n {
                        let mut sum = 0f32;
                        for j in p.rows[row] as usize..p.rows[row + 1] as usize {
                            sum += p.values[j] * a[p.columns[j] as usize];
                        }
                        let drive = (cfg.recurrence_gain as f32 * sum + input[row]) + ns[row];
                        out[row] = cfg.retention as f32 * a[row]
                            + (1.0_f64 - cfg.retention) as f32 * drive.max(0.).tanh();
                    }
                    std::mem::swap(a, &mut out);
                }
                Ok(a.clone())
            }
            Self::Metal(m) => {
                let dev = m.device.as_metal_device().map_err(|e| e.to_string())?;
                let input = dev.new_buffer_with_data(input).map_err(|e| e.to_string())?;
                let noises: Vec<_> = noise
                    .iter()
                    .map(|v| dev.new_buffer_with_data(v).map_err(|e| e.to_string()))
                    .collect::<Result<_, _>>()?;
                let mut current = m.activity.clone();
                let mut keep_alive = Vec::new();
                for ns in noises {
                    let op = StepOp {
                        metal: m,
                        n,
                        input: input.clone(),
                        noise: ns,
                        retention: cfg.retention as f32,
                        recurrence: cfg.recurrence_gain as f32,
                        update_gain: (1.0 - cfg.retention) as f32,
                    };
                    let next = current.apply_op1_no_bwd(&op).map_err(|e| e.to_string())?;
                    keep_alive.push((current, op));
                    current = next;
                }
                let result = current.to_vec1::<f32>().map_err(|e| e.to_string())?;
                drop(keep_alive);
                m.activity = current;
                Ok(result)
            }
        }
    }
}
