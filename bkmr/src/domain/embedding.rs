use std::any::Any;
// bkmr/src/domain/embedding.rs
use crate::domain::error::{DomainError, DomainResult};
use ndarray::Array1;
use std::io::Cursor;
use tracing::instrument;

/// Core trait for text embedding functionality
/// TypeId check fails because trait objects don’t inherently carry their concrete type’s TypeId
/// unless the trait extends Any, and you manually downcast.
/// calling .type_id() on a dyn Embedder trait object only sees the “Embedder trait object” layer,
/// not the concrete DummyEmbedding underneath
///
/// embedder.as_ref() returns &dyn Embedder.
/// dyn Embedder by default doesn’t extend Any. So .type_id() sees the trait object’s ID, not DummyEmbedding’s.
/// To fix this, you need to extend Any on the trait and implement as_any() to return a reference to self.
/// This way, you can downcast to the concrete type and check its TypeId.
pub trait Embedder: Send + Sync {
    /// Embeds text into a vector of floats
    fn embed(&self, text: &str) -> DomainResult<Option<Vec<f32>>>;
    fn as_any(&self) -> &dyn Any; // for downcasting
}

/// Calculate cosine similarity between two vectors
#[instrument(skip_all)]
pub fn cosine_similarity(vec1: &Array1<f32>, vec2: &Array1<f32>) -> f32 {
    let dot_product = vec1.dot(vec2);
    let magnitude_vec1 = vec1.dot(vec1).sqrt();
    let magnitude_vec2 = vec2.dot(vec2).sqrt();

    if magnitude_vec1 == 0.0 || magnitude_vec2 == 0.0 {
        return 0.0;
    }

    dot_product / (magnitude_vec1 * magnitude_vec2)
}

/// Deserialize bytes into float vector
#[instrument(skip_all)]
pub fn deserialize_embedding(bytes: Vec<u8>) -> Result<Vec<f32>, DomainError> {
    bincode::deserialize(&bytes).map_err(|e| DomainError::DeserializationError(e.to_string()))
}

/// Serialize float vector into bytes
#[instrument(skip_all)]
pub fn serialize_embedding(embedding: Vec<f32>) -> Result<Vec<u8>, DomainError> {
    bincode::serialize(&embedding).map_err(|e| DomainError::SerializationError(e.to_string()))
}

/// Convert byte array to ndarray
#[instrument(skip_all)]
pub fn bytes_to_array(bytes: &[u8]) -> Result<Array1<f32>, DomainError> {
    let mut cursor = Cursor::new(bytes);
    let num_floats = bytes.len() / 4;
    let mut values = Vec::with_capacity(num_floats);

    for _ in 0..num_floats {
        match byteorder::ReadBytesExt::read_f32::<byteorder::LittleEndian>(&mut cursor) {
            Ok(value) => values.push(value),
            Err(e) => return Err(DomainError::IoError(e)),
        }
    }

    Ok(Array1::from(values))
}

/// Convert ndarray to byte array
#[instrument(skip_all)]
pub fn array_to_bytes(array: &Array1<f32>) -> Result<Vec<u8>, DomainError> {
    let mut buffer = Vec::with_capacity(array.len() * 4);

    for &value in array.iter() {
        match byteorder::WriteBytesExt::write_f32::<byteorder::LittleEndian>(&mut buffer, value) {
            Ok(_) => {}
            Err(e) => return Err(DomainError::IoError(e)),
        }
    }

    Ok(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::array;

    const EPSILON: f32 = 1e-6;

    #[test]
    fn test_cosine_similarity() {
        let vec1 = array![1.0, 0.0];
        let vec2 = array![0.0, 1.0];

        // Orthogonal vectors should have similarity 0
        let similarity = cosine_similarity(&vec1, &vec2);
        assert!((similarity - 0.0).abs() < EPSILON);

        // Parallel vectors should have similarity 1
        let vec3 = array![1.0, 1.0];
        let vec4 = array![1.0, 1.0];
        let similarity = cosine_similarity(&vec3, &vec4);
        assert!((similarity - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let original = vec![1.0f32, 2.0, 3.0];

        let bytes = serialize_embedding(original.clone()).unwrap();
        let deserialized = deserialize_embedding(bytes).unwrap();

        assert_eq!(original, deserialized);
    }

    #[test]
    fn test_array_conversion_roundtrip() {
        let original = array![1.0f32, 2.0, 3.0, 4.0];

        let bytes = array_to_bytes(&original).unwrap();
        let reconstructed = bytes_to_array(&bytes).unwrap();

        // Compare each element with epsilon for floating point comparison
        for (a, b) in original.iter().zip(reconstructed.iter()) {
            assert!((a - b).abs() < EPSILON);
        }
    }
}
