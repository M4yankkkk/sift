//! Google DeepMind EmbeddingGemma 2 on ONNX Runtime: one model embeds text and images into a
//! shared 768-dimensional space, so a text query is compared directly against photos.
//!
//! Inference contract (from `onnx-community/embeddinggemma-2-ONNX`):
//! - `vision_encoder`: `pixel_values [b, patches, 16·16·3]` + `pixel_position_ids [b, patches, 2]`
//!   → `image_features [soft_tokens, 512]`
//! - `model`: `input_ids`, `attention_mask`, `{image,video,audio}_features [n, 512]`
//!   → `sentence_embedding [b, 768]`, with image features filling `<|image|>` placeholder slots.
//!
//! Image preprocessing ports `Gemma4ImageProcessor` from Hugging Face transformers.

use crate::error::{Result, SiftError};
use image::imageops::FilterType;
use ndarray::{Array2, Array3};
use ort::session::Session;
use ort::value::Value;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tokenizers::{Tokenizer, TruncationParams};

pub const REPO: &str = "onnx-community/embeddinggemma-2-ONNX";
/// Stored with every vector; changing it makes the app re-embed the index with the new setup.
pub const MODEL_ID: &str = "google/embeddinggemma-2 (onnx q8 text, q4 vision, 70 tokens)";
pub const DIM: usize = 768;

/// Calibrated on a real ~5k-file index (670 photos/screenshots, 12k meaningful text chunks).
/// Images: query cosines sit near 0.60 (sd ≈ 0.025); true matches stood 4–6.4σ above the mean
/// ("sunset" 5.9σ, "dog" 6.4σ, "earth" 4.5σ), queries with no matching photo topped out at 2.8–3.4σ.
/// Text: genuine matches 4.0–5.8σ ("database retry" 5.8σ, "machine learning" 4.7σ); queries with
/// no relevant document topped out at 2.8–3.8σ.
pub const RELEVANCE_PROFILE: crate::ranking::RelevanceProfile = crate::ranking::RelevanceProfile {
    image_z_blended: 3.5,
    image_z_visual: 3.0,
    text_z: 4.0,
    image_sim_floor: 0.55,
    text_sim_floor: 0.50,
    image_sim_small_corpus: 0.68,
    text_sim_small_corpus: 0.66,
};

const TOKENIZER_FILE: &str = "tokenizer.json";

/// ONNX weight precision. q4 uses 4-bit `MatMulNBits` kernels, q8 dynamic 8-bit quantization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Precision {
    Q8,
    Q4,
}

impl Precision {
    fn suffix(self) -> &'static str {
        match self {
            Precision::Q8 => "quantized",
            Precision::Q4 => "q4",
        }
    }

    fn files(self, component: &str) -> [String; 2] {
        let base = format!("onnx/{}_{}.onnx", component, self.suffix());
        [base.clone(), format!("{}_data", base)]
    }
}

/// How the model is loaded and how much detail each image gets.
#[derive(Debug, Clone, Copy)]
pub struct Gemma2Config {
    pub text_precision: Precision,
    pub vision_precision: Precision,
    /// `false` skips the vision encoder entirely (text-only installs).
    pub with_vision: bool,
    pub intra_threads: usize,
    /// Vision soft-token budget per image: 70, 140, 280 (model default), 560 or 1120.
    pub vision_budget: usize,
}

/// Defaults measured on an i7-13620H (8 threads): q4 vision at a 70-token budget embeds an image
/// in ~0.6 s versus ~3.6 s for q8 at the model's default 280, and scored the model card's
/// reference image/query pairs within 0.02 of full detail.
impl Default for Gemma2Config {
    fn default() -> Self {
        Self {
            text_precision: Precision::Q8,
            vision_precision: Precision::Q4,
            with_vision: true,
            intra_threads: 4,
            vision_budget: 70,
        }
    }
}

const PATCH_SIZE: usize = 16;
const POOLING_KERNEL: usize = 3;
const PATCH_PIXELS: usize = PATCH_SIZE * PATCH_SIZE * 3;
const FEATURE_DIM: usize = 512;

/// Document chunks are ~512 tokens; the cap bounds latency and memory on unusually dense chunks.
/// Keyword search still indexes every word.
const MAX_TEXT_TOKENS: usize = 512;
/// Refuse to decode absurdly large images (decoded RGB would exceed ~240 MB).
const MAX_IMAGE_PIXELS: u64 = 80_000_000;

const BOS: i64 = 2;
const EOS: i64 = 1;
const PAD: i64 = 0;
const BOI: i64 = 255_999;
const IMAGE_TOKEN: i64 = 258_880;
const EOI: i64 = 258_882;

pub struct EmbeddingGemma2 {
    text: Mutex<Session>,
    /// Downloaded vision encoder, loaded on the first image: searches only embed text queries, so
    /// an idle app never pays for it.
    vision_path: Option<PathBuf>,
    vision: Mutex<Option<Session>>,
    tokenizer: Tokenizer,
    vision_budget: usize,
    intra_threads: usize,
}

impl EmbeddingGemma2 {
    /// Loads the model, downloading it into `cache_dir` (Hugging Face cache layout) on first use.
    pub fn load(cache_dir: &Path, config: Gemma2Config) -> Result<Self> {
        let text_files = config.text_precision.files("model");
        let text_path = fetch(cache_dir, &[&text_files[0], &text_files[1]])?;
        let tokenizer_path = fetch(cache_dir, &[TOKENIZER_FILE])?;
        let vision_path = if config.with_vision {
            let files = config.vision_precision.files("vision_encoder");
            Some(fetch(cache_dir, &[&files[0], &files[1]])?)
        } else {
            None
        };
        let intra_threads = config.intra_threads;

        let mut tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| internal("load EmbeddingGemma 2 tokenizer", e))?;
        tokenizer
            .with_truncation(Some(TruncationParams {
                max_length: MAX_TEXT_TOKENS,
                ..Default::default()
            }))
            .map_err(|e| internal("configure tokenizer truncation", e))?;
        tokenizer.with_padding(None);

        let text = Mutex::new(session(&text_path, intra_threads)?);
        Ok(Self {
            text,
            vision_path,
            vision: Mutex::new(None),
            tokenizer,
            vision_budget: config.vision_budget,
            intra_threads,
        })
    }

    pub fn has_vision(&self) -> bool {
        self.vision_path.is_some()
    }

    /// Embeds already-prefixed texts (`task: search result | query: …`, `title: … | text: …`).
    pub fn embed_texts(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        if texts.is_empty() {
            return Ok(Vec::new());
        }
        let encodings = self
            .tokenizer
            .encode_batch(texts.to_vec(), true)
            .map_err(|e| internal("tokenize", e))?;
        let seq_len = encodings.iter().map(|e| e.len()).max().unwrap_or(0).max(1);
        let mut ids = Array2::<i64>::from_elem((texts.len(), seq_len), PAD);
        let mut mask = Array2::<i64>::zeros((texts.len(), seq_len));
        for (row, enc) in encodings.iter().enumerate() {
            for (col, id) in enc.get_ids().iter().enumerate() {
                ids[[row, col]] = *id as i64;
                mask[[row, col]] = 1;
            }
        }
        self.run_text(ids, mask, Array2::zeros((0, FEATURE_DIM)))
    }

    /// Embeds one image file. Images carry no task prefix.
    pub fn embed_image(&self, path: &Path) -> Result<Vec<f32>> {
        let vision_path = self
            .vision_path
            .as_ref()
            .ok_or_else(|| SiftError::Internal("vision encoder not installed".to_string()))?;

        let (w, h) = image::ImageReader::open(path)
            .map_err(|e| internal("open image", e))?
            .with_guessed_format()
            .map_err(|e| internal("detect image format", e))?
            .into_dimensions()
            .map_err(|e| internal("read image dimensions", e))?;
        if (w as u64) * (h as u64) > MAX_IMAGE_PIXELS {
            return Err(SiftError::Internal(format!(
                "image too large to embed ({}x{})",
                w, h
            )));
        }
        let img = image::open(path).map_err(|e| internal("decode image", e))?;
        let prepared = preprocess_image(&img, self.vision_budget)?;

        let features = {
            let mut guard = self.vision.lock().unwrap();
            if guard.is_none() {
                *guard = Some(session(vision_path, self.intra_threads)?);
            }
            let session = guard.as_mut().expect("vision session just loaded");
            let pixel_values = Value::from_array(prepared.pixel_values)
                .map_err(|e| internal("pixel tensor", e))?;
            let positions = Value::from_array(prepared.position_ids)
                .map_err(|e| internal("position tensor", e))?;
            let outputs = session
                .run(ort::inputs![
                    "pixel_values" => pixel_values,
                    "pixel_position_ids" => positions,
                ])
                .map_err(|e| internal("vision encoder", e))?;
            let (shape, data) = outputs["image_features"]
                .try_extract_tensor::<f32>()
                .map_err(|e| internal("read image features", e))?;
            let rows = shape[0] as usize;
            if rows < prepared.soft_tokens {
                return Err(SiftError::Internal(format!(
                    "vision encoder returned {} features, expected {}",
                    rows, prepared.soft_tokens
                )));
            }
            // Real tokens come first; anything after them stems from padding patches.
            Array2::from_shape_vec(
                (prepared.soft_tokens, FEATURE_DIM),
                data[..prepared.soft_tokens * FEATURE_DIM].to_vec(),
            )
            .map_err(|e| internal("reshape image features", e))?
        };

        let mut seq: Vec<i64> = Vec::with_capacity(prepared.soft_tokens + 4);
        seq.push(BOS);
        seq.push(BOI);
        seq.extend(std::iter::repeat(IMAGE_TOKEN).take(prepared.soft_tokens));
        seq.push(EOI);
        seq.push(EOS);
        let len = seq.len();
        let ids = Array2::from_shape_vec((1, len), seq).map_err(|e| internal("ids", e))?;
        let mask = Array2::<i64>::ones((1, len));
        Ok(self.run_text(ids, mask, features)?.remove(0))
    }

    fn run_text(
        &self,
        ids: Array2<i64>,
        mask: Array2<i64>,
        image_features: Array2<f32>,
    ) -> Result<Vec<Vec<f32>>> {
        let batch = ids.nrows();
        let mut session = self.text.lock().unwrap();
        let outputs = session
            .run(ort::inputs![
                "input_ids" => Value::from_array(ids).map_err(|e| internal("ids tensor", e))?,
                "attention_mask" => Value::from_array(mask).map_err(|e| internal("mask tensor", e))?,
                "image_features" => Value::from_array(image_features).map_err(|e| internal("features tensor", e))?,
                "video_features" => Value::from_array(Array2::<f32>::zeros((0, FEATURE_DIM))).map_err(|e| internal("video tensor", e))?,
                "audio_features" => Value::from_array(Array2::<f32>::zeros((0, FEATURE_DIM))).map_err(|e| internal("audio tensor", e))?,
            ])
            .map_err(|e| internal("text model", e))?;
        let (shape, data) = outputs["sentence_embedding"]
            .try_extract_tensor::<f32>()
            .map_err(|e| internal("read sentence embedding", e))?;
        let dim = shape[1] as usize;
        let mut result = Vec::with_capacity(batch);
        for row in 0..batch {
            let mut v = data[row * dim..(row + 1) * dim].to_vec();
            crate::embeddings::normalize_vector(&mut v);
            result.push(v);
        }
        Ok(result)
    }
}

fn internal(what: &str, e: impl std::fmt::Display) -> SiftError {
    SiftError::Internal(format!("EmbeddingGemma 2: {}: {}", what, e))
}

fn session(path: &Path, intra_threads: usize) -> Result<Session> {
    // Sift stays resident, so memory must come back after a burst of indexing. ONNX Runtime's CPU
    // arena and per-shape memory patterns otherwise keep the peak (2.7 GB after embedding a
    // 1,300-chunk PDF) reserved for the life of the process.
    let what = format!("load {}", path.display());
    let builder = Session::builder().map_err(|e| internal(&what, e))?;
    let builder = builder
        .with_execution_providers([ort::ep::CPU::default()
            .with_arena_allocator(false)
            .build()])
        .map_err(|e| internal(&what, e))?;
    let builder = builder
        .with_memory_pattern(false)
        .map_err(|e| internal(&what, e))?;
    let mut builder = builder
        .with_intra_threads(intra_threads.max(1))
        .map_err(|e| internal(&what, e))?;
    builder.commit_from_file(path).map_err(|e| internal(&what, e))
}

/// Downloads `files` from [`REPO`] into the Hugging Face cache under `cache_dir`, returning the
/// local path of the first one. Companion files (ONNX external data) land beside it.
fn fetch(cache_dir: &Path, files: &[&str]) -> Result<PathBuf> {
    let api = hf_hub::api::sync::ApiBuilder::new()
        .with_cache_dir(cache_dir.to_path_buf())
        .with_progress(false)
        .build()
        .map_err(|e| internal("init model download", e))?;
    let repo = api.model(REPO.to_string());
    let mut first = None;
    for file in files {
        let path = repo
            .get(file)
            .map_err(|e| internal(&format!("download {}", file), e))?;
        first.get_or_insert(path);
    }
    first.ok_or_else(|| SiftError::Internal("no model files requested".to_string()))
}

/// Pixels and patch coordinates ready for the vision encoder.
pub struct PreparedImage {
    pub pixel_values: Array3<f32>,
    pub position_ids: Array3<i64>,
    pub soft_tokens: usize,
}

/// Largest size that keeps the aspect ratio, fits `max_patches`, and has sides divisible by
/// `pooling_kernel * patch_size` (port of transformers' `get_aspect_ratio_preserving_size`).
pub fn aspect_ratio_preserving_size(
    height: usize,
    width: usize,
    patch_size: usize,
    max_patches: usize,
    pooling_kernel: usize,
) -> Result<(usize, usize)> {
    let total_px = (height * width) as f64;
    let target_px = (max_patches * patch_size * patch_size) as f64;
    let factor = (target_px / total_px).sqrt();
    let side_mult = pooling_kernel * patch_size;
    let mut target_h = ((factor * height as f64) / side_mult as f64).floor() as usize * side_mult;
    let mut target_w = ((factor * width as f64) / side_mult as f64).floor() as usize * side_mult;

    if target_h == 0 && target_w == 0 {
        return Err(SiftError::Internal(format!(
            "cannot resize {}x{} image for embedding",
            width, height
        )));
    }
    let max_side = (max_patches / (pooling_kernel * pooling_kernel)) * side_mult;
    if target_h == 0 {
        target_h = side_mult;
        target_w = ((width / height) * side_mult).min(max_side);
    } else if target_w == 0 {
        target_w = side_mult;
        target_h = ((height / width) * side_mult).min(max_side);
    }
    if (target_h * target_w) as f64 > target_px {
        return Err(SiftError::Internal(format!(
            "resizing {}x{} exceeds the patch budget",
            width, height
        )));
    }
    Ok((target_h, target_w))
}

/// Resize (bicubic), rescale to [0, 1], cut into 16×16 patches flattened as (row, col, channel),
/// and pad to the fixed patch budget with zero pixels and `-1` positions.
pub fn preprocess_image(img: &image::DynamicImage, vision_budget: usize) -> Result<PreparedImage> {
    let max_patches = vision_budget * POOLING_KERNEL * POOLING_KERNEL;
    let rgb = img.to_rgb8();
    let (w, h) = (rgb.width() as usize, rgb.height() as usize);
    let (th, tw) =
        aspect_ratio_preserving_size(h, w, PATCH_SIZE, max_patches, POOLING_KERNEL)?;
    let resized = if (th, tw) == (h, w) {
        rgb
    } else {
        image::imageops::resize(&rgb, tw as u32, th as u32, FilterType::CatmullRom)
    };

    let patches_h = th / PATCH_SIZE;
    let patches_w = tw / PATCH_SIZE;
    let n_patches = patches_h * patches_w;
    let mut pixel_values = Array3::<f32>::zeros((1, max_patches, PATCH_PIXELS));
    let mut position_ids = Array3::<i64>::from_elem((1, max_patches, 2), -1);
    let raw = resized.as_raw();
    for py in 0..patches_h {
        for px in 0..patches_w {
            let patch = py * patches_w + px;
            position_ids[[0, patch, 0]] = px as i64;
            position_ids[[0, patch, 1]] = py as i64;
            let mut k = 0;
            for y in 0..PATCH_SIZE {
                let row_start = ((py * PATCH_SIZE + y) * tw + px * PATCH_SIZE) * 3;
                for v in &raw[row_start..row_start + PATCH_SIZE * 3] {
                    pixel_values[[0, patch, k]] = *v as f32 / 255.0;
                    k += 1;
                }
            }
        }
    }
    Ok(PreparedImage {
        pixel_values,
        position_ids,
        soft_tokens: n_patches / (POOLING_KERNEL * POOLING_KERNEL),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resize_matches_reference_budget() {
        // 640x480 at 2520 patches of 16px → sides divisible by 48 within budget
        let (h, w) = aspect_ratio_preserving_size(480, 640, 16, 2520, 3).unwrap();
        assert_eq!((h % 48, w % 48), (0, 0));
        assert!((h / 16) * (w / 16) <= 2520);
        assert!(w > h);
    }

    #[test]
    fn extreme_aspect_ratio_clamps() {
        let (h, w) = aspect_ratio_preserving_size(10, 5000, 16, 2520, 3).unwrap();
        assert_eq!(h, 48);
        assert!(w <= (2520 / 9) * 48);
    }

    #[test]
    fn patch_layout_and_padding() {
        let img = image::DynamicImage::new_rgb8(96, 48);
        let p = preprocess_image(&img, 280).unwrap();
        assert_eq!(p.pixel_values.shape(), &[1, 2520, PATCH_PIXELS]);
        let real = (p.soft_tokens) * 9;
        // Positions are (x, y) in row-major patch order; padding positions are -1.
        assert_eq!(p.position_ids[[0, 1, 0]], 1);
        assert_eq!(p.position_ids[[0, 1, 1]], 0);
        assert_eq!(p.position_ids[[0, real, 0]], -1);
    }
}
