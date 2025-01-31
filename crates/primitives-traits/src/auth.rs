use alloy::primitives::SignatureError;
use bytes::Bytes;
use std::io;
use thiserror::Error;

use crate::Chunk;

/// Timestamp type for time-based operations
pub type Timestamp = u64;

/// Core authorization proof trait
pub trait AuthProof: Send + Sync {
    /// Get the raw proof data
    fn proof_data(&self) -> &Bytes;
}

/// Core trait for authorization validation
pub trait Authorizer: Send + Sync {
    type Proof: AuthProof;

    /// Get total number of chunks this authorizer has authorized
    fn authorized_chunk_count(&self) -> u64;

    /// Validate a proof for a chunk
    fn validate(&self, chunk: &impl Chunk, proof: &Self::Proof) -> AuthResult<()>;
}

/// Trait for time-bound authorizations that can expire
pub trait TimeBoundAuthorizer: Authorizer {
    /// Remove expired authorizations and return count of removed items
    fn cleanup_expired(&mut self, now: Timestamp) -> AuthResult<u64>;
}

/// Trait for authorizers that manage reserved storage
pub trait Reserved: Authorizer {
    /// Get total chunks reserved
    fn reserved_chunks(&self) -> u64;

    /// Get remaining chunk capacity
    fn available_chunks(&self) -> u64;

    /// Check if can reserve more chunks
    fn can_authorize(&self) -> bool {
        self.available_chunks() > 0
    }
}

/// Trait for proof generation
pub trait AuthProofGenerator: Send + Sync {
    type Proof: AuthProof;

    /// Generate a proof for a chunk
    fn generate_proof(&self, chunk: &impl Chunk) -> AuthResult<Self::Proof>;
}

/// Authorization-specific errors
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid proof: {0}")]
    InvalidProof(&'static str),

    #[error("Proof expired")]
    Expired,

    #[error("Authorization capacity exceeded")]
    CapacityExceeded,

    #[error("Invalid state: {0}")]
    InvalidState(&'static str),

    #[error("Crypto error: {0}")]
    Crypto(#[from] SignatureError),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Type alias for Result with AuthError
pub type AuthResult<T> = std::result::Result<T, AuthError>;

// Helper methods for error creation
impl AuthError {
    pub fn invalid_proof(msg: &'static str) -> Self {
        Self::InvalidProof(msg)
    }

    pub fn invalid_state(msg: &'static str) -> Self {
        Self::InvalidState(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Test helpers and mocks would go here

    #[test]
    fn test_reserved_default_behavior() {
        // Test the default implementation of can_authorize
    }
}
