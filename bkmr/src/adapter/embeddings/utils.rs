use anyhow::{anyhow, Result};
use bincode::{deserialize, serialize};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use ndarray::Array1;
use std::io::Cursor;
use tracing::instrument;

/// Calculate cosine similarity between two vectors
#[instrument]
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
#[instrument]
pub fn deserialize_embedding(bytes: Vec<u8>) -> Result<Vec<f32>> {
    deserialize(&bytes).map_err(|e| anyhow!("Failed to deserialize embedding: {}", e))
}

/// Serialize float vector into bytes
#[instrument]
pub fn serialize_embedding(embedding: Vec<f32>) -> Result<Vec<u8>> {
    serialize(&embedding).map_err(|e| anyhow!("Failed to serialize embedding: {}", e))
}

/// Convert byte array to ndarray
#[instrument]
pub fn bytes_to_array(bytes: &[u8]) -> Array1<f32> {
    let mut cursor = Cursor::new(bytes);
    let num_floats = bytes.len() / 4;
    let mut values = Vec::with_capacity(num_floats);

    for _ in 0..num_floats {
        values.push(cursor.read_f32::<LittleEndian>().unwrap());
    }

    Array1::from(values)
}

/// Convert ndarray to byte array
#[instrument]
pub fn array_to_bytes(array: &Array1<f32>) -> Vec<u8> {
    let mut buffer = Vec::with_capacity(array.len() * 4);
    for &value in array.iter() {
        buffer.write_f32::<LittleEndian>(value).unwrap();
    }
    buffer
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_cmp::approx_eq;
    use ndarray::array;
    use rstest::*;

    const EPSILON: f32 = 1e-6;

    #[rstest]
    fn test_cosine_similarity() {
        let vec1 = array![1.0, 0.0];
        let vec2 = array![0.0, 1.0];
        assert!(approx_eq!(
            f32,
            cosine_similarity(&vec1, &vec2),
            0.0,
            epsilon = EPSILON
        ));

        let vec3 = array![1.0, 1.0];
        let vec4 = array![1.0, 1.0];
        assert!(approx_eq!(
            f32,
            cosine_similarity(&vec3, &vec4),
            1.0,
            epsilon = EPSILON
        ));
    }

    #[rstest]
    fn test_serialization_roundtrip() {
        let original = vec![1.0f32, 2.0, 3.0];
        let bytes = serialize_embedding(original.clone()).unwrap();
        let deserialized = deserialize_embedding(bytes).unwrap();
        assert_eq!(original, deserialized);
    }

    #[rstest]
    fn test_array_conversion_roundtrip() {
        let original = array![1.0f32, 2.0, 3.0, 4.0];
        let bytes = array_to_bytes(&original);
        let reconstructed = bytes_to_array(&bytes);
        assert_eq!(original, reconstructed);
    }
}
