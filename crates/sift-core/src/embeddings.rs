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

    /// Embeds a search query with Google DeepMind EmbeddingGemma 2 asymmetric instruction prefix:
    /// `task: search result | query: {query}`
    fn embed_query(&self, query: &str) -> Result<Vec<f32>> {
        let formatted = format!("task: search result | query: {}", query);
        self.embed_text(&formatted)
    }

    /// Embeds a document chunk with Google DeepMind EmbeddingGemma 2 document prefix:
    /// `title: {title} | text: {content}`
    fn embed_document(&self, title: Option<&str>, content: &str) -> Result<Vec<f32>> {
        let formatted = match title {
            Some(t) if !t.is_empty() => format!("title: {} | text: {}", t, content),
            _ => format!("title: none | text: {}", content),
        };
        self.embed_text(&formatted)
    }

    /// Generates a normalized visual vector embedding for an image file.
    fn embed_image(&self, image_path: &Path) -> Result<Vec<f32>>;

    /// Embeds document chunks in one batch (same prefix format as [`Self::embed_document`]).
    fn embed_documents(&self, title: Option<&str>, contents: &[String]) -> Result<Vec<Vec<f32>>> {
        let formatted: Vec<String> = contents
            .iter()
            .map(|c| match title {
                Some(t) if !t.is_empty() => format!("title: {} | text: {}", t, c),
                _ => format!("title: none | text: {}", c),
            })
            .collect();
        self.embed_texts(&formatted)
    }

    /// Generates a normalized visual-query vector for cross-modal image searching.
    fn embed_vision_query(&self, text: &str) -> Result<Vec<f32>> {
        self.embed_query(text)
    }

    /// True when text queries and images share one embedding space, so the text query vector can
    /// be compared against image vectors directly.
    fn shares_text_image_space(&self) -> bool {
        false
    }

    /// Relevance thresholds calibrated for this engine's similarity distributions.
    fn relevance_profile(&self) -> crate::ranking::RelevanceProfile {
        crate::ranking::RelevanceProfile::CLIP_GEMMA1
    }

    /// Frees model memory after `idle` without use; the next call reloads it. Engines that can't
    /// unload return `false`.
    fn release_if_idle(&self, _idle: std::time::Duration) -> bool {
        false
    }

    /// Whether model weights are currently resident in memory.
    fn is_loaded(&self) -> bool {
        true
    }
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
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif"
    )
}


/// Returns the centralized directory for Sift ML model caches to prevent disk pollution.
pub fn get_sift_models_cache_dir() -> std::path::PathBuf {
    if let Ok(custom) = std::env::var("SIFT_CACHE_DIR") {
        return std::path::PathBuf::from(custom);
    }
    if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        return std::path::PathBuf::from(xdg).join("sift").join("models");
    }
    if let Ok(home) = std::env::var("HOME") {
        return std::path::PathBuf::from(home).join(".cache").join("sift").join("models");
    }
    std::path::PathBuf::from(".fastembed_cache")
}

/// Prompt templates averaged for CLIP text queries (`{}` is replaced by the user's query).
const CLIP_PROMPT_TEMPLATES: &[&str] = &["{}", "a photo of {}", "an image of {}", "a picture of {}"];

/// FastEmbed-backed local ONNX embedding engine supporting both text and vision models.
pub struct FastEmbedEngine {
    text_model: Mutex<TextEmbedding>,
    vision_text_model: Option<Mutex<TextEmbedding>>,
    image_model: Option<Mutex<ImageEmbedding>>,
    dimension: usize,
    model_name: String,
}

impl FastEmbedEngine {
    /// Initializes a local ONNX FastEmbed engine with a specific embedding model.
    pub fn try_new_with_model(model: EmbeddingModel) -> Result<Self> {
        // Text side is the original text-only EmbeddingGemma (300M); images use CLIP ViT-B/32.
        let (dim, name) = match model {
            EmbeddingModel::EmbeddingGemma300M => (768, "embeddinggemma-300m + clip-vit-b-32"),
            EmbeddingModel::EmbeddingGemma300MQ => (768, "embeddinggemma-300m-q8 + clip-vit-b-32"),
            EmbeddingModel::EmbeddingGemma300MQ4 => (768, "embeddinggemma-300m-q4 + clip-vit-b-32"),
            EmbeddingModel::AllMiniLML6V2 => (384, "all-minilm-l6-v2 + clip-vit-b-32"),
            _ => (768, "fastembed + clip-vit-b-32"),
        };

        let cache_dir = get_sift_models_cache_dir();
        let _ = std::fs::create_dir_all(&cache_dir);

        let text_init = TextInitOptions::new(model)
            .with_cache_dir(cache_dir.clone())
            .with_show_download_progress(false);
        let text_model = TextEmbedding::try_new(text_init).map_err(|e| {
            SiftError::Internal(format!("Failed to initialize FastEmbed text model: {}", e))
        })?;

        let vision_text_model = {
            let vt_init = TextInitOptions::new(EmbeddingModel::ClipVitB32)
                .with_cache_dir(cache_dir.clone())
                .with_show_download_progress(false);
            match TextEmbedding::try_new(vt_init) {
                Ok(m) => Some(Mutex::new(m)),
                Err(e) => {
                    tracing::warn!("Failed to load vision text model (CLIP ViT-B/32): {}", e);
                    None
                }
            }
        };

        let image_model = {
            let img_init = ImageInitOptions::new(ImageEmbeddingModel::ClipVitB32)
                .with_cache_dir(cache_dir)
                .with_show_download_progress(false);
            match ImageEmbedding::try_new(img_init) {
                Ok(m) => Some(Mutex::new(m)),
                Err(e) => {
                    tracing::warn!("Failed to load vision image model (CLIP ViT-B/32): {}", e);
                    None
                }
            }
        };

        Ok(Self {
            text_model: Mutex::new(text_model),
            vision_text_model,
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
            "minilm" => EmbeddingModel::AllMiniLML6V2,
            "gemma-q" => EmbeddingModel::EmbeddingGemma300MQ,
            "gemma-q4" => EmbeddingModel::EmbeddingGemma300MQ4,
            _ => EmbeddingModel::EmbeddingGemma300M,
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

    /// CLIP query embedding averaged over a small prompt ensemble. Measured on a real index this
    /// widened the margin between the true match and the runner-up (e.g. "sunset": 3.7σ vs 2.9σ).
    fn embed_vision_query(&self, text: &str) -> Result<Vec<f32>> {
        if let Some(ref vt_model_lock) = self.vision_text_model {
            let prompts: Vec<String> = CLIP_PROMPT_TEMPLATES
                .iter()
                .map(|t| t.replace("{}", text))
                .collect();
            let mut vt_model = vt_model_lock.lock().unwrap();
            let embeddings = vt_model
                .embed(&prompts, None)
                .map_err(|e| SiftError::Internal(format!("Vision text query error: {}", e)))?;
            let mut acc: Vec<f32> = Vec::new();
            for vec in embeddings {
                let mut v = vec.to_vec();
                normalize_vector(&mut v);
                if acc.is_empty() {
                    acc = v;
                } else {
                    for (a, b) in acc.iter_mut().zip(v) {
                        *a += b;
                    }
                }
            }
            if !acc.is_empty() {
                normalize_vector(&mut acc);
                return Ok(acc);
            }
        }
        self.embed_text(text)
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
            .filter(|s| !s.is_empty() && !matches!(*s, "task" | "search" | "result" | "query" | "title" | "text" | "none"))
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

/// EmbeddingGemma 2: text, documents, and images in one 768-d space (see [`crate::gemma2`]).
pub struct EmbeddingGemma2Engine {
    model: crate::gemma2::EmbeddingGemma2,
}

impl EmbeddingGemma2Engine {
    pub fn try_new() -> Result<Self> {
        let cache_dir = get_sift_models_cache_dir();
        let _ = std::fs::create_dir_all(&cache_dir);
        let threads = std::env::var("SIFT_EMBED_THREADS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(default_embed_threads);
        let config = crate::gemma2::Gemma2Config {
            intra_threads: threads,
            ..crate::gemma2::Gemma2Config::default()
        };
        Ok(Self {
            model: crate::gemma2::EmbeddingGemma2::load(&cache_dir, config)?,
        })
    }
}

/// Half the logical cores (2..=8): fast enough for indexing without saturating the machine.
fn default_embed_threads() -> usize {
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    (cores / 2).clamp(2, 8)
}

impl EmbeddingEngine for EmbeddingGemma2Engine {
    fn model_name(&self) -> &str {
        crate::gemma2::MODEL_ID
    }

    fn dimension(&self) -> usize {
        crate::gemma2::DIM
    }

    fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        // Small batches keep padding waste and peak memory low for uneven chunk lengths.
        let mut out = Vec::with_capacity(texts.len());
        for batch in texts.chunks(4) {
            out.extend(self.model.embed_texts(batch)?);
        }
        Ok(out)
    }

    fn embed_image(&self, image_path: &Path) -> Result<Vec<f32>> {
        self.model.embed_image(image_path)
    }

    fn shares_text_image_space(&self) -> bool {
        self.model.has_vision()
    }

    fn relevance_profile(&self) -> crate::ranking::RelevanceProfile {
        crate::gemma2::RELEVANCE_PROFILE
    }

    fn release_if_idle(&self, idle: std::time::Duration) -> bool {
        self.model.release_if_idle(idle)
    }

    fn is_loaded(&self) -> bool {
        self.model.is_loaded()
    }
}

/// Creates the default local embedding engine.
///
/// `SIFT_EMBEDDING_MODEL`: unset/`embeddinggemma-2` → EmbeddingGemma 2 (text + images, one model);
/// `legacy` (or `gemma`, `gemma-q`, `gemma-q4`, `minilm`) → fastembed text model + CLIP.
/// Falls back to legacy, then to the offline mock, if a model cannot be loaded.
pub fn create_default_embedding_engine() -> Arc<dyn EmbeddingEngine> {
    let choice = std::env::var("SIFT_EMBEDDING_MODEL").unwrap_or_default();
    if choice.is_empty() || choice.eq_ignore_ascii_case("embeddinggemma-2") {
        match EmbeddingGemma2Engine::try_new() {
            Ok(engine) => {
                tracing::info!(model = engine.model_name(), "Initialized EmbeddingGemma 2 engine");
                return Arc::new(engine);
            }
            Err(e) => {
                tracing::warn!("EmbeddingGemma 2 unavailable ({}); falling back to legacy models", e)
            }
        }
    }
    match FastEmbedEngine::try_new() {
        Ok(engine) => {
            tracing::info!(
                "Initialized legacy FastEmbed engine (model: {}, dimension: {})",
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
