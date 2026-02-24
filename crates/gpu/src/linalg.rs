//! GPU-accelerated linear algebra operations for Xenobot.
//!
//! This module provides linear algebra operations optimized for Metal MPS on
//! macOS arm64 silicon, supporting the neural network and embedding computations.

use crate::error::{GpuError, Result};
use crate::metal::MetalDevice;
use crate::mps::MpsMatrixMultiplication;

/// Vector operations on GPU.
#[allow(dead_code)]
pub struct VectorOps {
    device: MetalDevice,
}

impl VectorOps {
    /// Create a new vector operations instance.
    pub fn new(device: MetalDevice) -> Self {
        Self { device }
    }

    /// Compute dot product of two vectors: a · b
    pub fn dot(&self, a: &[f32], b: &[f32]) -> Result<f32> {
        if a.len() != b.len() {
            return Err(GpuError::LinearAlgebra(format!(
                "Vector dimensions mismatch: {} != {}",
                a.len(),
                b.len()
            )));
        }

        // For small vectors, CPU computation is fine
        // For larger vectors, we could use MPS
        let mut result = 0.0;
        for i in 0..a.len() {
            result += a[i] * b[i];
        }
        Ok(result)
    }

    /// Compute vector norm (L2 norm).
    pub fn norm(&self, v: &[f32]) -> Result<f32> {
        let dot = self.dot(v, v)?;
        Ok(dot.sqrt())
    }

    /// Normalize vector to unit length.
    pub fn normalize(&self, v: &[f32]) -> Result<Vec<f32>> {
        let norm = self.norm(v)?;
        if norm == 0.0 {
            return Err(GpuError::LinearAlgebra(
                "Cannot normalize zero vector".to_string(),
            ));
        }

        Ok(v.iter().map(|&x| x / norm).collect())
    }

    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(&self, a: &[f32], b: &[f32]) -> Result<f32> {
        let dot = self.dot(a, b)?;
        let norm_a = self.norm(a)?;
        let norm_b = self.norm(b)?;

        if norm_a == 0.0 || norm_b == 0.0 {
            return Err(GpuError::LinearAlgebra(
                "Cannot compute similarity for zero vectors".to_string(),
            ));
        }

        Ok(dot / (norm_a * norm_b))
    }

    /// Add two vectors: c = a + b
    pub fn add(&self, a: &[f32], b: &[f32]) -> Result<Vec<f32>> {
        if a.len() != b.len() {
            return Err(GpuError::LinearAlgebra(format!(
                "Vector dimensions mismatch: {} != {}",
                a.len(),
                b.len()
            )));
        }

        Ok(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect())
    }

    /// Scale vector: b = alpha * a
    pub fn scale(&self, a: &[f32], alpha: f32) -> Vec<f32> {
        a.iter().map(|&x| x * alpha).collect()
    }
}

/// Matrix operations on GPU.
pub struct MatrixOps {
    device: MetalDevice,
}

impl MatrixOps {
    /// Create a new matrix operations instance.
    pub fn new(device: MetalDevice) -> Self {
        Self { device }
    }

    /// Perform matrix multiplication: C = A × B
    /// A: m × k, B: k × n, C: m × n
    pub fn matmul(&self, a: &[f32], b: &[f32], m: usize, k: usize, n: usize) -> Result<Vec<f32>> {
        if a.len() != m * k {
            return Err(GpuError::LinearAlgebra(format!(
                "Matrix A dimensions mismatch: expected {} elements ({}×{}), got {}",
                m * k,
                m,
                k,
                a.len()
            )));
        }

        if b.len() != k * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Matrix B dimensions mismatch: expected {} elements ({}×{}), got {}",
                k * n,
                k,
                n,
                b.len()
            )));
        }

        let mps_mmul = MpsMatrixMultiplication::new(self.device.clone())?;
        mps_mmul.multiply(a, b, m, k, n, 1.0, 0.0)
    }

    /// Perform matrix-vector multiplication: y = A × x
    /// A: m × n, x: n × 1, y: m × 1
    pub fn matvec(&self, a: &[f32], x: &[f32], m: usize, n: usize) -> Result<Vec<f32>> {
        if a.len() != m * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Matrix A dimensions mismatch: expected {} elements ({}×{}), got {}",
                m * n,
                m,
                n,
                a.len()
            )));
        }

        if x.len() != n {
            return Err(GpuError::LinearAlgebra(format!(
                "Vector x dimensions mismatch: expected {} elements, got {}",
                n,
                x.len()
            )));
        }

        // Treat x as a column vector (n × 1 matrix)
        // For MPS, we need to treat it as a matrix
        let mps_mmul = MpsMatrixMultiplication::new(self.device.clone())?;
        mps_mmul.multiply(a, x, m, n, 1, 1.0, 0.0)
    }

    /// Transpose matrix: B = A^T
    /// A: m × n, B: n × m
    pub fn transpose(&self, a: &[f32], m: usize, n: usize) -> Result<Vec<f32>> {
        if a.len() != m * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Matrix dimensions mismatch: expected {} elements ({}×{}), got {}",
                m * n,
                m,
                n,
                a.len()
            )));
        }

        let mut result = vec![0.0; n * m];
        for i in 0..m {
            for j in 0..n {
                result[j * m + i] = a[i * n + j];
            }
        }

        Ok(result)
    }

    /// Compute matrix addition: C = A + B
    /// A, B, C: m × n
    pub fn add(&self, a: &[f32], b: &[f32], m: usize, n: usize) -> Result<Vec<f32>> {
        if a.len() != m * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Matrix A dimensions mismatch: expected {} elements ({}×{}), got {}",
                m * n,
                m,
                n,
                a.len()
            )));
        }

        if b.len() != m * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Matrix B dimensions mismatch: expected {} elements ({}×{}), got {}",
                m * n,
                m,
                n,
                b.len()
            )));
        }

        Ok(a.iter().zip(b.iter()).map(|(x, y)| x + y).collect())
    }

    /// Compute matrix scaling: B = alpha * A
    pub fn scale(&self, a: &[f32], alpha: f32) -> Vec<f32> {
        a.iter().map(|&x| x * alpha).collect()
    }

    /// Compute Frobenius norm of matrix.
    pub fn frobenius_norm(&self, a: &[f32]) -> f32 {
        let mut sum = 0.0;
        for &x in a {
            sum += x * x;
        }
        sum.sqrt()
    }

    /// Compute trace of square matrix.
    pub fn trace(&self, a: &[f32], n: usize) -> Result<f32> {
        if a.len() != n * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Matrix must be square: expected {} elements, got {}",
                n * n,
                a.len()
            )));
        }

        let mut trace = 0.0;
        for i in 0..n {
            trace += a[i * n + i];
        }

        Ok(trace)
    }
}

/// Batch matrix operations for handling multiple matrices efficiently.
pub struct BatchMatrixOps {
    device: MetalDevice,
}

impl BatchMatrixOps {
    /// Create a new batch matrix operations instance.
    pub fn new(device: MetalDevice) -> Self {
        Self { device }
    }

    /// Perform batched matrix multiplication.
    /// For i in 0..batch_size: C[i] = A[i] × B[i]
    /// A[i]: m × k, B[i]: k × n, C[i]: m × n
    pub fn batched_matmul(
        &self,
        a: &[f32],
        b: &[f32],
        batch_size: usize,
        m: usize,
        k: usize,
        n: usize,
    ) -> Result<Vec<f32>> {
        if a.len() != batch_size * m * k {
            return Err(GpuError::LinearAlgebra(format!(
                "Batched matrix A dimensions mismatch: expected {} elements ({}×{}×{}), got {}",
                batch_size * m * k,
                batch_size,
                m,
                k,
                a.len()
            )));
        }

        if b.len() != batch_size * k * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Batched matrix B dimensions mismatch: expected {} elements ({}×{}×{}), got {}",
                batch_size * k * n,
                batch_size,
                k,
                n,
                b.len()
            )));
        }

        // For now, perform sequential multiplication
        // In a real implementation, this would use MPS batched operations
        let mut result = vec![0.0; batch_size * m * n];
        let mps_mmul = MpsMatrixMultiplication::new(self.device.clone())?;

        for i in 0..batch_size {
            let a_offset = i * m * k;
            let b_offset = i * k * n;
            let c_offset = i * m * n;

            let c_slice = mps_mmul.multiply(
                &a[a_offset..a_offset + m * k],
                &b[b_offset..b_offset + k * n],
                m,
                k,
                n,
                1.0,
                0.0,
            )?;

            result[c_offset..c_offset + m * n].copy_from_slice(&c_slice);
        }

        Ok(result)
    }

    /// Perform batched matrix-vector multiplication.
    /// For i in 0..batch_size: y[i] = A[i] × x[i]
    pub fn batched_matvec(
        &self,
        a: &[f32],
        x: &[f32],
        batch_size: usize,
        m: usize,
        n: usize,
    ) -> Result<Vec<f32>> {
        if a.len() != batch_size * m * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Batched matrix A dimensions mismatch: expected {} elements ({}×{}×{}), got {}",
                batch_size * m * n,
                batch_size,
                m,
                n,
                a.len()
            )));
        }

        if x.len() != batch_size * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Batched vector x dimensions mismatch: expected {} elements ({}×{}), got {}",
                batch_size * n,
                batch_size,
                n,
                x.len()
            )));
        }

        let mut result = vec![0.0; batch_size * m];
        let mps_mmul = MpsMatrixMultiplication::new(self.device.clone())?;

        for i in 0..batch_size {
            let a_offset = i * m * n;
            let x_offset = i * n;
            let y_offset = i * m;

            let y_slice = mps_mmul.multiply(
                &a[a_offset..a_offset + m * n],
                &x[x_offset..x_offset + n],
                m,
                n,
                1, // Treat x as n × 1 matrix
                1.0,
                0.0,
            )?;

            result[y_offset..y_offset + m].copy_from_slice(&y_slice);
        }

        Ok(result)
    }
}

/// Utility functions for linear algebra.
pub mod utils {
    use super::*;

    /// Convert vector to diagonal matrix.
    pub fn diag(v: &[f32], n: usize) -> Result<Vec<f32>> {
        if v.len() != n {
            return Err(GpuError::LinearAlgebra(format!(
                "Vector length must equal matrix dimension: expected {}, got {}",
                n,
                v.len()
            )));
        }

        let mut result = vec![0.0; n * n];
        for i in 0..n {
            result[i * n + i] = v[i];
        }

        Ok(result)
    }

    /// Extract diagonal from square matrix.
    pub fn extract_diag(a: &[f32], n: usize) -> Result<Vec<f32>> {
        if a.len() != n * n {
            return Err(GpuError::LinearAlgebra(format!(
                "Matrix must be square: expected {} elements, got {}",
                n * n,
                a.len()
            )));
        }

        let mut result = vec![0.0; n];
        for i in 0..n {
            result[i] = a[i * n + i];
        }

        Ok(result)
    }

    /// Create identity matrix of size n × n.
    pub fn identity(n: usize) -> Vec<f32> {
        let mut result = vec![0.0; n * n];
        for i in 0..n {
            result[i * n + i] = 1.0;
        }
        result
    }

    /// Create matrix of ones.
    pub fn ones(m: usize, n: usize) -> Vec<f32> {
        vec![1.0; m * n]
    }

    /// Create matrix of zeros.
    pub fn zeros(m: usize, n: usize) -> Vec<f32> {
        vec![0.0; m * n]
    }
}
