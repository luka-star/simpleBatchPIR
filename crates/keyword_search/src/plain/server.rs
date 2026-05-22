use shared::keyword::{build_keyword_index, KeywordClosure, KeywordIndex, RecordId};
use simplepir::SimplePIRServer;
use std::collections::HashMap;

use super::types::{PlainKeywordAnswer, PlainKeywordQuery};

pub struct PlainKeywordServer {
    pub index: KeywordIndex,
    pub pir: SimplePIRServer,
}

impl PlainKeywordServer {
    pub fn setup(mapping: &HashMap<String, Vec<RecordId>>) -> Self {
        let index = build_keyword_index(mapping);
        let pir = SimplePIRServer::setup(index.matrix.clone());

        Self { index, pir }
    }

    pub fn closure(&self) -> KeywordClosure {
        self.index.closure()
    }

    pub fn answer(&self, query: &PlainKeywordQuery) -> PlainKeywordAnswer {
        self.pir.answer(query)
    }
}
