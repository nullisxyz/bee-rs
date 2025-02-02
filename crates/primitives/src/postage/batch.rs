use alloy::primitives::{Address, BlockNumber, BlockTimestamp, FixedBytes, U256};
use nectar_primitives_traits::CHUNK_SIZE;

pub type BatchId = FixedBytes<32>;

/// Core batch data structure representing paid-for storage capacity
// TODO: Implement serialisation / deserialisation for saving this to storage
pub struct Batch {
    /// The batch ID
    id: BatchId,
    /// Normalised balance of the batch
    value: U256,
    /// The owner of the batch
    owner: Address,
    /// The size of the batch in chunks (2^depth)
    depth: u8,
    /// The depth of each collission bucket in the batch (referred to as the uniformity within the batch)
    bucket_depth: u8,
    /// The block number in which the batch was last updated
    last_updated: BlockNumber,
    /// Whether the batch is immutable or not
    immutable: bool,
}

impl Batch {
    /// Create a new batch
    pub fn new(
        id: BatchId,
        value: U256,
        owner: Address,
        depth: u8,
        bucket_depth: u8,
        last_updated: BlockNumber,
        immutable: bool,
    ) -> Self {
        Batch {
            id,
            value,
            owner,
            depth,
            bucket_depth,
            last_updated,
            immutable,
        }
    }

    /// Get the batch ID
    pub fn id(&self) -> &BatchId {
        &self.id
    }

    /// Get the value of the batch
    pub fn value(&self) -> U256 {
        self.value
    }

    /// Get the owner of the batch
    pub fn owner(&self) -> &Address {
        &self.owner
    }

    /// Get the depth of the batch
    pub fn depth(&self) -> u8 {
        self.depth
    }

    /// Get the bucket depth of the batch
    pub fn bucket_depth(&self) -> u8 {
        self.bucket_depth
    }

    /// Get the last updated block of the batch
    pub fn last_updated(&self) -> BlockNumber {
        self.last_updated
    }

    /// Get whether the batch is immutable or not
    pub fn immutable(&self) -> bool {
        self.immutable
    }

    /// Get the TTL remaining for the batch (in blocks) given the current out payment and price
    pub fn ttl_blocks(&self, current_out_payment: U256, current_price: U256) -> u64 {
        if self.value() <= current_out_payment {
            return 0;
        } else {
            let per_block = current_price * U256::from(Self::chunks(self.depth));

            ((self.value() - current_out_payment) / per_block).to()
        }
    }

    /// Given the TTL remaining for the batch (in blocks) and the block time, calculate the TTL remaining in seconds.
    pub fn ttl_seconds(&self, blocks_remaining: u64, block_time: u64) -> u64 {
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

    /// Given the current price of storage, calculate the cost of a batch
    pub fn cost(depth: u8, price: U256) -> U256 {
        price * U256::from(Self::chunks(depth))
    }

    /// Given a batch depth, calculate the number of chunks in the batch
    /// This is equivalent to 2^depth
    pub fn chunks(depth: u8) -> u64 {
        2_u64.pow(depth as u32)
    }

    /// Given a batch depth, calculate the size of the batch in bytes
    pub fn size(depth: u8) -> u64 {
        Self::chunks(depth) * CHUNK_SIZE as u64
    }

    /// Given a size in bytes, calculate the depth of a batch required to store that size
    /// Note: This does not take into account any additional overheads such as BMT file
    /// encoding.
    pub fn depth_for_size(size: u64) -> u8 {
        let chunks = size / CHUNK_SIZE as u64;

        (chunks as f64).log2().ceil() as u8
    }
}
