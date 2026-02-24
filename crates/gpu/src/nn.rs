//! Neural network inference on Metal GPU for Xenobot.
//!
//! This module provides GPU-accelerated neural network operations using Apple's
//! Metal Performance Shaders (MPS) framework, optimized for macOS arm64 silicon.
//!
//! Primary use cases:
//! - Embedding generation for RAG (Retrieval Augmented Generation)
//! - Text classification and similarity scoring
//! - Matrix operations for chat analysis

use crate::error::{GpuError, Result};
use crate::metal::MetalDevice;
use crate::mps::{MpsGraph, MpsMatrixMultiplication};

/// Neural network layer types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayerType {
    /// Fully connected (dense) layer.
    Dense,
    /// Embedding layer (lookup table).
    Embedding,
    /// Layer normalization.
    LayerNorm,
    /// Attention mechanism for transformers.
    Attention,
}

/// Neural network layer configuration.
#[derive(Debug, Clone)]
pub struct LayerConfig {
    /// Layer type.
    pub layer_type: LayerType,
    /// Input dimension.
    pub input_dim: usize,
    /// Output dimension.
    pub output_dim: usize,
    /// Whether the layer has bias.
    pub has_bias: bool,
    /// Activation function (if any).
    pub activation: Option<Activation>,
}

/// Activation functions.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Activation {
    /// ReLU activation.
    Relu,
    /// GELU activation.
    Gelu,
    /// Sigmoid activation.
    Sigmoid,
    /// Tanh activation.
    Tanh,
    /// No activation (identity).
    Linear,
}

/// Neural network model for GPU inference.
pub struct NeuralNetwork {
    device: MetalDevice,
    #[allow(dead_code)]
    mps_graph: MpsGraph,
    layers: Vec<LayerConfig>,
    weights: Vec<Vec<f32>>,
    biases: Vec<Option<Vec<f32>>>,
}

impl NeuralNetwork {
    /// Create a new neural network with the given configuration.
    pub fn new(device: MetalDevice, layers: Vec<LayerConfig>) -> Result<Self> {
        // Validate layer configurations
        for (i, layer) in layers.iter().enumerate() {
            if layer.input_dim == 0 || layer.output_dim == 0 {
                return Err(GpuError::NeuralNetwork(format!(
                    "Layer {} has zero dimension: input={}, output={}",
                    i, layer.input_dim, layer.output_dim
                )));
            }
        }

        // Initialize MPS graph
        let mps_graph = MpsGraph::new(device.clone());

        // Initialize weights and biases with zeros (to be loaded from trained model)
        let mut weights = Vec::with_capacity(layers.len());
        let mut biases = Vec::with_capacity(layers.len());

        for layer in &layers {
            let weight_size = layer.input_dim * layer.output_dim;
            weights.push(vec![0.0; weight_size]);

            if layer.has_bias {
                biases.push(Some(vec![0.0; layer.output_dim]));
            } else {
                biases.push(None);
            }
        }

        Ok(Self {
            device,
            mps_graph,
            layers,
            weights,
            biases,
        })
    }

    /// Load model weights from serialized data.
    pub fn load_weights(
        &mut self,
        weights_data: &[Vec<f32>],
        biases_data: &[Option<Vec<f32>>],
    ) -> Result<()> {
        if weights_data.len() != self.layers.len() {
            return Err(GpuError::NeuralNetwork(format!(
                "Mismatched weights count: expected {}, got {}",
                self.layers.len(),
                weights_data.len()
            )));
        }

        if biases_data.len() != self.layers.len() {
            return Err(GpuError::NeuralNetwork(format!(
                "Mismatched biases count: expected {}, got {}",
                self.layers.len(),
                biases_data.len()
            )));
        }

        for (i, (layer, weight_vec)) in self.layers.iter().zip(weights_data.iter()).enumerate() {
            let expected_size = layer.input_dim * layer.output_dim;
            if weight_vec.len() != expected_size {
                return Err(GpuError::NeuralNetwork(format!(
                    "Layer {} weight size mismatch: expected {}, got {}",
                    i,
                    expected_size,
                    weight_vec.len()
                )));
            }
        }

        self.weights.clone_from_slice(weights_data);
        self.biases.clone_from_slice(biases_data);

        Ok(())
    }

    /// Perform forward pass on input data.
    pub fn forward(&self, input: &[f32]) -> Result<Vec<f32>> {
        // Validate input size
        if let Some(first_layer) = self.layers.first() {
            if input.len() != first_layer.input_dim {
                return Err(GpuError::NeuralNetwork(format!(
                    "Input size mismatch: expected {}, got {}",
                    first_layer.input_dim,
                    input.len()
                )));
            }
        } else {
            return Err(GpuError::NeuralNetwork("No layers in network".to_string()));
        }

        let mut current = input.to_vec();
        let mps_mmul = MpsMatrixMultiplication::new(self.device.clone())?;

        for (_i, ((layer, weights), bias)) in self
            .layers
            .iter()
            .zip(&self.weights)
            .zip(&self.biases)
            .enumerate()
        {
            // Matrix multiplication: output = input * weights
            let batch_size = 1; // Single sample inference
            let output = mps_mmul.multiply(
                &current,
                weights,
                batch_size,
                layer.input_dim,
                layer.output_dim,
                1.0, // alpha
                0.0, // beta
            )?;

            // Add bias if present
            let mut result = if let Some(bias_vec) = bias {
                output
                    .iter()
                    .zip(bias_vec.iter())
                    .map(|(o, b)| o + b)
                    .collect()
            } else {
                output
            };

            // Apply activation function
            if let Some(activation) = layer.activation {
                result = Self::apply_activation(&result, activation);
            }

            current = result;
        }

        Ok(current)
    }

    /// Apply activation function to tensor.
    fn apply_activation(tensor: &[f32], activation: Activation) -> Vec<f32> {
        match activation {
            Activation::Relu => tensor.iter().map(|&x| x.max(0.0)).collect(),
            Activation::Gelu => {
                // Approximate GELU: x * 0.5 * (1.0 + tanh(sqrt(2.0/π) * (x + 0.044715 * x^3)))
                const SQRT_2_OVER_PI: f32 = 0.7978845608;
                const GELU_COEF: f32 = 0.044715;
                tensor
                    .iter()
                    .map(|&x| {
                        0.5 * x * (1.0 + (SQRT_2_OVER_PI * (x + GELU_COEF * x * x * x)).tanh())
                    })
                    .collect()
            }
            Activation::Sigmoid => tensor.iter().map(|&x| 1.0 / (1.0 + (-x).exp())).collect(),
            Activation::Tanh => tensor.iter().map(|&x| x.tanh()).collect(),
            Activation::Linear => tensor.to_vec(),
        }
    }

    /// Get embedding for input (specialized for embedding layers).
    pub fn embed(&self, input_ids: &[u32], embedding_layer_index: usize) -> Result<Vec<f32>> {
        if embedding_layer_index >= self.layers.len() {
            return Err(GpuError::NeuralNetwork(format!(
                "Invalid embedding layer index: {} (max {})",
                embedding_layer_index,
                self.layers.len() - 1
            )));
        }

        let layer = &self.layers[embedding_layer_index];
        if layer.layer_type != LayerType::Embedding {
            return Err(GpuError::NeuralNetwork(format!(
                "Layer {} is not an embedding layer",
                embedding_layer_index
            )));
        }

        // Embedding lookup
        let embedding_dim = layer.output_dim;
        let vocab_size = layer.input_dim;
        let mut embeddings = Vec::with_capacity(input_ids.len() * embedding_dim);

        for &id in input_ids {
            if id >= vocab_size as u32 {
                return Err(GpuError::NeuralNetwork(format!(
                    "Token ID {} exceeds vocab size {}",
                    id, vocab_size
                )));
            }

            let start = id as usize * embedding_dim;
            let end = start + embedding_dim;
            if end > self.weights[embedding_layer_index].len() {
                return Err(GpuError::NeuralNetwork(
                    "Weight matrix size mismatch for embedding".to_string(),
                ));
            }

            embeddings.extend_from_slice(&self.weights[embedding_layer_index][start..end]);
        }

        Ok(embeddings)
    }

    /// Get the device used by this neural network.
    pub fn device(&self) -> &MetalDevice {
        &self.device
    }

    /// Get the number of layers in the network.
    pub fn num_layers(&self) -> usize {
        self.layers.len()
    }
}

/// Builder for creating neural network configurations.
pub struct NeuralNetworkBuilder {
    layers: Vec<LayerConfig>,
}

impl NeuralNetworkBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    /// Add a dense layer.
    pub fn dense(
        mut self,
        input_dim: usize,
        output_dim: usize,
        has_bias: bool,
        activation: Option<Activation>,
    ) -> Self {
        self.layers.push(LayerConfig {
            layer_type: LayerType::Dense,
            input_dim,
            output_dim,
            has_bias,
            activation,
        });
        self
    }

    /// Add an embedding layer.
    pub fn embedding(mut self, vocab_size: usize, embedding_dim: usize) -> Self {
        self.layers.push(LayerConfig {
            layer_type: LayerType::Embedding,
            input_dim: vocab_size,
            output_dim: embedding_dim,
            has_bias: false,
            activation: None,
        });
        self
    }

    /// Build the neural network with the given device.
    pub fn build(self, device: MetalDevice) -> Result<NeuralNetwork> {
        NeuralNetwork::new(device, self.layers)
    }
}

impl Default for NeuralNetworkBuilder {
    fn default() -> Self {
        Self::new()
    }
}
