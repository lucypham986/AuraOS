//! Sprint 3 — AI Micro-Runtime: System-Service-level AI inference engine.
//!
//! Runs in Ring 3 user-space (or as a privileged kernel service during early
//! boot) and exposes a language-model and tensor-inference API to other
//! subsystems via IPC.
//!
//! **Current implementation:** architecture stub that establishes the public
//! API surface, validates input contracts, and logs all inference requests.
//! Full acceleration backends (ONNX Runtime, `llama.cpp` C-FFI, Vulkan
//! compute shaders, Qualcomm NPU API) are wired in through the
//! `AccelerationBackend` trait as Sprint 4 drivers become available.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;
use spin::Mutex;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that the AI runtime may return.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiError {
    /// The requested model is not loaded or unknown.
    ModelNotFound,
    /// Input tensor dimensions or dtype are incompatible with the model.
    InvalidInput,
    /// Output buffer is too small to hold the inference result.
    OutputTooSmall,
    /// The runtime has not been initialised (`init_ai_runtime` not called).
    NotInitialized,
    /// The acceleration backend reported a hardware fault.
    BackendError,
}

// ---------------------------------------------------------------------------
// Acceleration backend abstraction
// ---------------------------------------------------------------------------

/// Hardware acceleration tier available for inference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccelBackend {
    /// Software fallback — pure Rust scalar loops.
    Cpu,
    /// Vulkan compute shaders (cross-vendor GPU path).
    Vulkan,
    /// CUDA / NVLink (Nvidia).
    Cuda,
    /// ROCm (AMD).
    Rocm,
    /// Metal (Apple Silicon).
    Metal,
    /// DirectML (Windows/ARM64 with NPU).
    DirectMl,
    /// Qualcomm Hexagon DSP / NPU API.
    QualcommNpu,
}

impl AccelBackend {
    /// Human-readable name used in log output.
    pub const fn name(self) -> &'static str {
        match self {
            AccelBackend::Cpu => "CPU (scalar)",
            AccelBackend::Vulkan => "Vulkan Compute",
            AccelBackend::Cuda => "CUDA/NVLink",
            AccelBackend::Rocm => "ROCm",
            AccelBackend::Metal => "Metal",
            AccelBackend::DirectMl => "DirectML",
            AccelBackend::QualcommNpu => "Qualcomm NPU",
        }
    }
}

// ---------------------------------------------------------------------------
// Model descriptor
// ---------------------------------------------------------------------------

/// Lightweight descriptor for a loaded model.
#[derive(Debug, Clone)]
pub struct ModelInfo {
    /// Unique model identifier (e.g. `"llama3-8b"` or `"mobilevit-s"`).
    pub name: String,
    /// Number of model parameters (informational).
    pub param_count: u64,
    /// Memory footprint of the loaded weights in bytes.
    pub weight_bytes: usize,
    /// Preferred acceleration backend for this model.
    pub backend: AccelBackend,
}

// ---------------------------------------------------------------------------
// AI runtime
// ---------------------------------------------------------------------------

/// Kernel AI Micro-Runtime.
///
/// Manages model loading, KV-cache, and dispatches inference requests to the
/// appropriate acceleration backend.  Backed by a Zero-Copy Unified Memory
/// Bridge so that weight tensors are accessible from both CPU and GPU without
/// explicit copies (see `zero_copy` module).
pub struct AiRuntime {
    initialized: bool,
    loaded_models: Vec<ModelInfo>,
    active_backend: AccelBackend,
}

impl AiRuntime {
    pub const fn new() -> Self {
        Self {
            initialized: false,
            loaded_models: Vec::new(),
            active_backend: AccelBackend::Cpu,
        }
    }

    /// Probe available hardware and choose the best acceleration backend.
    pub fn init(&mut self) {
        // TODO: probe CPUID / device-tree / ACPI for GPU/NPU presence.
        // For now default to CPU scalar fallback.
        self.active_backend = AccelBackend::Cpu;
        self.initialized = true;
        log::info!(
            "AI Micro-Runtime initialized. Active backend: {}.",
            self.active_backend.name()
        );
    }

    /// Register a model with the runtime.  In a full implementation this
    /// would DMA-map the weights into the Zero-Copy memory bridge.
    pub fn load_model(&mut self, info: ModelInfo) -> Result<(), AiError> {
        if !self.initialized {
            return Err(AiError::NotInitialized);
        }
        log::info!(
            "AI: loading model '{}' ({} params, {} KiB weights, backend: {})",
            info.name,
            info.param_count,
            info.weight_bytes / 1024,
            info.backend.name(),
        );
        self.loaded_models.push(info);
        Ok(())
    }

    /// Run inference on a named model.
    ///
    /// * `model_name` — must match a previously loaded model.
    /// * `input`      — raw input tensor bytes (format defined per model).
    /// * `output`     — caller-provided buffer that receives the output tensor.
    ///
    /// Returns the number of bytes written into `output`.
    pub fn infer(
        &self,
        model_name: &str,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, AiError> {
        if !self.initialized {
            return Err(AiError::NotInitialized);
        }
        if input.is_empty() {
            return Err(AiError::InvalidInput);
        }

        let model = self
            .loaded_models
            .iter()
            .find(|m| m.name == model_name)
            .ok_or(AiError::ModelNotFound)?;

        log::debug!(
            "AI: infer '{}' via {} — {} input bytes",
            model.name,
            model.backend.name(),
            input.len()
        );

        // Stub: echo the first min(input.len(), output.len()) bytes back as
        // a placeholder inference result.
        let copy_len = input.len().min(output.len());
        if copy_len == 0 {
            return Err(AiError::OutputTooSmall);
        }
        output[..copy_len].copy_from_slice(&input[..copy_len]);
        Ok(copy_len)
    }

    /// Return information about all currently loaded models.
    pub fn list_models(&self) -> &[ModelInfo] {
        &self.loaded_models
    }
}

/// Global AI runtime instance.
pub static AI_RUNTIME: Mutex<AiRuntime> = Mutex::new(AiRuntime::new());

/// Initialise the AI Micro-Runtime and probe acceleration hardware.
pub fn init_ai_runtime() {
    AI_RUNTIME.lock().init();
}
