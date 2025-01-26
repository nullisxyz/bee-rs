mod bmt_body;
mod content;
mod single_owner;

pub use content::{ContentChunk, ContentChunkError};
pub use single_owner::{SingleOwnerChunk, SingleOwnerChunkError};
use swarm_primitives_traits::{Chunk as ChunkTrait, ChunkAddress};

#[derive(Debug, Eq, PartialEq)]
pub enum Chunk {
    Content(ContentChunk),
    SingleOwner(SingleOwnerChunk),
}

impl ChunkTrait for Chunk {
    fn address(&self) -> ChunkAddress {
        match self {
            Chunk::Content(c) => c.address(),
            Chunk::SingleOwner(c) => c.address(),
        }
    }
}
