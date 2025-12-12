use crate::{ChainSpec, DepositContract};
use alloc::{boxed::Box, vec::Vec};
use alloy_chains::Chain;
use alloy_eips::{calc_next_block_base_fee, eip1559::BaseFeeParams, eip7840::BlobParams};
use alloy_genesis::Genesis;
use alloy_primitives::{B256, U256};
use core::fmt::{Debug, Display};
use reth_ethereum_forks::EthereumHardforks;
use reth_network_peers::NodeRecord;
use reth_primitives_traits::{AlloyBlockHeader, BlockHeader};

/// Trait representing type configuring a chain spec.
#[auto_impl::auto_impl(&, Arc)]
pub trait EthChainSpec: Send + Sync + Unpin + Debug {
    /// The header type of the network.
    type Header: BlockHeader;

    /// Returns the [`Chain`] object this spec targets.
    fn chain(&self) -> Chain;

    /// Returns the chain id number
    fn chain_id(&self) -> u64 {
        self.chain().id()
    }

    /// Get the [`BaseFeeParams`] for the chain at the given timestamp.
    fn base_fee_params_at_timestamp(&self, timestamp: u64) -> BaseFeeParams;

    /// Get the [`BlobParams`] for the given timestamp
    fn blob_params_at_timestamp(&self, timestamp: u64) -> Option<BlobParams>;

    /// Returns the deposit contract data for the chain, if it's present
    fn deposit_contract(&self) -> Option<&DepositContract>;

    /// The genesis hash.
    fn genesis_hash(&self) -> B256;

    /// The delete limit for pruner, per run.
    fn prune_delete_limit(&self) -> usize;

    /// Returns a string representation of the hardforks.
    fn display_hardforks(&self) -> Box<dyn Display>;

    /// The genesis header.
    fn genesis_header(&self) -> &Self::Header;

    /// The genesis block specification.
    fn genesis(&self) -> &Genesis;

    /// The bootnodes for the chain, if any.
    fn bootnodes(&self) -> Option<Vec<NodeRecord>>;

    /// Returns `true` if this chain contains Optimism configuration.
    fn is_optimism(&self) -> bool {
        self.chain().is_optimism()
    }

    /// Returns `true` if this chain contains Ethereum configuration.
    fn is_ethereum(&self) -> bool {
        self.chain().is_ethereum()
    }

    /// Returns the final total difficulty if the Paris hardfork is known.
    fn final_paris_total_difficulty(&self) -> Option<U256>;

    /// Returns a simple string representation of base fee change multipliers for logging.
    /// Returns None if no multipliers are configured.
    fn base_fee_multipliers_info(&self) -> Option<String> {
        None
    }

    /// Get the base fee change multiplier for the given block number.
    /// Returns 1.0 if no multiplier is configured for that block.
    fn base_fee_multiplier_at_block(&self, _block_number: u64) -> f64 {
        1.0
    }

    /// See [`calc_next_block_base_fee`].
    fn next_block_base_fee(&self, parent: &Self::Header, target_timestamp: u64) -> Option<u64> {
        Some(calc_next_block_base_fee(
            parent.gas_used(),
            parent.gas_limit(),
            parent.base_fee_per_gas()?,
            self.base_fee_params_at_timestamp(target_timestamp),
        ))
    }
}

impl<H: BlockHeader> EthChainSpec for ChainSpec<H> {
    type Header = H;

    fn chain(&self) -> Chain {
        self.chain
    }

    fn base_fee_params_at_timestamp(&self, timestamp: u64) -> BaseFeeParams {
        self.base_fee_params_at_timestamp(timestamp)
    }

    fn blob_params_at_timestamp(&self, timestamp: u64) -> Option<BlobParams> {
        if let Some(blob_param) = self.blob_params.active_scheduled_params_at_timestamp(timestamp) {
            Some(*blob_param)
        } else if self.is_osaka_active_at_timestamp(timestamp) {
            Some(self.blob_params.osaka)
        } else if self.is_prague_active_at_timestamp(timestamp) {
            Some(self.blob_params.prague)
        } else if self.is_cancun_active_at_timestamp(timestamp) {
            Some(self.blob_params.cancun)
        } else {
            None
        }
    }

    fn deposit_contract(&self) -> Option<&DepositContract> {
        self.deposit_contract.as_ref()
    }

    fn genesis_hash(&self) -> B256 {
        self.genesis_hash()
    }

    fn prune_delete_limit(&self) -> usize {
        self.prune_delete_limit
    }

    fn display_hardforks(&self) -> Box<dyn Display> {
        Box::new(Self::display_hardforks(self))
    }

    fn genesis_header(&self) -> &Self::Header {
        self.genesis_header()
    }

    fn genesis(&self) -> &Genesis {
        self.genesis()
    }

    fn bootnodes(&self) -> Option<Vec<NodeRecord>> {
        self.bootnodes()
    }

    fn is_optimism(&self) -> bool {
        false
    }

    fn final_paris_total_difficulty(&self) -> Option<U256> {
        self.paris_block_and_final_difficulty.map(|(_, final_difficulty)| final_difficulty)
    }

    fn base_fee_multipliers_info(&self) -> Option<String> {
        if self.base_fee_change_multipliers.is_empty() {
            None
        } else {
            Some(self.display_base_fee_multipliers())
        }
    }

    fn base_fee_multiplier_at_block(&self, block_number: u64) -> f64 {
        self.base_fee_change_multiplier_at_block(block_number)
    }

    fn next_block_base_fee(&self, parent: &Self::Header, target_timestamp: u64) -> Option<u64> {
        let parent_base_fee = parent.base_fee_per_gas()?;
        let parent_gas_used = parent.gas_used();
        let parent_gas_limit = parent.gas_limit();
        let base_fee_params = self.base_fee_params_at_timestamp(target_timestamp);

        // Get multiplier for the next block
        let next_block_number = parent.number() + 1;
        let multiplier = self.base_fee_change_multiplier_at_block(next_block_number);

        // If multiplier is 1.0, use standard calculation
        if multiplier == 1.0 {
            return Some(calc_next_block_base_fee(
                parent_gas_used,
                parent_gas_limit,
                parent_base_fee,
                base_fee_params,
            ));
        }

        // Calculate base fee with multiplier applied to delta (as per Go reference)
        // This matches the logic from CalcBaseFee in the Go reference:
        // 1. Calculate standard base fee using EIP-1559 formula
        // 2. Calculate delta = |standard_base_fee - parent_base_fee|
        // 3. Apply multiplier to delta
        // 4. Apply adjusted delta to parent_base_fee (add if increased, subtract if decreased)

        // Calculate standard base fee first
        let standard_base_fee = calc_next_block_base_fee(
            parent_gas_used,
            parent_gas_limit,
            parent_base_fee,
            base_fee_params,
        );

        // Calculate delta (change in base fee)
        let base_fee_delta = if standard_base_fee > parent_base_fee {
            standard_base_fee - parent_base_fee
        } else if parent_base_fee > standard_base_fee {
            parent_base_fee - standard_base_fee
        } else {
            // No change, return parent base fee
            return Some(parent_base_fee);
        };

        // Apply multiplier to delta (as per Go reference: applyBaseFeeMultiplier)
        // Use pure integer arithmetic like Go's big.Int - no floating point operations
        // Scale multiplier to 10^18 (like Solidity wei) for maximum precision
        // This matches Go's big.Int precision exactly
        const PRECISION: u128 = 1_000_000_000_000_000_000; // 10^18

        // Convert f64 multiplier to fixed-point integer representation
        // Use truncation (not rounding) to match Go's big.Int behavior exactly
        // Go's big.Int uses integer division which truncates, not rounds
        let multiplier_scaled = (multiplier * PRECISION as f64) as u128;

        // Perform multiplication in u128 (like big.Int.Mul)
        let product = base_fee_delta as u128 * multiplier_scaled;

        // Use simple integer division (truncation) like Go's big.Int.Div
        // Go's big.Int.Div truncates towards zero, which is what integer division does
        let adjusted_delta = (product / PRECISION) as u64;

        // Apply adjusted delta to parent base fee
        let adjusted_base_fee = if standard_base_fee > parent_base_fee {
            // Base fee increased: enforce minimum delta of 1 when increasing (as per Go: enforceMinOne=true)
            parent_base_fee.saturating_add(adjusted_delta.max(1))
        } else {
            // Base fee decreased: no minimum delta enforcement (as per Go: enforceMinOne=false)
            // Ensure we don't go below 0
            parent_base_fee.saturating_sub(adjusted_delta)
        };

        Some(adjusted_base_fee)
    }
}
