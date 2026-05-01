use oprf::{keygen, MockOprf, DEFAULT_M};
use rand_08::thread_rng;
use shared::keyword::{build_secure_keyword_index, SecureKeywordClosure, SecureKeywordIndex};
use shared::models::Band;

use super::plain::{setup, SetupResult};

pub struct SecureKeywordSetup {
    pub oprf: MockOprf,
    pub keyword_index: SecureKeywordIndex,
    pub keyword_closure: SecureKeywordClosure,
    pub setup_result: SetupResult,
}

pub fn build_secure_keyword_setup(db: &[Band]) -> SecureKeywordSetup {
    let mut rng = thread_rng();
    let oprf_key = keygen(DEFAULT_M, &mut rng);
    let keyword_index = build_secure_keyword_index(db, &oprf_key);
    let keyword_closure = keyword_index.closure();
    let setup_result = setup(&keyword_index.matrix);
    let oprf = MockOprf::new(oprf_key);

    SecureKeywordSetup {
        oprf,
        keyword_index,
        keyword_closure,
        setup_result,
    }
}
