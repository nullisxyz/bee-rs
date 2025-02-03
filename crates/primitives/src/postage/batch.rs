use alloy::{
    primitives::{Address, BlockNumber, BlockTimestamp, FixedBytes, U256},
    signers::Signer,
};
use nectar_primitives_traits::CHUNK_SIZE;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use thiserror::Error;

pub type BatchId = FixedBytes<32>;

/// Core batch data structure representing paid-for storage capacity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Batch {
    /// The batch ID
    id: BatchId,
    /// Normalised balance of the batch
    value: U256,
    /// The owner of the batch
    owner: Address,
    /// Depth directly corresponds to the number of chunks permitted to be signed by the batch (2^depth)
    depth: u8,
    /// The depth of each collision bucket in the batch (referred to as the uniformity within the batch)
    /// This is required to be greater than or equal to the **storage depth**, otherwise the batch is
    /// invalid.
    bucket_depth: u8,
    /// Whether the batch is immutable or not
    immutable: bool,
}

impl Batch {
    /// Get whether the batch is immutable or not
    pub fn immutable(&self) -> bool {
        self.immutable
    }

    /// Get the TTL remaining for the batch (in blocks) given the current out payment and price
    pub fn ttl_blocks(&self, current_out_payment: U256, current_price: U256) -> u64 {
        match self.value <= current_out_payment {
            true => 0,
            false => {
                let per_block = current_price * U256::from(Self::chunks(self.depth));
                ((self.value - current_out_payment) / per_block).to()
            }
        }
    }

    /// Given the TTL remaining for the batch (in blocks) and the block time, calculate the TTL remaining in seconds.
    pub const fn ttl_seconds(&self, blocks_remaining: u64, block_time: u64) -> u64 {
        blocks_remaining * block_time
    }

    /// Given the current out payment, current price and block time, calculate the TTL remaining in seconds.
    pub fn ttl(&self, current_out_payment: U256, current_price: U256, block_time: u64) -> u64 {
        let blocks_remaining = self.ttl_blocks(current_out_payment, current_price);
        self.ttl_seconds(blocks_remaining, block_time)
    }

    /// Get the expiry block of the batch
    pub fn expiry_block_number(
        &self,
        current_out_payment: U256,
        current_price: U256,
        current_block_number: BlockNumber,
    ) -> BlockNumber {
        current_block_number + self.ttl_blocks(current_out_payment, current_price)
    }

    /// Get the expiry time of the batch (in unix time)
    pub fn expiry(
        &self,
        current_out_payment: U256,
        current_price: U256,
        current_timestamp: BlockTimestamp,
        block_time: u64,
    ) -> u64 {
        let blocks_remaining = self.ttl_blocks(current_out_payment, current_price);
        let ttl_seconds = self.ttl_seconds(blocks_remaining, block_time);
        current_timestamp + ttl_seconds
    }

    /// Determine if the batch is expired
    pub fn expired(
        &self,
        current_out_payment: U256,
        current_price: U256,
        current_block_number: BlockNumber,
    ) -> bool {
        self.expiry_block_number(current_out_payment, current_price, current_block_number)
            <= current_block_number
    }

    /// Determine the maximum number of collisions possible in a bucket given the batch's
    /// depth and bucket depth.
    pub const fn max_collisions(&self) -> u64 {
        2_u64.pow((self.depth - self.bucket_depth) as u32)
    }

    /// Given the current price of storage, calculate the cost of a batch
    pub fn cost(depth: u8, price: U256) -> U256 {
        price * U256::from(Self::chunks(depth))
    }

    /// Given a batch depth, calculate the number of chunks in the batch
    /// This is equivalent to 2^depth.
    ///
    /// # Panics
    /// If the depth is greater than 63, this function will panic.
    #[track_caller]
    pub const fn chunks(depth: u8) -> u64 {
        2_u64.pow(depth as u32)
    }

    /// Given a batch depth, calculate the size of the batch in bytes
    pub const fn size(depth: u8) -> u64 {
        Self::chunks(depth) * CHUNK_SIZE as u64
    }

    /// Given a size in bytes, calculate the depth of a batch required to store that size.
    /// Note that uploading a data of 0 bytes length is allowed, but essentially represents
    /// a chunk of 0 bytes that is padded to the chunk size.
    pub fn depth_for_size(size: u64) -> u8 {
        let chunks = (size / CHUNK_SIZE as u64).min(1);
        (chunks as f64).log2().ceil() as u8
    }
}

#[derive(Debug, Error)]
pub enum BatchError {
    #[error("Invalid depth: {0}")]
    InvalidDepth(u8),
    #[error("Invalid bucket depth: {0}")]
    InvalidBucketDepth(u8),
    #[error("Size is too small for batch")]
    SizeTooSmall,
    #[error("Duration in blocks must be greater than 0")]
    DurationInBlocksZero,
    #[error("Block time must be greater than 0")]
    BlockTimeZero,
    #[error("Missing required field: {0}")]
    MissingField(&'static str),
}

// Helper methods for error creation
impl BatchError {
    pub fn missing_field(field: &'static str) -> Self {
        BatchError::MissingField(field)
    }
}

// Marker traits for builder states
pub trait BuilderState {}
pub struct Initial;
pub struct WithSigner;
pub struct WithSize;
pub struct WithValue;
impl BuilderState for Initial {}
impl BuilderState for WithSigner {}
impl BuilderState for WithSize {}
impl BuilderState for WithValue {}

const MIN_BUCKET_DEPTH: u8 = 16;
const IMMUTABLE_DEFAULT: bool = false;

#[derive(Default)]
struct BatchConfig {
    id: Option<BatchId>,
    value: Option<U256>,
    owner: Option<Address>,
    depth: Option<u8>,
    bucket_depth: Option<u8>,
    immutable: Option<bool>,
}

impl BatchConfig {
    fn build(self) -> Result<Batch, BatchError> {
        let id = self.id.ok_or(BatchError::missing_field("id"))?;
        let value = self.value.ok_or(BatchError::missing_field("value"))?;
        let owner = self.owner.ok_or(BatchError::missing_field("owner"))?;
        let depth = self.depth.ok_or(BatchError::missing_field("depth"))?;
        let bucket_depth = self.bucket_depth.unwrap_or(depth.max(MIN_BUCKET_DEPTH));
        let immutable = self.immutable.unwrap_or(IMMUTABLE_DEFAULT);

        if bucket_depth > depth {
            return Err(BatchError::InvalidBucketDepth(bucket_depth));
        }

        Ok(Batch {
            id,
            value,
            owner,
            depth,
            bucket_depth,
            immutable,
        })
    }
}

pub struct BatchBuilder<S: BuilderState> {
    config: BatchConfig,
    _state: PhantomData<S>,
}

impl<S: BuilderState> BatchBuilder<S> {
    pub fn with_id(mut self, id: BatchId) -> Self {
        self.config.id = Some(id);
        self
    }

    pub fn with_immutable(mut self, immutable: bool) -> Self {
        self.config.immutable = Some(immutable);
        self
    }

    pub fn with_bucket_depth(mut self, bucket_depth: u8) -> Result<Self, BatchError> {
        if bucket_depth < MIN_BUCKET_DEPTH {
            return Err(BatchError::InvalidBucketDepth(bucket_depth));
        }
        self.config.bucket_depth = Some(bucket_depth);
        Ok(self)
    }
}

impl BatchBuilder<Initial> {
    pub fn new() -> Self {
        Self {
            config: BatchConfig::default(),
            _state: PhantomData,
        }
    }

    pub fn with_signer(mut self, signer: impl Signer) -> BatchBuilder<WithSigner> {
        self.config.owner = Some(signer.address());
        BatchBuilder {
            config: self.config,
            _state: PhantomData,
        }
    }
}

impl BatchBuilder<WithSigner> {
    pub fn auto_size(mut self, size_bytes: u64) -> Result<BatchBuilder<WithSize>, BatchError> {
        if size_bytes == 0 {
            return Err(BatchError::SizeTooSmall);
        }

        let depth = Batch::depth_for_size(size_bytes);
        self.config.depth = Some(depth);
        self.config.bucket_depth = Some(depth.max(MIN_BUCKET_DEPTH));

        Ok(BatchBuilder {
            config: self.config,
            _state: PhantomData,
        })
    }

    pub fn manual_value(mut self, value: U256) -> BatchBuilder<WithValue> {
        self.config.value = Some(value);
        BatchBuilder {
            config: self.config,
            _state: PhantomData,
        }
    }
}

impl BatchBuilder<WithSize> {
    pub fn auto_value(
        mut self,
        price: U256,
        duration_blocks: impl Into<Duration>,
    ) -> Result<BatchBuilder<WithValue>, BatchError> {
        let duration = duration_blocks.into();
        let blocks = duration.to_blocks()?;

        if blocks == 0 {
            return Err(BatchError::DurationInBlocksZero);
        }

        let depth = self.config.depth.ok_or(BatchError::InvalidDepth(0))?;
        let value = Batch::cost(depth, price) * U256::from(blocks);
        self.config.value = Some(value);

        Ok(BatchBuilder {
            config: self.config,
            _state: PhantomData,
        })
    }

    pub fn manual_value(mut self, value: U256) -> BatchBuilder<WithValue> {
        self.config.value = Some(value);
        BatchBuilder {
            config: self.config,
            _state: PhantomData,
        }
    }
}

impl BatchBuilder<WithValue> {
    pub fn build(self) -> Result<Batch, BatchError> {
        self.config.build()
    }
}

pub enum Duration {
    Blocks(u64),
    Time { secs: u64, block_time: u64 },
}

impl Duration {
    fn to_blocks(&self) -> Result<u64, BatchError> {
        match self {
            Duration::Blocks(blocks) => Ok(*blocks),
            Duration::Time { secs, block_time } => {
                if *block_time == 0 {
                    return Err(BatchError::BlockTimeZero);
                }
                Ok(*secs / *block_time)
            }
        }
    }
}

impl From<u64> for Duration {
    fn from(blocks: u64) -> Self {
        Duration::Blocks(blocks)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_builder() -> Result<(), BatchError> {
        // Manual configuration
        let batch1 = BatchBuilder::new()
            .with_signer(Signer::new(Address::zero()))
            .with_id(BatchId::zero())
            .manual_value(U256::from(100))
            .build()?;

        assert_eq!(batch1.value, U256::from(100));

        // Automatic size and value calculation
        let batch2 = BatchBuilder::new()
            .with_signer(Signer::new(Address::zero()))
            .with_id(BatchId::zero())
            .auto_size(1 << 20)? // 1 MiB
            .auto_value(
                U256::from(10),
                Duration::Time {
                    secs: 3600,
                    block_time: 15,
                },
            )?
            .with_immutable(true)
            .build()?;

        assert!(batch2.immutable());
        Ok(())
    }
}
