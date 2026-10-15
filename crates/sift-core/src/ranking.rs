//! Query intent parsing, per-query similarity normalisation, and weighted rank fusion.
//!
//! Thresholds here were calibrated against a real ~5.5k-file index (3.2k images, 14.6k text chunks):
//! - CLIP image similarities sit in a narrow band (mean ≈ 0.18–0.22, sd ≈ 0.02–0.03) that shifts per
//!   query, so absolute cut-offs either flood or starve. Relevant images stood ≥ 3σ above the query's
//!   own corpus mean; unrelated queries ("database retry", "invoice") topped out around 2.5σ.
//! - EmbeddingGemma text chunks: junk tops out ≈ 3.9σ, genuine concept matches land at ≥ 4.5σ.

use std::collections::HashMap;

/// Minimum number of scored vectors before per-query z-scores are trusted.
/// Smaller corpora fall back to absolute similarity floors.
pub const MIN_STATS_N: usize = 50;

/// Relevance gates for one embedding engine. Each model has its own similarity scale (CLIP
/// text-image cosines sit near 0.2, EmbeddingGemma 2's near 0.6), so thresholds travel with it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RelevanceProfile {
    /// z-score an image must reach to appear in the blended "All" list.
    pub image_z_blended: f32,
    /// z-score an image must reach in explicit Visual mode (the user asked for images).
    pub image_z_visual: f32,
    /// z-score a text/OCR chunk must reach to count as a semantic concept match.
    pub text_z: f32,
    /// Absolute floors applied alongside z-gates (guard against degenerate corpora).
    pub image_sim_floor: f32,
    pub text_sim_floor: f32,
    /// Absolute thresholds used when the corpus is too small for z-scores.
    pub image_sim_small_corpus: f32,
    pub text_sim_small_corpus: f32,
}

impl RelevanceProfile {
    /// CLIP ViT-B/32 images + EmbeddingGemma 300M text (legacy engine, and the test mock).
    /// Relevant images stood ≥ 3σ above the query's corpus mean; unrelated queries topped out
    /// near 2.5σ. Text junk topped out ≈ 3.9σ, genuine matches ≥ 4.5σ.
    pub const CLIP_GEMMA1: Self = Self {
        image_z_blended: 3.0,
        image_z_visual: 2.5,
        text_z: 4.2,
        image_sim_floor: 0.20,
        text_sim_floor: 0.22,
        image_sim_small_corpus: 0.22,
        text_sim_small_corpus: 0.30,
    };
}

impl Default for RelevanceProfile {
    fn default() -> Self {
        Self::CLIP_GEMMA1
    }
}

/// Reciprocal-rank-fusion damping constant. Small, so the head of each list dominates.
pub const RRF_K: f32 = 4.0;

/// Running mean / standard deviation of one query's similarities against a corpus.
#[derive(Debug, Clone, Copy, Default)]
pub struct SimStats {
    pub n: usize,
    pub mean: f32,
    pub sd: f32,
}

impl SimStats {
    pub fn from_sums(n: usize, sum: f64, sum_sq: f64) -> Self {
        if n == 0 {
            return Self::default();
        }
        let mean = sum / n as f64;
        let var = (sum_sq / n as f64 - mean * mean).max(0.0);
        Self {
            n,
            mean: mean as f32,
            sd: var.sqrt() as f32,
        }
    }

    /// Whether this distribution is large and spread enough for z-scores to mean anything.
    pub fn is_reliable(&self) -> bool {
        self.n >= MIN_STATS_N && self.sd > 1e-4
    }

    pub fn z(&self, sim: f32) -> f32 {
        if self.sd > 1e-6 {
            (sim - self.mean) / self.sd
        } else {
            0.0
        }
    }
}

pub fn clamp01(x: f32) -> f32 {
    x.clamp(0.0, 1.0)
}

/// Hard result-type restriction requested explicitly by the query.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeFilter {
    Images,
    Extensions(Vec<String>),
}

impl TypeFilter {
    pub fn allows(&self, extension: &str) -> bool {
        let ext = extension.to_lowercase();
        match self {
            TypeFilter::Images => is_image_extension(&ext),
            TypeFilter::Extensions(exts) => exts.contains(&ext),
        }
    }

    pub fn allows_images(&self) -> bool {
        match self {
            TypeFilter::Images => true,
            TypeFilter::Extensions(exts) => exts.iter().any(|e| is_image_extension(e)),
        }
    }
}

pub fn is_image_extension(ext: &str) -> bool {
    matches!(
        ext.to_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif" | "tif" | "tiff" | "heic" | "avif"
    )
}

const IMAGE_WORDS: &[&str] = &[
    "image",
    "images",
    "img",
    "photo",
    "photos",
    "picture",
    "pictures",
    "pic",
    "pics",
    "wallpaper",
    "wallpapers",
    "screenshot",
    "screenshots",
    "jpg",
    "jpeg",
    "png",
    "webp",
    "gif",
];
const PDF_EXTS: &[&str] = &["pdf"];
const SLIDE_EXTS: &[&str] = &["ppt", "pptx", "odp", "key", "pdf"];
const DOC_EXTS: &[&str] = &["pdf", "doc", "docx", "odt", "rtf", "txt", "md"];
const SHEET_EXTS: &[&str] = &["xlsx", "xls", "csv", "ods", "tsv"];
const CODE_EXTS: &[&str] = &[
    "rs", "py", "js", "jsx", "ts", "tsx", "go", "java", "kt", "c", "h", "cpp", "hpp", "cc", "cs",
    "rb", "php", "swift", "sh", "sql", "html", "css", "scss", "json", "yaml", "yml", "toml",
];

/// Words that carry no subject on their own ("a screenshot", "my photos").
const FILLERS: &[&str] = &[
    "a", "an", "the", "my", "some", "all", "any", "me", "show", "find",
];

/// Connector words that may follow a leading type word: "pic of earth", "slides on transformers".
const CONNECTORS: &[&str] = &["of", "from", "with", "about", "on", "for", "showing"];

fn exts(list: &[&str]) -> TypeFilter {
    TypeFilter::Extensions(list.iter().map(|e| e.to_string()).collect())
}

fn type_word_filter(word: &str) -> Option<TypeFilter> {
    if IMAGE_WORDS.contains(&word) {
        return Some(TypeFilter::Images);
    }
    match word {
        "pdf" | "pdfs" => Some(exts(PDF_EXTS)),
        "slides" | "slide" | "ppt" | "pptx" | "presentation" | "presentations" | "deck" => {
            Some(exts(SLIDE_EXTS))
        }
        "doc" | "docs" | "docx" | "document" | "documents" => Some(exts(DOC_EXTS)),
        "spreadsheet" | "spreadsheets" | "xlsx" | "excel" | "csv" | "sheet" | "sheets" => {
            Some(exts(SHEET_EXTS))
        }
        _ => None,
    }
}

/// Explicit `type:` / `ext:` operator values.
fn operator_filter(key: &str, value: &str) -> Option<TypeFilter> {
    let value = value.trim_start_matches('.').to_lowercase();
    if value.is_empty() {
        return None;
    }
    match key {
        "type" => match value.as_str() {
            "code" | "source" => Some(exts(CODE_EXTS)),
            other => type_word_filter(other).or_else(|| {
                CODE_EXTS
                    .iter()
                    .chain(DOC_EXTS)
                    .chain(SHEET_EXTS)
                    .chain(SLIDE_EXTS)
                    .find(|e| **e == other)
                    .map(|e| exts(&[*e]))
            }),
        },
        // Exact extension, so ext:png does not widen to every image type.
        "ext" => Some(TypeFilter::Extensions(vec![value])),
        _ => None,
    }
}

/// Language names that softly prefer source files without excluding anything else.
fn language_extensions(word: &str) -> &'static [&'static str] {
    match word {
        "rust" => &["rs"],
        "python" => &["py"],
        "javascript" | "js" => &["js", "jsx", "mjs"],
        "typescript" | "ts" => &["ts", "tsx"],
        "java" => &["java"],
        "kotlin" => &["kt"],
        "golang" => &["go"],
        "cpp" | "c++" => &["cpp", "hpp", "cc"],
        "yaml" => &["yaml", "yml"],
        "json" => &["json"],
        "toml" => &["toml"],
        "sql" => &["sql"],
        "bash" | "shell" => &["sh"],
        "markdown" => &["md"],
        _ => &[],
    }
}

/// What the user is looking for, derived from the raw query string.
#[derive(Debug, Clone, Default)]
pub struct QueryIntent {
    /// Text handed to the retrievers, with explicit type words and operators removed.
    pub text: String,
    /// Hard restriction on result types (from "sunset photo", "resume pdf", `type:pdf`, ...).
    pub filter: Option<TypeFilter>,
    /// Extensions softly preferred (e.g. "retry logic in rust" → .rs). Never filters.
    pub preferred_extensions: Vec<&'static str>,
    /// Query looks like an identifier or path (snake_case, CamelCase, `a::b`, `foo.rs`).
    pub code_like: bool,
    /// Query contains a quoted phrase: the user wants an exact keyword match.
    pub quoted: bool,
    /// Number of words in `text`.
    pub word_count: usize,
}

impl QueryIntent {
    pub fn parse(raw: &str) -> Self {
        let raw = raw.trim();
        let quoted = raw.matches('"').count() >= 2;

        let mut filter: Option<TypeFilter> = None;
        let mut words: Vec<&str> = Vec::new();
        for token in raw.split_whitespace() {
            if let Some((key, value)) = token.split_once(':') {
                let key = key.to_lowercase();
                if key == "type" || key == "ext" {
                    if let Some(f) = operator_filter(&key, value) {
                        filter = Some(f);
                        continue;
                    }
                }
            }
            words.push(token);
        }

        // Type words only act as a filter at the end ("sunset photo"), at the start followed by a
        // connector ("screenshot of error"), or as the whole query ("screenshots").
        // "photo editing tutorial" therefore stays a plain query.
        let norm = |w: &str| {
            w.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        };
        let mut removed_type_word: Option<&str> = None;
        if filter.is_none() && !quoted && !words.is_empty() {
            let last = norm(words[words.len() - 1]);
            if let Some(f) = type_word_filter(&last) {
                filter = Some(f);
                if words.len() > 1 {
                    removed_type_word = words.pop();
                    // "photos of" → drop dangling connector too
                    if let Some(prev) = words.last() {
                        if CONNECTORS.contains(&norm(prev).as_str()) && words.len() > 1 {
                            words.pop();
                        }
                    }
                }
            } else if words.len() >= 3 {
                let first = norm(words[0]);
                let second = norm(words[1]);
                if let Some(f) = type_word_filter(&first) {
                    if CONNECTORS.contains(&second.as_str()) {
                        filter = Some(f);
                        removed_type_word = words.drain(0..2).next();
                    }
                }
            }
        }
        // "a screenshot", "my photos": nothing meaningful left, so search for the type word itself.
        if let Some(type_word) = removed_type_word {
            if words.iter().all(|w| FILLERS.contains(&norm(w).as_str())) {
                words = vec![type_word];
            }
        }

        let text = words.join(" ");
        let mut preferred_extensions: Vec<&'static str> = Vec::new();
        for w in &words {
            for ext in language_extensions(&norm(w)) {
                if !preferred_extensions.contains(ext) {
                    preferred_extensions.push(ext);
                }
            }
        }

        let code_like = words.iter().any(|w| looks_like_identifier(w));
        let word_count = words.len();

        Self {
            text,
            filter,
            preferred_extensions,
            code_like,
            quoted,
            word_count,
        }
    }

    pub fn allows_images(&self) -> bool {
        self.filter.as_ref().is_none_or(|f| f.allows_images())
    }

    pub fn allows(&self, extension: &str) -> bool {
        self.filter.as_ref().is_none_or(|f| f.allows(extension))
    }

    /// Multiplier on the image list's fusion weight derived from the query's shape.
    pub fn image_shape_factor(&self) -> f32 {
        if self.filter == Some(TypeFilter::Images) {
            return 1.25;
        }
        if self.code_like {
            return 0.4;
        }
        if self.quoted {
            return 0.5;
        }
        // One- or two-word noun queries ("earth", "red car") are the classic visual query shape.
        if self.word_count == 1 {
            return 1.25;
        }
        if self.word_count == 2 {
            return 1.1;
        }
        if self.word_count >= 6 {
            return 0.75;
        }
        1.0
    }
}

fn looks_like_identifier(word: &str) -> bool {
    let w = word.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == '?');
    if w.contains("::") || w.contains("->") || w.contains("()") || w.contains('/') {
        return true;
    }
    let inner_underscore = w.trim_matches('_').contains('_');
    if inner_underscore {
        return true;
    }
    // camelCase / PascalCase with an inner capital after a lowercase letter
    let chars: Vec<char> = w.chars().collect();
    if chars
        .windows(2)
        .any(|p| p[0].is_lowercase() && p[1].is_uppercase())
    {
        return true;
    }
    // file.ext with a short alphabetic extension
    if let Some((stem, ext)) = w.rsplit_once('.') {
        if !stem.is_empty()
            && (1..=5).contains(&ext.len())
            && ext.chars().all(|c| c.is_ascii_alphanumeric())
            && ext.chars().any(|c| c.is_ascii_alphabetic())
        {
            return true;
        }
    }
    false
}

/// Path fragments that indicate bundled assets rather than the user's own files.
const NOISE_PATH_SEGMENTS: &[&str] = &[
    "/assets/",
    "-assets/",
    "/icons/",
    "/icon/",
    "/themes/",
    "-theme/",
    "/sass/",
    "/scss/",
    "/cursors/",
    "-cursors/",
    "/node_modules/",
    "/vendor/",
    "/site-packages/",
    "/.venv/",
    "/venv/",
    "/build/",
    "/dist/",
    "/__pycache__/",
    "/drawable",
    "/mipmap",
];

/// Ranking multiplier (≤ 1.0) for files that are probably bundled assets or UI sprites.
pub fn noise_factor(path: &str, extension: &str, size_bytes: i64) -> f32 {
    let lower = path.to_lowercase();
    let mut factor = 1.0;
    if NOISE_PATH_SEGMENTS.iter().any(|seg| lower.contains(seg)) {
        factor *= 0.5;
    }
    let ext = extension.to_lowercase();
    if ext == "svg" {
        factor *= 0.6;
    }
    if is_image_extension(&ext) && size_bytes < 8 * 1024 {
        factor *= 0.6;
    }
    factor
}

/// Weighted reciprocal rank fusion. Each list is `(weight, ids in rank order)`; ids may repeat
/// across lists (that is the point) but should be unique within one list.
pub fn weighted_rrf(lists: &[(f32, Vec<String>)]) -> HashMap<String, f32> {
    let mut fused: HashMap<String, f32> = HashMap::new();
    for (weight, ids) in lists {
        if *weight <= 0.0 {
            continue;
        }
        for (rank, id) in ids.iter().enumerate() {
            *fused.entry(id.clone()).or_insert(0.0) += weight / (RRF_K + rank as f32 + 1.0);
        }
    }
    fused
}

/// Fusion weight for a semantic list given how far its best hit stands above the query's corpus.
/// Returns a value in [lo, hi]; `None` stats (small corpus) gets a neutral mid weight.
pub fn strength_weight(top_z: Option<f32>, gate: f32, span: f32, lo: f32, hi: f32) -> f32 {
    match top_z {
        Some(z) => lo + (hi - lo) * clamp01((z - gate) / span),
        None => (lo + hi) / 2.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trailing_type_word_becomes_filter() {
        let i = QueryIntent::parse("sunset photo");
        assert_eq!(i.text, "sunset");
        assert_eq!(i.filter, Some(TypeFilter::Images));

        let i = QueryIntent::parse("resume pdf");
        assert_eq!(i.text, "resume");
        assert!(i.filter.as_ref().unwrap().allows("pdf"));
        assert!(!i.filter.as_ref().unwrap().allows("png"));
    }

    #[test]
    fn leading_type_word_needs_connector() {
        let i = QueryIntent::parse("screenshot of error dialog");
        assert_eq!(i.text, "error dialog");
        assert_eq!(i.filter, Some(TypeFilter::Images));

        let i = QueryIntent::parse("photo editing tutorial");
        assert_eq!(i.text, "photo editing tutorial");
        assert_eq!(i.filter, None);

        let i = QueryIntent::parse("photos of goa");
        assert_eq!(i.text, "goa");
        assert_eq!(i.filter, Some(TypeFilter::Images));
    }

    #[test]
    fn subject_words_are_not_type_words() {
        for q in ["sunset", "earth", "beach", "mountain landscape"] {
            let i = QueryIntent::parse(q);
            assert_eq!(i.text, q);
            assert_eq!(i.filter, None, "{q}");
        }
    }

    #[test]
    fn filler_only_remainder_keeps_type_word() {
        let i = QueryIntent::parse("a screenshot");
        assert_eq!(i.text, "screenshot");
        assert_eq!(i.filter, Some(TypeFilter::Images));

        let i = QueryIntent::parse("my photos");
        assert_eq!(i.text, "photos");
    }

    #[test]
    fn lone_type_word_keeps_text() {
        let i = QueryIntent::parse("screenshots");
        assert_eq!(i.text, "screenshots");
        assert_eq!(i.filter, Some(TypeFilter::Images));
    }

    #[test]
    fn operators_filter_and_are_removed() {
        let i = QueryIntent::parse("type:pdf invoice");
        assert_eq!(i.text, "invoice");
        assert!(i.filter.as_ref().unwrap().allows("pdf"));

        let i = QueryIntent::parse("logo ext:.PNG");
        assert_eq!(i.text, "logo");
        assert!(i.filter.as_ref().unwrap().allows("png"));
        assert!(!i.filter.as_ref().unwrap().allows("jpg"));
    }

    #[test]
    fn code_shape_and_language_preference() {
        assert!(QueryIntent::parse("execute_entanglement_protocol").code_like);
        assert!(QueryIntent::parse("TantivySearchIndex").code_like);
        assert!(QueryIntent::parse("storage.rs").code_like);
        assert!(!QueryIntent::parse("how to reset password").code_like);

        let i = QueryIntent::parse("retry logic in rust");
        assert_eq!(i.preferred_extensions, vec!["rs"]);
        assert_eq!(i.filter, None);
    }

    #[test]
    fn quoted_queries_never_filter() {
        let i = QueryIntent::parse("\"vacation photo\"");
        assert!(i.quoted);
        assert_eq!(i.filter, None);
    }

    #[test]
    fn rrf_rewards_agreement_and_rank() {
        let fused = weighted_rrf(&[
            (1.0, vec!["a".into(), "b".into()]),
            (1.0, vec!["b".into(), "c".into()]),
        ]);
        assert!(fused["b"] > fused["a"]);
        assert!(fused["a"] > fused["c"]);
    }

    #[test]
    fn noise_factor_penalises_theme_assets() {
        assert!(
            noise_factor(
                "/home/u/Downloads/WhiteSur-gtk-theme/src/assets/x.png",
                "png",
                50_000
            ) < 1.0
        );
        assert_eq!(
            noise_factor("/home/u/Documents/images (1).jpeg", "jpeg", 38_113),
            1.0
        );
        assert!(noise_factor("/home/u/Pictures/tiny.png", "png", 3_000) < 1.0);
    }

    #[test]
    fn sim_stats_z() {
        let s = SimStats::from_sums(4, 2.0, 1.1);
        assert!((s.mean - 0.5).abs() < 1e-6);
        assert!(s.z(0.5).abs() < 1e-6);
    }
}
