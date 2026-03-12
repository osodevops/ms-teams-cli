use serde::{Deserialize, Serialize};

/// POST body for /search/query
#[derive(Debug, Clone, Serialize)]
pub struct SearchRequest {
    pub requests: Vec<SearchQueryRequest>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryRequest {
    pub entity_types: Vec<String>,
    pub query: SearchQueryString,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchQueryString {
    pub query_string: String,
}

impl SearchRequest {
    pub fn new(entity_type: &str, query: &str, size: u64) -> Self {
        Self {
            requests: vec![SearchQueryRequest {
                entity_types: vec![entity_type.to_string()],
                query: SearchQueryString {
                    query_string: query.to_string(),
                },
                from: Some(0),
                size: Some(size),
            }],
        }
    }
}

/// Response from /search/query
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    pub value: Vec<SearchResultSet>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultSet {
    pub search_terms: Option<Vec<String>>,
    pub hits_containers: Option<Vec<HitsContainer>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HitsContainer {
    pub total: Option<u64>,
    pub more_results_available: Option<bool>,
    pub hits: Option<Vec<SearchHit>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchHit {
    pub hit_id: Option<String>,
    pub rank: Option<u64>,
    pub summary: Option<String>,
    pub resource: Option<serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_request_serializes() {
        let req = SearchRequest::new("chatMessage", "hello", 10);
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["requests"][0]["entityTypes"][0], "chatMessage");
        assert_eq!(json["requests"][0]["query"]["queryString"], "hello");
        assert_eq!(json["requests"][0]["size"], 10);
    }

    #[test]
    fn search_response_deserializes() {
        let json = r#"{
            "value": [{
                "searchTerms": ["test"],
                "hitsContainers": [{
                    "total": 1,
                    "moreResultsAvailable": false,
                    "hits": [{
                        "hitId": "hit-1",
                        "rank": 1,
                        "summary": "test hit",
                        "resource": {"id": "r-1"}
                    }]
                }]
            }]
        }"#;
        let resp: SearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.value.len(), 1);
        let hits = resp.value[0].hits_containers.as_ref().unwrap();
        assert_eq!(
            hits[0].hits.as_ref().unwrap()[0].hit_id.as_deref(),
            Some("hit-1")
        );
    }
}
