use shared::keyword::{
    build_keyword_database, KeywordClientContext, KeywordDatabase, RecordIdxList,
};
use simplepir::{
    types::{SimplePIRRecordAnswer, SimplePIRRecordQuery},
    SimplePIRServer,
};
use std::collections::HashMap;

pub struct PlainKeywordServer {
    pub keyword_database: KeywordDatabase,
    pub pir: SimplePIRServer,
}

impl PlainKeywordServer {
    pub fn setup(mapping: &HashMap<String, RecordIdxList>) -> Self {
        let keyword_database = build_keyword_database(mapping);
        let pir = SimplePIRServer::setup(keyword_database.matrix.clone());

        Self {
            keyword_database,
            pir,
        }
    }

    pub fn client_context(&self) -> KeywordClientContext {
        self.keyword_database.client_context()
    }

    pub fn answer(&self, query: &SimplePIRRecordQuery) -> SimplePIRRecordAnswer {
        self.pir.answer(query)
    }
}
