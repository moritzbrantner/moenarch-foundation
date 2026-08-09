#![doc = include_str!("../README.md")]

pub mod surface;
use std::collections::BTreeMap;

use media_core::{DetectError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

fn invalid_argument(message: impl Into<String>) -> DetectError {
    DetectError::InvalidArgument(message.into())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Checked tensor dimensions with a non-zero rank and non-zero extents.
pub struct TensorShape {
    dims: Vec<usize>,
}

impl TensorShape {
    /// Creates a tensor shape after validating rank, extents, and element count.
    pub fn new(dims: impl Into<Vec<usize>>) -> Result<Self> {
        let shape = Self { dims: dims.into() };
        shape.validate()?;
        Ok(shape)
    }

    /// Borrows the dimensions in storage order.
    pub fn dimensions(&self) -> &[usize] {
        &self.dims
    }

    /// Returns the number of dimensions.
    pub fn rank(&self) -> usize {
        self.dims.len()
    }

    /// Multiplies all dimensions and fails if the count overflows `usize`.
    pub fn element_count(&self) -> Result<usize> {
        self.dims.iter().try_fold(1_usize, |count, dimension| {
            count
                .checked_mul(*dimension)
                .ok_or_else(|| invalid_argument("tensor shape element count overflowed usize"))
        })
    }

    /// Builds a new shape with the same element count as this shape.
    pub fn reshape(&self, dims: impl Into<Vec<usize>>) -> Result<Self> {
        let reshaped = Self::new(dims)?;
        if reshaped.element_count()? != self.element_count()? {
            return Err(invalid_argument(format!(
                "cannot reshape tensor with {} elements into {} elements",
                self.element_count()?,
                reshaped.element_count()?
            )));
        }
        Ok(reshaped)
    }

    fn validate(&self) -> Result<()> {
        if self.dims.is_empty() {
            return Err(invalid_argument(
                "tensor shape must have at least one dimension",
            ));
        }
        if self.dims.contains(&0) {
            return Err(invalid_argument(
                "tensor shape dimensions must be greater than zero",
            ));
        }
        let _ = self.element_count()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Owned finite `f32` tensor values plus optional JSON metadata.
pub struct F32Tensor {
    shape: TensorShape,
    values: Vec<f32>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    metadata: BTreeMap<String, Value>,
}

impl F32Tensor {
    /// Creates an owned tensor when shape and finite-value validation pass.
    pub fn new(shape: TensorShape, values: Vec<f32>) -> Result<Self> {
        let tensor = Self {
            shape,
            values,
            metadata: BTreeMap::new(),
        };
        tensor.validate()?;
        Ok(tensor)
    }

    /// Creates an owned tensor from raw dimensions and values.
    pub fn from_dims(dims: impl Into<Vec<usize>>, values: Vec<f32>) -> Result<Self> {
        Self::new(TensorShape::new(dims)?, values)
    }

    /// Borrows the checked tensor shape.
    pub fn shape(&self) -> &TensorShape {
        &self.shape
    }

    /// Borrows the contiguous tensor values.
    pub fn values(&self) -> &[f32] {
        &self.values
    }

    /// Consumes the tensor and returns its contiguous values.
    pub fn into_values(self) -> Vec<f32> {
        self.values
    }

    /// Borrows optional transport metadata attached to the tensor.
    pub fn metadata(&self) -> &BTreeMap<String, Value> {
        &self.metadata
    }

    /// Attaches one metadata entry and returns the updated tensor.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Inserts or replaces one metadata entry in place.
    pub fn set_metadata(&mut self, key: impl Into<String>, value: impl Into<Value>) -> &mut Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Changes only the shape metadata after verifying the element count is unchanged.
    pub fn reshape(mut self, dims: impl Into<Vec<usize>>) -> Result<Self> {
        self.shape = self.shape.reshape(dims)?;
        Ok(self)
    }

    /// Borrows this tensor as a validated view that shares the value slice.
    pub fn as_view(&self) -> F32TensorView<'_> {
        F32TensorView {
            shape: self.shape.clone(),
            values: &self.values,
            metadata: self.metadata.clone(),
        }
    }

    /// Verifies shape/value count agreement and rejects non-finite values.
    pub fn validate(&self) -> Result<()> {
        let expected = self.shape.element_count()?;
        if expected != self.values.len() {
            return Err(invalid_argument(format!(
                "tensor shape expects {expected} elements but tensor has {}",
                self.values.len()
            )));
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("tensor values must be finite"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
/// Borrowed finite `f32` tensor values with owned shape and metadata.
pub struct F32TensorView<'a> {
    shape: TensorShape,
    values: &'a [f32],
    metadata: BTreeMap<String, Value>,
}

impl<'a> F32TensorView<'a> {
    /// Creates a borrowed tensor view when shape and values are compatible.
    pub fn new(shape: TensorShape, values: &'a [f32]) -> Result<Self> {
        let view = Self {
            shape,
            values,
            metadata: BTreeMap::new(),
        };
        view.validate()?;
        Ok(view)
    }

    /// Creates a borrowed tensor view from raw dimensions and values.
    pub fn from_dims(dims: impl Into<Vec<usize>>, values: &'a [f32]) -> Result<Self> {
        Self::new(TensorShape::new(dims)?, values)
    }

    /// Borrows the checked tensor shape.
    pub fn shape(&self) -> &TensorShape {
        &self.shape
    }

    /// Borrows the underlying contiguous value slice.
    pub fn values(&self) -> &'a [f32] {
        self.values
    }

    /// Borrows optional transport metadata attached to the view.
    pub fn metadata(&self) -> &BTreeMap<String, Value> {
        &self.metadata
    }

    /// Attaches one metadata entry and returns the updated view.
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<Value>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }

    /// Changes only the shape metadata after verifying the element count is unchanged.
    pub fn reshape(mut self, dims: impl Into<Vec<usize>>) -> Result<Self> {
        self.shape = self.shape.reshape(dims)?;
        Ok(self)
    }

    /// Copies the borrowed values into an owned tensor while preserving metadata.
    pub fn into_owned(self) -> Result<F32Tensor> {
        let mut tensor = F32Tensor::new(self.shape, self.values.to_vec())?;
        tensor.metadata = self.metadata;
        Ok(tensor)
    }

    /// Verifies shape/value count agreement and rejects non-finite values.
    pub fn validate(&self) -> Result<()> {
        let expected = self.shape.element_count()?;
        if expected != self.values.len() {
            return Err(invalid_argument(format!(
                "tensor shape expects {expected} elements but tensor view has {}",
                self.values.len()
            )));
        }
        if self.values.iter().any(|value| !value.is_finite()) {
            return Err(invalid_argument("tensor values must be finite"));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_or_zero_dimension_shapes() {
        assert!(TensorShape::new(Vec::<usize>::new()).is_err());
        assert!(TensorShape::new([1, 0, 2]).is_err());
    }

    #[test]
    fn rejects_overflowing_shape_element_counts() {
        assert!(TensorShape::new([usize::MAX, 2]).is_err());
    }

    #[test]
    fn rejects_wrong_element_count() {
        let error = F32Tensor::from_dims([2, 2], vec![0.0; 3]).unwrap_err();
        assert!(matches!(error, DetectError::InvalidArgument(_)));
    }

    #[test]
    fn rejects_non_finite_values() {
        let error = F32Tensor::from_dims([1, 2], vec![0.0, f32::NAN]).unwrap_err();
        assert!(matches!(error, DetectError::InvalidArgument(_)));
    }

    #[test]
    fn reshapes_when_element_counts_match() {
        let tensor = F32Tensor::from_dims([1, 4], vec![0.0; 4]).unwrap();
        let reshaped = tensor.reshape([2, 2]).unwrap();
        assert_eq!(reshaped.shape().dimensions(), &[2, 2]);
        assert_eq!(reshaped.values(), &[0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn reshape_rejects_changed_element_count() {
        let tensor = F32Tensor::from_dims([1, 4], vec![0.0; 4]).unwrap();
        assert!(tensor.reshape([3, 2]).is_err());
    }

    #[test]
    fn view_round_trips_into_owned_tensor() {
        let view = F32TensorView::from_dims([1, 1, 2], &[0.25, 0.75]).unwrap();
        let owned = view.into_owned().unwrap();
        assert_eq!(owned.values(), &[0.25, 0.75]);
    }
}
