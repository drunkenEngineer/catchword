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
use std::sync::{Arc, OnceLock};

use catchword_engine::Tokenizer;
use ort::environment::ThreadManager;
use ort::session::Session;
use ort::value::Tensor;
use sha2::{Digest, Sha256};

/// How the runtime may use the processor.
#[derive(Debug, Clone, Copy)]
pub struct Threads {
    /// Threads for one embedding, the calling one included. 0 lets the
    /// runtime choose: one per physical core.
    pub count: usize,
    /// For work in the background: called first on each thread the runtime
    /// starts, to lower its priority (RSC-2), and threads wait without
    /// spinning between pieces of work.
    pub background: Option<fn()>,
}

impl Threads {
    /// The runtime's own choice, for the command-line tool and benchmarks.
    pub const RUNTIME_DEFAULT: Threads = Threads {
        count: 0,
        background: None,
    };
}

/// Starts the runtime's threads, calling a function on each one first.
struct BackgroundThreads(fn());

impl ThreadManager for BackgroundThreads {
    type Thread = std::thread::JoinHandle<()>;

    fn create(&self, work: impl FnOnce() + Send + 'static) -> ort::Result<Self::Thread> {
        let first = self.0;
        Ok(std::thread::spawn(move || {
            first();
            work();
        }))
    }

    fn join(thread: Self::Thread) -> ort::Result<()> {
        let _ = thread.join();
        Ok(())
    }
}

/// How a model turns its per-token output into one vector.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pooling {
    /// The first token's vector.
    Cls,
    /// The average of all tokens' vectors.
    Mean,
}

/// What the engine needs to know about a model. Shipped with the app.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModelManifest {
    /// The model's folder under `models/`.
    pub name: &'static str,
    /// The exact upstream revision the files come from.
    pub revision: &'static str,
    pub dimensions: usize,
    /// Inputs are cut to this many tokens, special tokens included: the
    /// model's own limit, or less where the model allows far more, so one
    /// enormous "word" cannot cost minutes.
    pub max_tokens: usize,
    pub pooling: Pooling,
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
    pooling: Pooling::Cls,
    query_prefix: "",
    passage_prefix: "",
    model_sha256: "a6022dd8220ea6f6595562a1328ee216f4a94faa55362f2f4747c80f1e78772e",
    tokenizer_sha256: "4f2842d568e2724370aec203652a42ac783c7937f8347a1a2cc7506d71f1582f",
};

/// The model the app uses, kept after the benchmark (ADR-20).
pub const DEFAULT_MODEL: ModelManifest = GRANITE_97M;

/// The baseline the provisional model must beat (ADR-6). It averages all
/// token vectors, needs "query: " and "passage: " prefixes, and reads at
/// most 512 tokens. Fetched by scripts/fetch-eval.sh, for the benchmark.
pub const E5_SMALL: ModelManifest = ModelManifest {
    name: "multilingual-e5-small",
    revision: "614241f622f53c4eeff9890bdc4f31cfecc418b3",
    dimensions: 384,
    max_tokens: 512,
    pooling: Pooling::Mean,
    query_prefix: "query: ",
    passage_prefix: "passage: ",
    model_sha256: "dd476dd0c2514e9b9be83aeb3853fac0763e0bdf4a71645407587d77c48a2d88",
    tokenizer_sha256: "0b44a9d7b51c3c62626640cda0e2c2f70fdacdc25bbbd68038369d14ebdf4c39",
};

/// The file name of the ONNX Runtime library on this system.
pub fn runtime_library_name() -> String {
    format!(
        "{}onnxruntime{}",
        std::env::consts::DLL_PREFIX,
        std::env::consts::DLL_SUFFIX
    )
}

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

/// A failed session setting; the half-built session is not needed back.
impl From<ort::Error<ort::session::builder::SessionBuilder>> for EmbedError {
    fn from(error: ort::Error<ort::session::builder::SessionBuilder>) -> Self {
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
    let library = runtime_library_name();
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
    tokenizer: ModelTokenizer,
    manifest: ModelManifest,
    /// BERT-style models also take a "segment" input, all zeros here.
    needs_token_types: bool,
}

impl Embedder {
    /// Check the model files against their pinned checksums, then load them.
    pub fn load(
        paths: &Paths,
        manifest: &ModelManifest,
        threads: Threads,
    ) -> Result<Self, EmbedError> {
        let model = paths.model.join("model.onnx");
        let tokenizer = paths.model.join("tokenizer.json");
        verify(&model, manifest.model_sha256)?;
        verify(&tokenizer, manifest.tokenizer_sha256)?;
        start_runtime(&paths.runtime)?;
        let mut builder = Session::builder()?.with_intra_threads(threads.count)?;
        if let Some(first) = threads.background {
            builder = builder
                .with_intra_op_spinning(false)?
                .with_thread_manager(BackgroundThreads(first))?;
        }
        let session = builder.commit_from_file(&model)?;
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer)
            .map_err(|error| EmbedError::Tokenizer(error.to_string()))?;
        let tokenizer = ModelTokenizer(Arc::new(tokenizer));
        let needs_token_types = session
            .inputs()
            .iter()
            .any(|input| input.name() == "token_type_ids");
        Ok(Self {
            session,
            tokenizer,
            manifest: *manifest,
            needs_token_types,
        })
    }

    pub fn manifest(&self) -> &ModelManifest {
        &self.manifest
    }

    /// The model's tokenizer, to size passages. It can be used on another
    /// thread while this model embeds: splitting text never waits for it.
    pub fn tokenizer(&self) -> ModelTokenizer {
        self.tokenizer.clone()
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
            .0
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
        let mut inputs = ort::inputs![
            "input_ids" => Tensor::from_array(([1, length], ids))?,
            "attention_mask" => Tensor::from_array(([1, length], vec![1i64; length]))?,
        ];
        if self.needs_token_types {
            let types = Tensor::from_array(([1, length], vec![0i64; length]))?;
            inputs.push(("token_type_ids".into(), types.into()));
        }
        let outputs = self.session.run(inputs)?;
        let (shape, values) = outputs[0].try_extract_tensor::<f32>()?;
        let dimensions = self.manifest.dimensions;
        if shape.len() != 3 || shape[1] as usize != length || shape[2] as usize != dimensions {
            return Err(EmbedError::Runtime(format!(
                "unexpected output shape {shape:?}"
            )));
        }
        Ok(unit_length(&pool(
            values,
            dimensions,
            self.manifest.pooling,
        )))
    }
}

/// One vector from a model's output, a row of `dimensions` numbers per token.
fn pool(values: &[f32], dimensions: usize, pooling: Pooling) -> Vec<f32> {
    match pooling {
        Pooling::Cls => values[..dimensions].to_vec(),
        Pooling::Mean => {
            let tokens = values.len() / dimensions;
            let mut sum = vec![0f32; dimensions];
            for token in values.chunks_exact(dimensions) {
                for (total, value) in sum.iter_mut().zip(token) {
                    *total += value;
                }
            }
            sum.iter().map(|total| total / tokens as f32).collect()
        }
    }
}

impl Tokenizer for Embedder {
    fn token_ends(&self, text: &str) -> Vec<usize> {
        self.tokenizer.token_ends(text)
    }
}

/// A model's tokenizer, shared: copies are cheap and point to the same one.
#[derive(Clone)]
pub struct ModelTokenizer(Arc<tokenizers::Tokenizer>);

impl Tokenizer for ModelTokenizer {
    fn token_ends(&self, text: &str) -> Vec<usize> {
        match self.0.encode(text, false) {
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
    fn pooling_takes_the_first_token_or_the_average() {
        // Two tokens of three numbers each.
        let output = [1.0, 2.0, 3.0, 3.0, 4.0, 5.0];
        assert_eq!(pool(&output, 3, Pooling::Cls), vec![1.0, 2.0, 3.0]);
        assert_eq!(pool(&output, 3, Pooling::Mean), vec![2.0, 3.0, 4.0]);
    }

    #[test]
    fn vectors_are_scaled_to_unit_length() {
        let vector = unit_length(&[3.0, 4.0]);
        assert_eq!(vector, vec![0.6, 0.8]);
        assert_eq!(unit_length(&[0.0, 0.0]), vec![0.0, 0.0]);
    }
}
