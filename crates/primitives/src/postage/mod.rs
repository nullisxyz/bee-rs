use std::collections::{HashMap, HashSet};

use alloy::primitives::Bytes;
use nectar_primitives_traits::{
    AuthError, AuthProof, AuthResult, Authorizer, Chunk, ChunkAddress, Reserved,
    TimeBoundAuthorizer, Timestamp,
};

/// Batch identifier
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct BatchId([u8; 32]);

/// Postage stamp proof
#[derive(Clone)]
pub struct PostageProof {
    batch_id: BatchId,
    stamp_index: u32,
    timestamp: Timestamp,
    signature: PrimitiveSignature,
}

impl AuthProof for PostageProof {
    fn proof_data(&self) -> &Bytes {
        // Return serialized proof data
    }
}

/// Information about a stamp batch
struct BatchInfo {
    /// When the batch expires
    expires_at: Timestamp,
    /// Total depth (2^depth = total stamps available)
    depth: u8,
    /// Amount paid per chunk
    amount_per_chunk: u64,
    /// Set of used stamp indices
    used_stamps: HashSet<u32>,
    /// Whether batch is immutable
    immutable: bool,
}

impl BatchInfo {
    fn is_valid(&self) -> bool {
        !self.used_stamps.len() >= self.max_stamps()
    }

    fn max_stamps(&self) -> usize {
        1 << self.depth
    }

    fn is_stamp_used(&self, index: u32) -> bool {
        self.used_stamps.contains(&index)
    }

    fn use_stamp(&mut self, index: u32) -> bool {
        if index as usize >= self.max_stamps() {
            return false;
        }
        self.used_stamps.insert(index)
    }
}

/// Maps chunk addresses to their stamp authorizations
#[derive(Default)]
struct ChunkAuthorizations {
    /// Maps chunks to (batch_id, stamp_index) pairs
    authorizations: HashMap<ChunkAddress, HashSet<(BatchId, u32)>>,
    /// Total count of authorizations
    total_count: u64,
}

impl ChunkAuthorizations {
    fn add(&mut self, chunk: ChunkAddress, batch_id: BatchId, stamp_index: u32) {
        if self
            .authorizations
            .entry(chunk)
            .or_default()
            .insert((batch_id, stamp_index))
        {
            self.total_count += 1;
        }
    }

    fn remove_batch(&mut self, batch_id: &BatchId) -> u64 {
        let mut removed = 0;
        self.authorizations.retain(|_, auths| {
            let before_len = auths.len();
            auths.retain(|(bid, _)| bid != batch_id);
            removed += before_len - auths.len();
            !auths.is_empty()
        });
        self.total_count -= removed;
        removed as u64
    }
}

pub struct PostageAuthorizer {
    /// Active batches
    batches: HashMap<BatchId, BatchInfo>,
    /// Chunk authorizations
    chunk_auths: ChunkAuthorizations,
}

impl PostageAuthorizer {
    pub fn new() -> Self {
        Self {
            batches: HashMap::new(),
            chunk_auths: ChunkAuthorizations::default(),
        }
    }

    /// Add a new batch
    pub fn add_batch(
        &mut self,
        id: BatchId,
        depth: u8,
        expires_at: Timestamp,
        amount_per_chunk: u64,
        immutable: bool,
    ) -> AuthResult<()> {
        if self.batches.contains_key(&id) {
            return Err(AuthError::InvalidState("batch already exists"));
        }

        self.batches.insert(
            id,
            BatchInfo {
                expires_at,
                depth,
                amount_per_chunk,
                used_stamps: HashSet::new(),
                immutable,
            },
        );

        Ok(())
    }
}

impl Authorizer for PostageAuthorizer {
    type Proof = PostageProof;

    fn authorized_chunk_count(&self) -> u64 {
        self.chunk_auths.total_count
    }

    fn validate(&self, chunk: &impl Chunk, proof: &Self::Proof) -> AuthResult<()> {
        let batch = self
            .batches
            .get(&proof.batch_id)
            .ok_or(AuthError::InvalidProof("batch not found"))?;

        // Check batch validity
        if batch.expires_at <= proof.timestamp {
            return Err(AuthError::Expired);
        }

        // Verify stamp hasn't been used
        if batch.is_stamp_used(proof.stamp_index) {
            return Err(AuthError::InvalidProof("stamp already used"));
        }

        // Verify stamp index is within batch depth
        if proof.stamp_index as usize >= batch.max_stamps() {
            return Err(AuthError::InvalidProof("invalid stamp index"));
        }

        // Verify proof signature
        proof.verify_signature().map_err(AuthError::Crypto)?;

        Ok(())
    }
}

impl TimeBoundAuthorizer for PostageAuthorizer {
    fn cleanup_expired(&mut self, now: Timestamp) -> AuthResult<u64> {
        let expired_batches: Vec<BatchId> = self
            .batches
            .iter()
            .filter(|(_, info)| info.expires_at <= now)
            .map(|(id, _)| id.clone())
            .collect();

        let mut total_cleaned = 0;
        for batch_id in expired_batches {
            self.batches.remove(&batch_id);
            total_cleaned += self.chunk_auths.remove_batch(&batch_id);
        }

        Ok(total_cleaned)
    }
}

impl Reserved for PostageAuthorizer {
    fn reserved_chunks(&self) -> u64 {
        self.batches
            .values()
            .map(|batch| batch.max_stamps() as u64)
            .sum()
    }

    fn available_chunks(&self) -> u64 {
        self.reserved_chunks() - self.authorized_chunk_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_expiry() {
        let mut auth = PostageAuthorizer::new();

        // Add batch that expires at t=100
        auth.add_batch(
            BatchId([0; 32]),
            8,    // depth
            100,  // expires_at
            1000, // amount per chunk
            true, // immutable
        )
        .unwrap();

        // Cleanup at t=50 should do nothing
        assert_eq!(auth.cleanup_expired(50).unwrap(), 0);

        // Cleanup at t=150 should remove the batch
        assert_eq!(auth.cleanup_expired(150).unwrap(), 0);
        assert!(auth.batches.is_empty());
    }
}
