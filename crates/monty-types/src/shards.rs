//! Shards selected by the host for a single interpreter session.

/// A group of operations the host can enable together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(u32)]
pub enum Shard {
    /// Evaluate Python source supplied by sandbox code, including string annotations.
    DynamicEvaluation = 1,
}

/// The enabled shards. An explicit selection keeps the same members when new shards are added.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Shards(u32);

impl Shards {
    /// Start with only core Python operations.
    #[must_use]
    pub const fn core() -> Self {
        Self(0)
    }

    /// Include the operations provided by `shard`.
    #[must_use]
    pub const fn with(self, shard: Shard) -> Self {
        Self(self.0 | shard as u32)
    }

    /// Omit the operations provided by `shard`.
    #[must_use]
    pub const fn without(self, shard: Shard) -> Self {
        Self(self.0 & !(shard as u32))
    }

    /// Whether the session provides this shard.
    #[must_use]
    pub const fn contains(self, shard: Shard) -> bool {
        self.0 & shard as u32 != 0
    }

    /// Enabled shards in the worker protocol.
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Decode a worker request, rejecting unknown shard bits.
    #[must_use]
    pub const fn from_bits(bits: u32) -> Option<Self> {
        if bits & !(Shard::DynamicEvaluation as u32) == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }
}

impl Default for Shards {
    fn default() -> Self {
        Self::core().with(Shard::DynamicEvaluation)
    }
}
