//! # Non Fungible Token
//! The module provides implementations for non-fungible-token.
//!
//! - [`Config`](./trait.Config.html)
//! - [`Call`](./enum.Call.html)
//! - [`Module`](./struct.Module.html)
//!
//! ## Overview
//!
//! This module provides basic functions to create and manager
//! NFT(non fungible token) such as `create_class`, `transfer`, `mint`, `burn`.

//! ### Module Functions
//!
//! - `create_class` - Create NFT(non fungible token) class
//! - `transfer` - Transfer NFT(non fungible token) to another account.
//! - `mint` - Mint NFT(non fungible token)
//! - `burn` - Burn NFT(non fungible token)
//! - `destroy_class` - Destroy NFT(non fungible token) class

#![cfg_attr(not(feature = "std"), no_std)]

use codec::{Decode, Encode};
use frame_support::{decl_error, decl_module, decl_storage, ensure, Parameter};
use sp_runtime::{
	traits::{AtLeast32BitUnsigned, CheckedAdd, CheckedSub, Member, One, Zero},
	DispatchError, DispatchResult, RuntimeDebug,
};
use sp_std::vec::Vec;

mod mock;
mod tests;

/// Class info
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct ClassInfo<TokenId, AccountId, Data> {
	/// Class metadata
	pub metadata: Vec<u8>,
	/// Total issuance for the class
	pub total_issuance: TokenId,
	/// Class owner
	pub owner: AccountId,
	/// Class Properties
	pub data: Data,
}

/// Token info
#[derive(Encode, Decode, Clone, Eq, PartialEq, RuntimeDebug)]
pub struct TokenInfo<AccountId, Data> {
	/// Token metadata
	pub metadata: Vec<u8>,
	/// Token owner
	pub owner: AccountId,
	/// Token Properties
	pub data: Data,
}

pub trait Config: frame_system::Config {
	/// The class ID type
	type ClassId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy;
	/// The token ID type
	type TokenId: Parameter + Member + AtLeast32BitUnsigned + Default + Copy;
	/// The class properties type
	type ClassData: Parameter + Member;
	/// The token properties type
	type TokenData: Parameter + Member;
}

decl_error! {
	/// Error for non-fungible-token module.
	pub enum Error for Module<T: Config> {
		/// No available class ID
		NoAvailableClassId,
		/// No available token ID
		NoAvailableTokenId,
		/// Token(ClassId, TokenId) not found
		TokenNotFound,
		/// Class not found
		ClassNotFound,
		/// The operator is not the owner of the token and has no permission
		NoPermission,
		/// Arithmetic calculation overflow
		NumOverflow,
		/// Can not destroy class
		/// Total issuance is not 0
		CannotDestroyClass,
	}
}

pub type ClassInfoOf<T> =
	ClassInfo<<T as Config>::TokenId, <T as frame_system::Config>::AccountId, <T as Config>::ClassData>;
pub type TokenInfoOf<T> = TokenInfo<<T as frame_system::Config>::AccountId, <T as Config>::TokenData>;

decl_storage! {
	trait Store for Module<T: Config> as NonFungibleToken {
		/// Next available class ID.
		pub NextClassId get(fn next_class_id): T::ClassId;
		/// Next available token ID.
		pub NextTokenId get(fn next_token_id): map hasher(twox_64_concat) T::ClassId => T::TokenId;
		/// Store class info.
		///
		/// Returns `None` if class info not set or removed.
		pub Classes get(fn classes): map hasher(twox_64_concat) T::ClassId => Option<ClassInfoOf<T>>;
		/// Store token info.
		///
		/// Returns `None` if token info not set or removed.
		pub Tokens get(fn tokens): double_map hasher(twox_64_concat) T::ClassId, hasher(twox_64_concat) T::TokenId => Option<TokenInfoOf<T>>;
		/// Token existence check by owner and class ID.
		#[cfg(not(feature = "disable-tokens-by-owner"))]
		pub TokensByOwner get(fn tokens_by_owner): double_map hasher(twox_64_concat) T::AccountId, hasher(twox_64_concat) (T::ClassId, T::TokenId) => Option<()>;
	}
}

decl_module! {
	pub struct Module<T: Config> for enum Call where origin: T::Origin {
	}
}

impl<T: Config> Module<T> {
	/// Create NFT(non fungible token) class
	pub fn create_class(
		owner: &T::AccountId,
		metadata: Vec<u8>,
		data: T::ClassData,
	) -> Result<T::ClassId, DispatchError> {
		let class_id = NextClassId::<T>::try_mutate(|id| -> Result<T::ClassId, DispatchError> {
			let current_id = *id;
			*id = id.checked_add(&One::one()).ok_or(Error::<T>::NoAvailableClassId)?;
			Ok(current_id)
		})?;

		let info = ClassInfo {
			metadata,
			total_issuance: Default::default(),
			owner: owner.clone(),
			data,
		};
		Classes::<T>::insert(class_id, info);

		Ok(class_id)
	}

	/// Transfer NFT(non fungible token) from `from` account to `to` account
	pub fn transfer(from: &T::AccountId, to: &T::AccountId, token: (T::ClassId, T::TokenId)) -> DispatchResult {
		Tokens::<T>::try_mutate(token.0, token.1, |token_info| -> DispatchResult {
			let mut info = token_info.as_mut().ok_or(Error::<T>::TokenNotFound)?;
			ensure!(info.owner == *from, Error::<T>::NoPermission);
			if from == to {
				// no change needed
				return Ok(());
			}

			info.owner = to.clone();

			#[cfg(not(feature = "disable-tokens-by-owner"))]
			{
				TokensByOwner::<T>::remove(from, token);
				TokensByOwner::<T>::insert(to, token, ());
			}

			Ok(())
		})
	}

	/// Mint NFT(non fungible token) to `owner`
	pub fn mint(
		owner: &T::AccountId,
		class_id: T::ClassId,
		metadata: Vec<u8>,
		data: T::TokenData,
	) -> Result<T::TokenId, DispatchError> {
		NextTokenId::<T>::try_mutate(class_id, |id| -> Result<T::TokenId, DispatchError> {
			let token_id = *id;
			*id = id.checked_add(&One::one()).ok_or(Error::<T>::NoAvailableTokenId)?;

			Classes::<T>::try_mutate(class_id, |class_info| -> DispatchResult {
				let info = class_info.as_mut().ok_or(Error::<T>::ClassNotFound)?;
				info.total_issuance = info
					.total_issuance
					.checked_add(&One::one())
					.ok_or(Error::<T>::NumOverflow)?;
				Ok(())
			})?;

			let token_info = TokenInfo {
				metadata,
				owner: owner.clone(),
				data,
			};
			Tokens::<T>::insert(class_id, token_id, token_info);
			#[cfg(not(feature = "disable-tokens-by-owner"))]
			TokensByOwner::<T>::insert(owner, (class_id, token_id), ());

			Ok(token_id)
		})
	}

	/// Burn NFT(non fungible token) from `owner`
	pub fn burn(owner: &T::AccountId, token: (T::ClassId, T::TokenId)) -> DispatchResult {
		Tokens::<T>::try_mutate_exists(token.0, token.1, |token_info| -> DispatchResult {
			let t = token_info.take().ok_or(Error::<T>::TokenNotFound)?;
			ensure!(t.owner == *owner, Error::<T>::NoPermission);

			Classes::<T>::try_mutate(token.0, |class_info| -> DispatchResult {
				let info = class_info.as_mut().ok_or(Error::<T>::ClassNotFound)?;
				info.total_issuance = info
					.total_issuance
					.checked_sub(&One::one())
					.ok_or(Error::<T>::NumOverflow)?;
				Ok(())
			})?;

			#[cfg(not(feature = "disable-tokens-by-owner"))]
			TokensByOwner::<T>::remove(owner, token);

			Ok(())
		})
	}

	/// Destroy NFT(non fungible token) class
	pub fn destroy_class(owner: &T::AccountId, class_id: T::ClassId) -> DispatchResult {
		Classes::<T>::try_mutate_exists(class_id, |class_info| -> DispatchResult {
			let info = class_info.take().ok_or(Error::<T>::ClassNotFound)?;
			ensure!(info.owner == *owner, Error::<T>::NoPermission);
			ensure!(info.total_issuance == Zero::zero(), Error::<T>::CannotDestroyClass);

			NextTokenId::<T>::remove(class_id);

			Ok(())
		})
	}

	pub fn is_owner(account: &T::AccountId, token: (T::ClassId, T::TokenId)) -> bool {
		#[cfg(feature = "disable-tokens-by-owner")]
		return Tokens::<T>::get(token.0, token.1).map_or(false, |token| token.owner == *account);

		#[cfg(not(feature = "disable-tokens-by-owner"))]
		TokensByOwner::<T>::contains_key(account, token)
	}
}
