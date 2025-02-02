use alloy::primitives::PrimitiveSignature;
use nectar_primitives_traits::AuthProof;

use super::Batch;

pub struct PostageStamp {
    batch: Batch,
    index: u64,
    timestamp: u64,
    signature: PrimitiveSignature,
}

impl AuthProof for PostageStamp {
    fn proof_data(&self) -> &bytes::Bytes {
        // Combines `batch_id`, `index`, `timestamp` and `signature` into a single byte array
        todo!()
    }
}
