//! The stand-in model meets the same contract as the real one (MNT-2).

use catchword_test_support::{conformance, WordModel};

#[test]
fn the_stand_in_model_meets_the_embedding_contract() {
    conformance::embedder(&mut WordModel);
}
