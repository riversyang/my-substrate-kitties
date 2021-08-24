#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use codec::{Decode, Encode, HasCompact};
	use frame_support::{dispatch::DispatchResult, pallet_prelude::*, traits::Randomness};
	use frame_system::pallet_prelude::*;
	use sp_io::hashing::blake2_128;

	#[derive(Encode, Decode)]
	pub struct Kitty(pub [u8; 16]);

	#[derive(Encode, Decode, Debug, Clone, PartialEq)]
	pub enum Gender {
		Male,
		Female,
	}

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
		/// Identifier for the kitty.
		type KittyId: From<u32> + Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	pub struct Pallet<T>(_);

	#[pallet::storage]
	#[pallet::getter(fn kitties_count)]
	pub type KittiesCount<T> = StorageValue<_, u32>;

	#[pallet::storage]
	#[pallet::getter(fn kitties)]
	pub type Kitties<T: Config> =
		StorageMap<_, Blake2_128Concat, T::KittyId, Option<Kitty>, ValueQuery>;

	#[pallet::storage]
	#[pallet::getter(fn kitties_owner)]
	pub type KittiesOwner<T: Config> =
		StorageMap<_, Blake2_128Concat, T::KittyId, Option<T::AccountId>, ValueQuery>;

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		KittyCreated(T::KittyId, T::AccountId),
		KittyTransfered(T::KittyId, T::AccountId, T::AccountId),
	}

	#[pallet::error]
	pub enum Error<T> {
		KittiesCountOverflow,
		KittyNotExists,
		NotOwnerOfKitty,
		CanNotBreedWithSameGender,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn create(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let dna = Self::get_random_value(&who);
			Self::create_kitty(dna, &who)
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn transfer(
			origin: OriginFor<T>,
			id: T::KittyId,
			new_owner: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::transfer_kitty(&id, &who, &new_owner)
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn breed(origin: OriginFor<T>, id1: T::KittyId, id2: T::KittyId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Kitties::<T>::get(id1).is_some(), Error::<T>::KittyNotExists);
			ensure!(Kitties::<T>::get(id2).is_some(), Error::<T>::KittyNotExists);
			let kitty1 = Kitties::<T>::get(id1).unwrap();
			let kitty2 = Kitties::<T>::get(id1).unwrap();
			ensure!(kitty1.gender() == kitty2.gender(), Error::<T>::CanNotBreedWithSameGender);
			let selector = Self::get_random_value(&who);
			let mut dna = [0u8; 16];
			for i in 0..dna.len() {
				dna[i] = (selector[i] & kitty1.0[i]) | (selector[i] & kitty2.0[i]);
			}
			Self::create_kitty(dna, &who)
		}
	}

	impl<T: Config> Pallet<T> {
		fn get_random_value(sender: &T::AccountId) -> [u8; 16] {
			let payload = (
				T::Randomness::random_seed(),
				&sender,
				<frame_system::Pallet<T>>::extrinsic_index(),
			);
			payload.using_encoded(blake2_128)
		}

		fn create_kitty(dna: [u8; 16], owner: &T::AccountId) -> DispatchResult {
			let count = match Self::kitties_count() {
				Some(count) => {
					ensure!(count != u32::MAX, Error::<T>::KittiesCountOverflow);
					count + 1
				}
				None => 1,
			};
			let id = T::KittyId::from(count);
			Kitties::<T>::insert(id, Some(Kitty(dna)));
			KittiesOwner::<T>::insert(id, Some(owner.clone()));
			KittiesCount::<T>::put(count);

			Self::deposit_event(Event::KittyCreated(id.clone(), owner.clone()));

			Ok(())
		}

		fn transfer_kitty(
			id: &T::KittyId,
			owner: &T::AccountId,
			new_owner: &T::AccountId,
		) -> DispatchResult {
			ensure!(Kitties::<T>::get(id).is_some(), Error::<T>::KittyNotExists);
			ensure!(Some(owner.clone()) == KittiesOwner::<T>::get(id), Error::<T>::NotOwnerOfKitty);
			KittiesOwner::<T>::insert(id, Some(new_owner));
			Self::deposit_event(Event::KittyTransfered(
				id.clone(),
				owner.clone(),
				new_owner.clone(),
			));
			Ok(())
		}
	}

	impl Kitty {
		pub fn gender(&self) -> Gender {
			if self.0[0] % 2 == 0 {
				Gender::Male
			} else {
				Gender::Female
			}
		}
	}
}
