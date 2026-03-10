//! # Pallet Inscription
//!
//! Genesis inscription permanently engraved in Block 0.
//! Read-only after genesis — no extrinsics, no mutations.

#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;
use alloc::vec::Vec;

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_support::pallet_prelude::*;
	use sp_core::H256;

	#[pallet::pallet]
	#[pallet::without_storage_info]
	pub struct Pallet<T>(_);

	#[pallet::config]
	pub trait Config: frame_system::Config {}

	/// The raw inscription text, set once at genesis.
	#[pallet::storage]
	pub type Inscription<T: Config> = StorageValue<_, Vec<u8>, ValueQuery>;

	/// SHA-256 hash of the inscription, set once at genesis.
	#[pallet::storage]
	pub type InscriptionHash<T: Config> = StorageValue<_, H256, ValueQuery>;

	#[pallet::genesis_config]
	pub struct GenesisConfig<T: Config> {
		/// Raw inscription bytes (UTF-8 text).
		pub inscription: Vec<u8>,
		#[serde(skip)]
		pub _phantom: core::marker::PhantomData<T>,
	}

	impl<T: Config> Default for GenesisConfig<T> {
		fn default() -> Self {
			Self {
				inscription: b"NEXUS".to_vec(),
				_phantom: Default::default(),
			}
		}
	}

	#[pallet::genesis_build]
	impl<T: Config> BuildGenesisConfig for GenesisConfig<T> {
		fn build(&self) {
			assert!(!self.inscription.is_empty(), "Genesis inscription must not be empty");

			// Compute SHA-256 hash
			let hash = sp_core::hashing::sha2_256(&self.inscription);
			let h256 = H256::from_slice(&hash);

			Inscription::<T>::put(&self.inscription);
			InscriptionHash::<T>::put(h256);
		}
	}
}
