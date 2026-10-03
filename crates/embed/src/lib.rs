//! Catchword embedding runtime: turns passages and queries into vectors with
//! a local model, through ONNX Runtime.
//!
//! Nothing is downloaded here. The runtime library and the model come from
//! `scripts/fetch-embedding.sh` (or, later, the installer), and the model's
//! checksums are checked every time it is loaded (ADR-18).

use std::fmt;
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use catchword_engine::Tokenizer;
use ort::session::Session;
use ort::value::Tensor;
use sha2::{Digest, Sha256};

/// What the engine needs to know about a model. Shipped with the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelManifest {
    /// The model's folder under `models/`.
    pub name: &'static str,
    /// The exact upstream revision the files come from.
    pub revision: &'static str,
    pub dimensions: usize,
    /// Inputs are cut to this many tokens. Passages are far shorter; this
    /// only stops one enormous "word" from costing minutes.
    pub max_tokens: usize,
    pub query_prefix: &'static str,
    pub passage_prefix: &'static str,
    pub model_sha256: &'static str,
    pub tokenizer_sha256: &'static str,
}

impl ModelManifest {
    /// Names the model and revision. Vectors from different models cannot be
    /// compared, so the index records this.
    pub fn id(&self) -> String {
        format!("{}@{}", self.name, &self.revision[..12])
    }
}

/// The provisional model (ADR-6, ADR-18). Its embedding is the vector of the
/// first token (CLS pooling), compared by cosine similarity, with no prefixes.
pub const GRANITE_97M: ModelManifest = ModelManifest {
    name: "granite-embedding-97m-multilingual-r2",
    revision: "835ad14087e140460703cf0fae09f97d469d65c2",
    dimensions: 384,
    max_tokens: 1024,
    query_prefix: "",
    passage_prefix: "",
    model_sha256: "a6022dd8220ea6f6595562a1328ee216f4a94faa55362f2f4747c80f1e78772e",
    tokenizer_sha256: "4f2842d568e2724370aec203652a42ac783c7937f8347a1a2cc7506d71f1582f",
};

#[derive(Debug)]
pub enum EmbedError {
    /// A file is missing: run scripts/fetch-embedding.sh, or reinstall.
    Missing(PathBuf),
    /// A model file is not the pinned one: refused, as damaged or tampered.
    Checksum(PathBuf),
    Runtime(String),
    Tokenizer(String),
}

impl fmt::Display for EmbedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EmbedError::Missing(path) => write!(f, "missing {}", path.display()),
            EmbedError::Checksum(path) => write!(
                f,
                "{} does not match its pinned checksum and was not loaded",
                path.display()
            ),
            EmbedError::Runtime(message) => write!(f, "ONNX Runtime: {message}"),
            EmbedError::Tokenizer(message) => write!(f, "tokenizer: {message}"),
        }
    }
}

impl std::error::Error for EmbedError {}

impl From<ort::Error> for EmbedError {
    fn from(error: ort::Error) -> Self {
        EmbedError::Runtime(error.to_string())
    }
}

/// Where the runtime library and a model's files are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paths {
    pub runtime: PathBuf,
    pub model: PathBuf,
}

/// Look for the runtime and the model next to this program, where an
/// installed app keeps them, and in debug builds also in the source tree's
/// vendor/ folder. Only these known places are tried, by full path: Windows
/// ships an older onnxruntime.dll of its own, which must never be picked up.
pub fn find(manifest: &ModelManifest) -> Option<Paths> {
    let library = format!(
        "{}onnxruntime{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    );
    let mut places = Vec::new();
    if let Some(folder) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        places.push(Paths {
            runtime: folder.join(&library),
            model: folder.join("models").join(manifest.name),
        });
    }
    if cfg!(debug_assertions) {
        let vendor = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vendor");
        places.push(Paths {
            runtime: vendor.join("onnxruntime").join(&library),
            model: vendor.join("models").join(manifest.name),
        });
    }
    places
        .into_iter()
        .find(|paths| paths.runtime.is_file() && paths.model.is_dir())
}

/// A loaded model, ready to embed text.
pub struct Embedder {
    session: Session,
    tokenizer: tokenizers::Tokenizer,
    manifest: ModelManifest,
}

impl Embedder {
    /// Check the model files against their pinned checksums, then load them.
    pub fn load(paths: &Paths, manifest: &ModelManifest) -> Result<Self, EmbedError> {
        let model = paths.model.join("model.onnx");
        let tokenizer = paths.model.join("tokenizer.json");
        verify(&model, manifest.model_sha256)?;
        verify(&tokenizer, manifest.tokenizer_sha256)?;
        start_runtime(&paths.runtime)?;
        let session = Session::builder()?.commit_from_file(&model)?;
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer)
            .map_err(|error| EmbedError::Tokenizer(error.to_string()))?;
        Ok(Self {
            session,
            tokenizer,
            manifest: *manifest,
        })
    }

    pub fn manifest(&self) -> &ModelManifest {
        &self.manifest
    }

    /// One unit-length vector per passage, in order.
    pub fn embed_passages(&mut self, passages: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        let prefix = self.manifest.passage_prefix;
        let texts: Vec<String> = passages.iter().map(|p| format!("{prefix}{p}")).collect();
        self.embed(texts)
    }

    /// The vector for a search query.
    pub fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, EmbedError> {
        let text = format!("{}{query}", self.manifest.query_prefix);
        Ok(self.embed(vec![text])?.remove(0))
    }

    /// Embed each text on its own. The 8-bit model sets its rounding scale
    /// from everything in one run, so a passage run together with others, or
    /// padded to their length, gets a different vector. Measured on
    /// 3 October 2026: batching was no faster (ADR-18).
    fn embed(&mut self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, EmbedError> {
        texts.iter().map(|text| self.embed_one(text)).collect()
    }

    fn embed_one(&mut self, text: &str) -> Result<Vec<f32>, EmbedError> {
        let encoding = self
            .tokenizer
            .encode(text, true)
            .map_err(|error| EmbedError::Tokenizer(error.to_string()))?;
        let mut ids: Vec<i64> = encoding.get_ids().iter().map(|&id| i64::from(id)).collect();
        // Keep the start, where the pooled first token is, and the end marker.
        if ids.len() > self.manifest.max_tokens {
            let end_marker = ids[ids.len() - 1];
            ids.truncate(self.manifest.max_tokens - 1);
            ids.push(end_marker);
        }
        let length = ids.len();
        let outputs = self.session.run(ort::inputs![
            "input_ids" => Tensor::from_array(([1, length], ids))?,
            "attention_mask" => Tensor::from_array(([1, length], vec![1i64; length]))?,
        ])?;
        let (shape, values) = outputs[0].try_extract_tensor::<f32>()?;
        let dimensions = self.manifest.dimensions;
        if shape.len() != 3 || shape[2] as usize != dimensions {
            return Err(EmbedError::Runtime(format!(
                "unexpected output shape {shape:?}"
            )));
        }
        // The first token's vector is the embedding (CLS pooling).
        Ok(unit_length(&values[..dimensions]))
    }
}

impl Tokenizer for Embedder {
    fn token_ends(&self, text: &str) -> Vec<usize> {
        match self.tokenizer.encode(text, false) {
            Ok(encoding) => encoding.get_offsets().iter().map(|&(_, end)| end).collect(),
            Err(_) => Vec::new(),
        }
    }
}

fn unit_length(vector: &[f32]) -> Vec<f32> {
    let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if length == 0.0 {
        return vector.to_vec();
    }
    vector.iter().map(|x| x / length).collect()
}

/// Refuse a file whose SHA-256 is not `expected`.
fn verify(path: &Path, expected: &str) -> Result<(), EmbedError> {
    let mut file = File::open(path).map_err(|_| EmbedError::Missing(path.to_path_buf()))?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher).map_err(|_| EmbedError::Missing(path.to_path_buf()))?;
    let actual: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual == expected {
        Ok(())
    } else {
        Err(EmbedError::Checksum(path.to_path_buf()))
    }
}

/// Load ONNX Runtime from `library`, once per process.
fn start_runtime(library: &Path) -> Result<(), EmbedError> {
    static STARTED: OnceLock<Result<(), String>> = OnceLock::new();
    STARTED
        .get_or_init(|| {
            if !library.is_file() {
                return Err(format!("missing {}", library.display()));
            }
            ort::init_from(library)
                .map(|builder| {
                    builder.commit();
                })
                .map_err(|error| error.to_string())
        })
        .clone()
        .map_err(EmbedError::Runtime)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_with_the_wrong_checksum_is_refused() {
        let file = std::env::temp_dir().join("catchword-embed-test-checksum.bin");
        std::fs::write(&file, b"not the model").unwrap();
        let wrong = "0".repeat(64);
        assert!(matches!(
            verify(&file, &wrong),
            Err(EmbedError::Checksum(_))
        ));
        let right: String = Sha256::digest(b"not the model")
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert!(verify(&file, &right).is_ok());
        std::fs::remove_file(&file).unwrap();
        assert!(matches!(verify(&file, &wrong), Err(EmbedError::Missing(_))));
    }

    #[test]
    fn vectors_are_scaled_to_unit_length() {
        let vector = unit_length(&[3.0, 4.0]);
        assert_eq!(vector, vec![0.6, 0.8]);
        assert_eq!(unit_length(&[0.0, 0.0]), vec![0.0, 0.0]);
    }
}
