use alloy_primitives::{Address, Keccak256, PrimitiveSignature, B256};
use alloy_signer::Signer;
use swarm_primitives_traits::{Chunk, ChunkAddress, ChunkBody, ChunkDecoding, ChunkEncoding, Span};

use super::bmt_body::{BMTBody, BMTBodyError};

const ID_SIZE: usize = std::mem::size_of::<B256>();
const SIGNATURE_SIZE: usize = std::mem::size_of::<PrimitiveSignature>();
const MIN_SOC_FIELDS_SIZE: usize = ID_SIZE + SIGNATURE_SIZE;

#[derive(Debug, thiserror::Error)]
pub enum SingleOwnerChunkError {
    #[error("BMTBody error: {0}")]
    BMTBodyError(#[from] BMTBodyError),
    #[error("Recovered owner mismatch")]
    AlloySignerError(#[from] alloy_signer::Error),
    #[error("Recovered chunk mismatch, expected address: {address}, recovered address {recovered_chunk_address} with recovered owner {recovered_owner}")]
    ChunkMismatch {
        address: ChunkAddress,
        recovered_chunk_address: ChunkAddress,
        recovered_owner: Address,
    },
    #[error("Data too small ({min_size} bytes), got {actual_size} bytes")]
    InsufficientData { min_size: usize, actual_size: usize },
}

#[derive(Debug, Eq, PartialEq)]
pub struct SingleOwnerChunk {
    id: B256,
    owner: Address,
    signature: PrimitiveSignature,
    body: BMTBody,
}

impl SingleOwnerChunk {
    pub async fn new(
        id: B256,
        data: Vec<u8>,
        signer: impl Signer,
    ) -> Result<Self, SingleOwnerChunkError> {
        let body = BMTBody::new(data.len() as Span, data)?;
        let hash = Self::to_sign(id, &body).await;
        let signature = signer.sign_hash(&hash).await?;

        Ok(Self {
            id,
            owner: signer.address(),
            signature,
            body,
        })
    }

    pub async fn new_signed(
        address: ChunkAddress,
        id: B256,
        signature: PrimitiveSignature,
        data: Vec<u8>,
    ) -> Result<Self, SingleOwnerChunkError> {
        let body = BMTBody::new(data.len() as Span, data)?;
        let hash = Self::to_sign(id, &body).await;
        let recovered_owner = signature.recover_address_from_msg(&hash).unwrap();

        let chunk = Self {
            id,
            owner: recovered_owner,
            signature,
            body,
        };

        let recovered_chunk_address = chunk.address().await;
        match recovered_chunk_address == address {
            true => Ok(chunk),
            false => Err(SingleOwnerChunkError::ChunkMismatch {
                address,
                recovered_chunk_address,
                recovered_owner,
            }),
        }
    }

    async fn to_sign(id: B256, body: &impl ChunkBody) -> B256 {
        let mut hasher = Keccak256::new();
        hasher.update(id);
        hasher.update(body.hash().await);

        hasher.finalize()
    }
}

impl swarm_primitives_traits::Chunk for SingleOwnerChunk {
    async fn address(&self) -> ChunkAddress {
        let mut hasher = Keccak256::new();
        hasher.update(self.id);
        hasher.update(self.owner);

        hasher.finalize()
    }

    async fn verify(&self, address: ChunkAddress) -> bool {
        let hash = Self::to_sign(self.id, &self.body).await;
        let recovered = self.signature.recover_address_from_msg(hash);

        if let Ok(recovered_owner) = recovered {
            return recovered_owner == self.owner && address == self.address().await;
        }

        false
    }
}

impl ChunkEncoding for SingleOwnerChunk {
    fn size(&self) -> usize {
        MIN_SOC_FIELDS_SIZE + self.body.size()
    }

    fn to_boxed_slice(&self) -> Box<[u8]> {
        let mut result = Vec::with_capacity(self.size());
        result.extend_from_slice(&self.id.as_ref());
        result.extend_from_slice(&self.signature.as_bytes());
        result.extend_from_slice(&self.body.to_boxed_slice());

        result.into_boxed_slice()
    }
}

impl ChunkDecoding for SingleOwnerChunk {
    async fn from_slice(value: &[u8]) -> Result<Self, impl std::error::Error> {
        if value.len() < MIN_SOC_FIELDS_SIZE {
            return Err(SingleOwnerChunkError::InsufficientData {
                min_size: MIN_SOC_FIELDS_SIZE,
                actual_size: value.len(),
            });
        }

        // SAFETY: Unwrap is safe as indexing of the slice is guarded by the above conditional.
        let id = B256::from_slice(&value[0..ID_SIZE]);
        let signature = PrimitiveSignature::try_from(&value[ID_SIZE..MIN_SOC_FIELDS_SIZE])?;
        let body = BMTBody::from_slice(&value[MIN_SOC_FIELDS_SIZE..]).await?;
        let hash = Self::to_sign(id, &body).await;
        let recovered_owner = signature.recover_address_from_msg(&hash).unwrap();

        Ok(Self {
            id,
            owner: recovered_owner,
            signature,
            body,
        })
    }
}
