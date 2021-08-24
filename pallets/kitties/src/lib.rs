#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use codec::{Decode, Encode, HasCompact};
	use frame_support::{
		dispatch::DispatchResult,
		pallet_prelude::*,
		traits::{Currency, Randomness, ReservableCurrency},
	};
	use frame_system::pallet_prelude::*;
	use sp_io::hashing::blake2_128;

	#[derive(Clone, Encode, Decode)]
	pub struct Kitty<T: Config> {
		pub dna: [u8; 16],
		pub owner: T::AccountId,
		pub price: Option<BalanceOf<T>>,
	}

	#[derive(Encode, Decode, Debug, Clone, PartialEq)]
	pub enum Gender {
		Male,
		Female,
	}

	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;

	/// Configure the pallet by specifying the parameters and types on which it depends.
	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// Because this pallet emits events, it depends on the runtime's definition of an event.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		type Randomness: Randomness<Self::Hash, Self::BlockNumber>;
		/// Identifier for the kitty.
		type KittyId: From<u32> + Member + Parameter + Default + Copy + HasCompact + MaxEncodedLen;
		/// The currency trait.
		type Currency: ReservableCurrency<Self::AccountId>;
		/// The owner of kitty must reserve a certain amount of currency
		#[pallet::constant]
		type HoldingDepositForOneKitty: Get<BalanceOf<Self>>;
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
		StorageMap<_, Blake2_128Concat, T::KittyId, Option<Kitty<T>>, ValueQuery>;

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		KittyCreated(T::KittyId, T::AccountId),
		KittyTransfered(T::KittyId, T::AccountId, T::AccountId),
		KittyPriceSet(T::KittyId, BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		KittiesCountOverflow,
		KittyNotExists,
		NotOwnerOfKitty,
		CanNotBreedWithSameGender,
		KittyNotForSell,
		PaymentNotEnough,
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
			ensure!(kitty1.gender() != kitty2.gender(), Error::<T>::CanNotBreedWithSameGender);

			let selector = Self::get_random_value(&who);
			let mut dna = [0u8; 16];
			for i in 0..dna.len() {
				dna[i] = (selector[i] & kitty1.dna[i]) | (selector[i] & kitty2.dna[i]);
			}
			Self::create_kitty(dna, &who)
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn set_price(
			origin: OriginFor<T>,
			id: T::KittyId,
			price: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Kitties::<T>::get(id).is_some(), Error::<T>::KittyNotExists);
			let kitty = Kitties::<T>::get(id).unwrap();
			ensure!(who.clone() == kitty.owner, Error::<T>::NotOwnerOfKitty);

			let mut kitty = Kitties::<T>::get(id).unwrap();
			kitty.price = Some(price);
			Kitties::<T>::insert(id, Some(kitty.clone()));

			Self::deposit_event(Event::KittyPriceSet(id.clone(), price));
			Ok(())
		}

		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn buy(origin: OriginFor<T>, id: T::KittyId, payment: BalanceOf<T>) -> DispatchResult {
			let buyer = ensure_signed(origin)?;
			ensure!(Kitties::<T>::get(id).is_some(), Error::<T>::KittyNotExists);
			let kitty = Kitties::<T>::get(id).unwrap();
			ensure!(kitty.price.is_some(), Error::<T>::KittyNotForSell);
			let price = kitty.price.unwrap();
			ensure!(payment >= price, Error::<T>::PaymentNotEnough);
			T::Currency::transfer(
				&buyer,
				&kitty.owner,
				price,
				frame_support::traits::ExistenceRequirement::KeepAlive,
			)?;
			Self::transfer_kitty(&id, &kitty.owner, &buyer)
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
			T::Currency::reserve(&owner, T::HoldingDepositForOneKitty::get())?;

			let id = T::KittyId::from(count);
			Kitties::<T>::insert(id, Some(Kitty {
				dna,
				owner: owner.clone(),
				price: Option::None,
			}));
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
			let mut kitty = Kitties::<T>::get(id).unwrap();
			ensure!(owner.clone() == kitty.owner, Error::<T>::NotOwnerOfKitty);
			T::Currency::reserve(&new_owner, T::HoldingDepositForOneKitty::get())?;

			T::Currency::unreserve(&owner, T::HoldingDepositForOneKitty::get());
			kitty.owner = new_owner.clone();
			Kitties::<T>::insert(id, Some(kitty.clone()));

			Self::deposit_event(Event::KittyTransfered(
				id.clone(),
				owner.clone(),
				new_owner.clone(),
			));
			Ok(())
		}
	}

	impl<T: Config> Kitty<T> {
		pub fn gender(&self) -> Gender {
			if self.dna[0] % 2 == 0 {
				Gender::Male
			} else {
				Gender::Female
			}
		}
	}
}
