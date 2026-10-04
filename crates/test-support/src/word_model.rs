//! A stand-in embedding model for tests: each word adds to one of 64
//! dimensions, so texts sharing words are near each other. It knows no
//! meaning and needs no model files; it is enough to test what is done with
//! vectors, and it passes the same conformance suite as the real model.

use catchword_embed::{Embed, EmbedError, ModelManifest, Pooling};
use catchword_engine::{Tokenizer, WordTokenizer};

pub const WORD_MODEL: ModelManifest = ModelManifest {
    name: "test-words",
    revision: "000000000000-test",
    dimensions: 64,
    max_tokens: 512,
    pooling: Pooling::Mean,
    query_prefix: "",
    passage_prefix: "",
    model_sha256: "",
    tokenizer_sha256: "",
};

pub struct WordModel;

impl WordModel {
    fn vector(text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; WORD_MODEL.dimensions];
        for word in text.split_whitespace() {
            let word = word
                .trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase();
            if word.is_empty() {
                continue;
            }
            // FNV-1a: the same word always lands in the same dimension.
            let hash = word.bytes().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
            vector[(hash % WORD_MODEL.dimensions as u64) as usize] += 1.0;
        }
        let length = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
        if length == 0.0 {
            // No words: still a vector of unit length.
            vector[0] = 1.0;
            return vector;
        }
        vector.iter().map(|x| x / length).collect()
    }
}

impl Embed for WordModel {
    fn manifest(&self) -> &ModelManifest {
        &WORD_MODEL
    }

    fn tokenizer(&self) -> Box<dyn Tokenizer + Send + Sync> {
        Box::new(WordTokenizer)
    }

    fn embed_passages(&mut self, passages: &[&str]) -> Result<Vec<Vec<f32>>, EmbedError> {
        Ok(passages.iter().map(|text| Self::vector(text)).collect())
    }

    fn embed_query(&mut self, query: &str) -> Result<Vec<f32>, EmbedError> {
        Ok(Self::vector(query))
    }
}
