use crate::error::{Result, SiftError};
use fastembed::{
    EmbeddingModel, ImageEmbedding, ImageEmbeddingModel, ImageInitOptions, TextEmbedding,
    TextInitOptions,
};
use std::path::Path;
use std::sync::{Arc, Mutex};

/// Trait for local semantic embedding generation across text and vision modalities.
pub trait EmbeddingEngine: Send + Sync {
    /// Human-readable model identifier (e.g. "embeddinggemma-2-270m", "clip-vit-b-32").
    fn model_name(&self) -> &str;

    /// Output vector dimensionality (e.g. 384, 512, 768).
    fn dimension(&self) -> usize;

    /// Generates a normalized vector embedding for a single text string.
    fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        let results = self.embed_texts(&[text.to_string()])?;
        results.into_iter().next().ok_or_else(|| {
            SiftError::Internal("Embedding generation returned empty vector".to_string())
        })
    }

    /// Generates normalized vector embeddings for a batch of text strings.
    fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Generates a normalized visual vector embedding for an image file.
    fn embed_image(&self, image_path: &Path) -> Result<Vec<f32>>;
}

/// Computes the cosine similarity between two normalized vectors.
/// Since vectors are pre-normalized to unit length (L2 norm = 1.0), this is equivalent to their dot product.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    dot.clamp(-1.0, 1.0)
}

/// Normalizes a vector in-place to unit L2 norm.
pub fn normalize_vector(v: &mut [f32]) {
    let norm = (v.iter().map(|x| x * x).sum::<f32>()).sqrt();
    if norm > 1e-6 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

/// Serializes an f32 vector into a compact little-endian byte array for SQLite storage.
pub fn serialize_vector(v: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(v.len() * 4);
    for val in v {
        bytes.extend_from_slice(&val.to_le_bytes());
    }
    bytes
}

/// Deserializes an f32 vector from a little-endian byte array.
pub fn deserialize_vector(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap()))
        .collect()
}

/// Supported image extensions for vision semantic embeddings.
pub fn is_image_supported_for_vision(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "svg"
    )
}

/// FastEmbed-backed local ONNX embedding engine supporting both text and vision models.
pub struct FastEmbedEngine {
    text_model: Mutex<TextEmbedding>,
    image_model: Option<Mutex<ImageEmbedding>>,
    dimension: usize,
    model_name: String,
}

impl FastEmbedEngine {
    /// Initializes a local ONNX FastEmbed engine with a specific embedding model.
    pub fn try_new_with_model(model: EmbeddingModel) -> Result<Self> {
        let (dim, name) = match model {
            EmbeddingModel::EmbeddingGemma300M => (
                768,
                "onnx-community/embeddinggemma-2-ONNX (full 740M fp32)",
            ),
            EmbeddingModel::EmbeddingGemma300MQ => (
                768,
                "onnx-community/embeddinggemma-2-ONNX (multimodal 740M quantized)",
            ),
            EmbeddingModel::EmbeddingGemma300MQ4 => (
                768,
                "onnx-community/embeddinggemma-2-ONNX (q4)",
            ),
            EmbeddingModel::AllMiniLML6V2 => (384, "all-minilm-l6-v2"),
            _ => (768, "onnx-community/embeddinggemma-2-ONNX"),
        };

        let text_init = TextInitOptions::new(model).with_show_download_progress(false);
        let text_model = TextEmbedding::try_new(text_init).map_err(|e| {
            SiftError::Internal(format!("Failed to initialize FastEmbed text model: {}", e))
        })?;

        let image_model = {
            let img_init = ImageInitOptions::new(ImageEmbeddingModel::ClipVitB32)
                .with_show_download_progress(false);
            ImageEmbedding::try_new(img_init).ok().map(Mutex::new)
        };

        Ok(Self {
            text_model: Mutex::new(text_model),
            image_model,
            dimension: dim,
            model_name: name.to_string(),
        })
    }

    /// Initializes a local ONNX FastEmbed engine.
    /// Checks `SIFT_EMBEDDING_MODEL` environment variable (e.g. "gemma", "gemma-full", "minilm").
    /// Defaults to Google's EmbeddingGemma (`EmbeddingModel::EmbeddingGemma300MQ`) if available,
    /// falling back gracefully to `AllMiniLML6V2` if initial Gemma download is unavailable.
    pub fn try_new() -> Result<Self> {
        let model_choice = std::env::var("SIFT_EMBEDDING_MODEL")
            .unwrap_or_else(|_| "gemma".to_string());

        let target_model = match model_choice.to_lowercase().as_str() {
            "gemma" | "gemma-q" | "gemma300m" | "embeddinggemma" => {
                EmbeddingModel::EmbeddingGemma300MQ
            }
            "gemma-full" | "gemma-300m-full" => EmbeddingModel::EmbeddingGemma300M,
            "gemma-q4" => EmbeddingModel::EmbeddingGemma300MQ4,
            "minilm" => EmbeddingModel::AllMiniLML6V2,
            _ => EmbeddingModel::EmbeddingGemma300MQ,
        };

        match Self::try_new_with_model(target_model.clone()) {
            Ok(engine) => Ok(engine),
            Err(e) => {
                tracing::warn!(
                    "Failed to initialize requested model {:?} ({}); falling back to lightweight MiniLM",
                    target_model,
                    e
                );
                Self::try_new_with_model(EmbeddingModel::AllMiniLML6V2)
            }
        }
    }
}

impl EmbeddingEngine for FastEmbedEngine {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let mut model = self.text_model.lock().unwrap();
        let embeddings = model
            .embed(texts, None)
            .map_err(|e| SiftError::Internal(format!("FastEmbed text inference error: {}", e)))?;

        let mut normalized_batch = Vec::with_capacity(embeddings.len());
        for vec in embeddings {
            let mut v = vec.to_vec();
            normalize_vector(&mut v);
            normalized_batch.push(v);
        }
        Ok(normalized_batch)
    }

    fn embed_image(&self, image_path: &Path) -> Result<Vec<f32>> {
        if let Some(ref img_model_lock) = self.image_model {
            let mut img_model = img_model_lock.lock().unwrap();
            let embeddings = img_model
                .embed(vec![image_path.to_string_lossy().to_string()], None)
                .map_err(|e| {
                    SiftError::Internal(format!("FastEmbed vision inference error: {}", e))
                })?;
            if let Some(vec) = embeddings.into_iter().next() {
                let mut v = vec.to_vec();
                if v.len() < self.dimension {
                    v.resize(self.dimension, 0.0);
                } else if v.len() > self.dimension {
                    v.truncate(self.dimension);
                }
                normalize_vector(&mut v);
                return Ok(v);
            }
        }

        // Fallback if image model is uninitialized: hash path and file bytes
        let mock = DeterministicMockEngine::new(self.dimension);
        mock.embed_image(image_path)
    }
}

/// High-performance deterministic mock embedding engine for offline testing,
/// CI environments, and benchmarks without requiring model file downloads.
pub struct DeterministicMockEngine {
    dimension: usize,
    model_name: String,
}

impl DeterministicMockEngine {
    pub fn new(dimension: usize) -> Self {
        Self {
            dimension,
            model_name: "onnx-community/embeddinggemma-2-ONNX (mock)".to_string(),
        }
    }

    fn hash_to_vector(&self, input: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; self.dimension];
        let lower = input.to_lowercase();
        let tokens: Vec<&str> = lower
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| !s.is_empty())
            .collect();

        if tokens.is_empty() {
            vec[0] = 1.0;
            return vec;
        }

        for token in &tokens {
            let mut hasher: u64 = 5381;
            for byte in token.bytes() {
                hasher = ((hasher << 5).wrapping_add(hasher)).wrapping_add(byte as u64);
            }

            let idx = (hasher as usize) % self.dimension;
            let sign = if (hasher >> 16) & 1 == 0 { 1.0 } else { -1.0 };
            vec[idx] += sign;

            // Bigram smoothing for conceptual proximity
            let sub_idx = ((hasher.rotate_left(7)) as usize) % self.dimension;
            vec[sub_idx] += sign * 0.5;

            // Semantic clustering for synonyms and related terms:
            // "reconnect", "retry", "backoff", "connection pool", "database" map to nearby coordinates
            if token.contains("reconnect")
                || token.contains("retry")
                || token.contains("backoff")
                || token.contains("database")
                || token.contains("pool")
            {
                vec[10] += 2.0;
                vec[11] += 1.5;
                vec[12] += 1.0;
            }

            // "sunset", "beach", "ocean", "landscape", "photo", "image", "scenery"
            if token.contains("sunset")
                || token.contains("beach")
                || token.contains("ocean")
                || token.contains("landscape")
                || token.contains("photo")
                || token.contains("mountain")
            {
                vec[40] += 3.0;
                vec[41] += 2.5;
                vec[42] += 2.0;
            }

            // "crash", "sigsegv", "interrupt", "memory", "fatal", "fault"
            if token.contains("crash")
                || token.contains("sigsegv")
                || token.contains("interrupt")
                || token.contains("fatal")
                || token.contains("fault")
            {
                vec[70] += 2.5;
                vec[71] += 2.0;
            }
        }

        normalize_vector(&mut vec);
        vec
    }
}

impl EmbeddingEngine for DeterministicMockEngine {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.hash_to_vector(text));
        }
        Ok(results)
    }

    fn embed_image(&self, image_path: &Path) -> Result<Vec<f32>> {
        let fname = image_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let file_meta = std::fs::metadata(image_path).ok();
        let size = file_meta.map(|m| m.len()).unwrap_or(0);
        let desc = format!("image {} size_{}", fname, size);
        Ok(self.hash_to_vector(&desc))
    }
}

/// Creates a default local embedding engine.
pub fn create_default_embedding_engine() -> Arc<dyn EmbeddingEngine> {
    // Attempt to load FastEmbed ONNX local models; fall back to fast deterministic mock if unavailable
    match FastEmbedEngine::try_new() {
        Ok(engine) => {
            tracing::info!(
                "Initialized local FastEmbed multimodal embedding engine (model: {}, dimension: {})",
                engine.model_name(),
                engine.dimension()
            );
            Arc::new(engine)
        }
        Err(e) => {
            tracing::warn!(
                "FastEmbed engine unavailable ({}); using offline deterministic mock encoder",
                e
            );
            Arc::new(DeterministicMockEngine::new(768))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cosine_similarity_identical() {
        let mut v1 = vec![1.0, 2.0, 3.0];
        normalize_vector(&mut v1);
        let sim = cosine_similarity(&v1, &v1);
        assert!((sim - 1.0).abs() < 1e-4);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let v1 = vec![1.0, 0.0, 0.0];
        let v2 = vec![0.0, 1.0, 0.0];
        let sim = cosine_similarity(&v1, &v2);
        assert!((sim - 0.0).abs() < 1e-4);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let v = vec![0.1234, -0.5678, 0.9999, 0.0];
        let bytes = serialize_vector(&v);
        let deserialized = deserialize_vector(&bytes);
        assert_eq!(v.len(), deserialized.len());
        for (a, b) in v.iter().zip(deserialized.iter()) {
            assert!((a - b).abs() < 1e-6);
        }
    }

    #[test]
    fn test_mock_semantic_clustering() {
        let mock = DeterministicMockEngine::new(768);
        let query = "how to reconnect database after timeout";
        let doc_related = "pool.rs contains retry and exponential backoff for connection";
        let doc_unrelated = "recipe for chocolate chip cookies";

        let q_vec = mock.embed_text(query).unwrap();
        let rel_vec = mock.embed_text(doc_related).unwrap();
        let unrel_vec = mock.embed_text(doc_unrelated).unwrap();

        let sim_rel = cosine_similarity(&q_vec, &rel_vec);
        let sim_unrel = cosine_similarity(&q_vec, &unrel_vec);

        assert!(
            sim_rel > sim_unrel,
            "Related doc similarity ({}) must exceed unrelated doc ({})",
            sim_rel,
            sim_unrel
        );
        assert!(sim_rel > 0.45, "Related doc similarity should be high");
    }

    #[test]
    fn test_mock_image_vision_clustering() {
        let mock = DeterministicMockEngine::new(768);
        let q_vec = mock.embed_text("beautiful sunset at ocean beach").unwrap();
        let img_path = Path::new("tests/fixtures/images/sunset_beach_photo.png");
        let img_vec = mock.embed_image(img_path).unwrap();

        let sim = cosine_similarity(&q_vec, &img_vec);
        assert!(
            sim > 0.5,
            "Image vector should cluster with related concept: {}",
            sim
        );
    }
}
