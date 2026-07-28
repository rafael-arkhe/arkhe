use qdrant_client::qdrant::{value::Kind, SearchParams, SearchPoints};
use qdrant_client::Qdrant;

use crate::error::IdentityError;
use crate::types::DidDocument;

pub struct QdrantDidResolver {
    client: Qdrant,
    collection: String,
}

impl QdrantDidResolver {
    pub async fn new(url: &str, collection: &str) -> Result<Self, IdentityError> {
        let client = Qdrant::from_url(url)
            .build()
            .map_err(|e| IdentityError::Qdrant(format!("connect: {}", e)))?;
        Ok(Self {
            client,
            collection: collection.to_string(),
        })
    }

    pub async fn resolve(&self, did: &str) -> Result<DidDocument, IdentityError> {
        let response = self
            .client
            .search_points(SearchPoints {
                collection_name: self.collection.clone(),
                vector: self.did_to_vector(did),
                limit: 1,
                with_payload: Some(true.into()),
                params: Some(SearchParams {
                    exact: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await
            .map_err(|e| IdentityError::Qdrant(format!("search: {}", e)))?;

        let point = response
            .result
            .into_iter()
            .next()
            .ok_or_else(|| IdentityError::DidResolution("DID not found".into()))?;

        let doc_json = match point.payload.get("did_document").and_then(|v| v.kind.as_ref()) {
            Some(Kind::StringValue(s)) => s.clone(),
            _ => {
                return Err(IdentityError::DidResolution(
                    "missing document payload".into(),
                ))
            }
        };

        serde_json::from_str(&doc_json)
            .map_err(|e| IdentityError::DidResolution(format!("json decode: {}", e)))
    }

    fn did_to_vector(&self, did: &str) -> Vec<f32> {
        use sha3::{Digest, Sha3_256};
        let hash = Sha3_256::digest(did.as_bytes());
        hash.iter().map(|&b| b as f32 / 255.0).collect()
    }
}
