//! Owned runtime query data; no live-reference, snapshot or library authority.
//!
//! Runtime normalization is one source-defined pass. Saved-query decoding,
//! migration and payload serialization have their separate existing contracts.

use crate::collection_codec::{self, Query};

/// Mutable request data. Neither this type nor serializing its criteria proves
/// that runtime normalization or live database validation has occurred.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct QueryRequest {
    pub criteria: Query,
    pub page: i64,
    pub page_size: i64,
    pub sort: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum QueryInputError {
    #[error("story library query is invalid")]
    InvalidQuery,
    #[error("frozen query compatibility invariant failed")]
    FrozenData,
    #[error("runtime query buffer bounds exceed representable capacity")]
    ResourceOverflow,
}

/// Scalar result data, not a snapshot, write acknowledgement or close receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Count {
    pub total_items: i64,
}

/// Domain refusal data for a later transaction-local query participant.
/// Storage, cancellation, admission and resource failures remain independent.
#[derive(Clone, Copy, Debug, Eq, PartialEq, thiserror::Error)]
pub enum QueryDomainError {
    #[error("story library query is invalid")]
    InvalidQuery,
}

/// Consume an owned request and apply exactly one frozen runtime pass.
///
/// This validates/defaults navigation even when the caller will only count.
/// It does not check live definition kinds, value ownership, catalog state or
/// revisions. A later Count invocation must call this once on its accounted
/// owned working copy, without trusting a prior caller normalization flag.
/// Checked buffer-bound overflow is a resource refusal, not invalid input;
/// this API supplies no total-heap bound or graceful out-of-memory guarantee.
pub fn normalize_request(request: QueryRequest) -> Result<QueryRequest, QueryInputError> {
    collection_codec::runtime::normalize_request(request)
}
