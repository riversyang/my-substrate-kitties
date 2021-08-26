#![cfg_attr(not(feature = "std"), no_std)]

pub use pallet::*;

#[frame_support::pallet]
pub mod pallet {
	use codec::{Decode, Encode, HasCompact};
	use frame_support::{
		dispatch::DispatchResult,
		fail,
		pallet_prelude::*,
		traits::{Currency, Randomness, ReservableCurrency, Time},
		Printable,
	};
	use frame_system::pallet_prelude::*;
	use sp_io::hashing::blake2_128;

	type BalanceOf<T> =
		<<T as Config>::Currency as Currency<<T as frame_system::Config>::AccountId>>::Balance;
	type MomentOf<T> = <<T as Config>::Time as Time>::Moment;

	#[derive(Clone, Encode, Decode)]
	pub struct Kitty<T: Config> {
		pub dna: [u8; 16],
		pub birth_time: MomentOf<T>,
	}

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
		type KittyId: From<u32>
			+ Member
			+ Parameter
			+ Default
			+ Copy
			+ HasCompact
			+ MaxEncodedLen
			+ Printable;
		/// The currency trait.
		type Currency: ReservableCurrency<Self::AccountId>;
		/// The owner of kitty must reserve a certain amount of currency
		#[pallet::constant]
		type HoldingDepositForOneKitty: Get<BalanceOf<Self>>;
		/// Time
		type Time: Time;
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
		StorageMap<_, Blake2_128Concat, T::KittyId, Kitty<T>, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn kitties_owner)]
	pub type KittiesOwner<T: Config> =
		StorageMap<_, Blake2_128Concat, T::KittyId, T::AccountId, OptionQuery>;

	#[pallet::storage]
	#[pallet::getter(fn kitties_price)]
	pub type KittiesPrice<T: Config> =
		StorageMap<_, Blake2_128Concat, T::KittyId, BalanceOf<T>, OptionQuery>;

	#[pallet::event]
	#[pallet::metadata(T::AccountId = "AccountId")]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		KittyCreated(T::KittyId),
		KittyTransfered(T::KittyId, T::AccountId, T::AccountId),
		KittyBorn(T::KittyId, T::KittyId, T::KittyId),
		KittyAbandoned(T::KittyId),
		KittyAdopted(T::KittyId, T::AccountId),
		KittyPriceSet(T::KittyId, BalanceOf<T>),
		KittyPriceCleared(T::KittyId),
		KittySold(T::KittyId, T::AccountId, T::AccountId, BalanceOf<T>),
	}

	#[pallet::error]
	pub enum Error<T> {
		KittiesCountOverflow,
		KittyNotExists,
		NotOwnerOfKitty,
		CanNotAdoptKittyWithAnOwner,
		CanNotBreedWithSameGender,
		KittyNotForSell,
		PaymentNotEnough,
		NoNeedToBuyKittyWithoutAnOwner,
	}

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		/// Create a new kitty.
		///
		/// The owner of new kitty is left empty, which means it can be 'adopted'.
		/// Todo: apply that this function should only be called by 'God', who is the supervisor of this pallet.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn create(origin: OriginFor<T>) -> DispatchResult {
			let who = ensure_signed(origin)?;
			let dna = Self::get_random_value(&who);

			let id = Self::create_kitty(dna)?;

			Self::deposit_event(Event::KittyCreated(id));
			Ok(())
		}

		/// Simple transfer a kitty to another one without any fee.
		///
		/// This function can only be called by the owner of the kitty.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn transfer(
			origin: OriginFor<T>,
			id: T::KittyId,
			new_owner: T::AccountId,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_owner(&id, &who)?;

			Self::transfer_kitty(&id, &who, &new_owner)
		}

		/// Let two kitties to breed.
		///
		/// The two kitties MUST have different genders.
		/// The person who help breeding will NOT become the owner of new born kitty automatically.
		/// The owner of new born kitty is left empty, which means it can be 'adopted'.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn breed(origin: OriginFor<T>, id1: T::KittyId, id2: T::KittyId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Kitties::<T>::contains_key(id1), Error::<T>::KittyNotExists);
			ensure!(Kitties::<T>::contains_key(id2), Error::<T>::KittyNotExists);

			let id = Self::breed_kitty(&id1, &id2, &who)?;

			Self::deposit_event(Event::KittyBorn(id, id1, id2));
			Ok(())
		}

		/// Abandon a kitty, clear its owner.
		///
		/// This function can only be called by the owner of the kitty.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn abandon(origin: OriginFor<T>, id: T::KittyId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			Self::ensure_owner(&id, &who)?;

			T::Currency::unreserve(&who, T::HoldingDepositForOneKitty::get());
			KittiesOwner::<T>::remove(id);
			KittiesPrice::<T>::remove(id);

			Self::deposit_event(Event::KittyAbandoned(id.clone()));
			Ok(())
		}

		/// Adopt a kitty without an owner.
		///
		/// The adoption will reserve a certain amount of Balance from the adoptor.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn adopt(origin: OriginFor<T>, id: T::KittyId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(!KittiesOwner::<T>::contains_key(id), Error::<T>::CanNotAdoptKittyWithAnOwner);

			T::Currency::reserve(&who, T::HoldingDepositForOneKitty::get())?;
			KittiesOwner::<T>::insert(id, who.clone());

			Self::deposit_event(Event::KittyAdopted(id.clone(), who));
			Ok(())
		}

		/// Set price for a kitty, indicate that the kitty is for sell.
		///
		/// This function can only be called by the owner of the kitty.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn set_price(
			origin: OriginFor<T>,
			id: T::KittyId,
			price: BalanceOf<T>,
		) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Kitties::<T>::contains_key(id), Error::<T>::KittyNotExists);
			Self::ensure_owner(&id, &who)?;

			KittiesPrice::<T>::insert(id, price);

			Self::deposit_event(Event::KittyPriceSet(id.clone(), price));
			Ok(())
		}

		/// Clear price for a kitty, indicate that the kitty is NOT for sell.
		///
		/// This function can only be called by the owner of the kitty.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn clear_price(origin: OriginFor<T>, id: T::KittyId) -> DispatchResult {
			let who = ensure_signed(origin)?;
			ensure!(Kitties::<T>::contains_key(id), Error::<T>::KittyNotExists);
			Self::ensure_owner(&id, &who)?;

			KittiesPrice::<T>::remove(id);

			Self::deposit_event(Event::KittyPriceCleared(id.clone()));
			Ok(())
		}

		/// Buy a kitty that was priced
		///
		/// Only a kitty with price (and of course with an owner) can be bought.
		#[pallet::weight(10_000 + T::DbWeight::get().writes(1))]
		pub fn buy(origin: OriginFor<T>, id: T::KittyId, payment: BalanceOf<T>) -> DispatchResult {
			let buyer = ensure_signed(origin)?;
			ensure!(Kitties::<T>::contains_key(id), Error::<T>::KittyNotExists);
			let owner = match KittiesOwner::<T>::get(id) {
				Some(owner) => owner,
				None => fail!(Error::<T>::NoNeedToBuyKittyWithoutAnOwner),
			};
			let price = match KittiesPrice::<T>::get(id) {
				Some(price) => {
					ensure!(payment >= price, Error::<T>::PaymentNotEnough);
					price
				}
				None => fail!(Error::<T>::KittyNotForSell),
			};

			T::Currency::transfer(
				&buyer,
				&owner,
				price,
				frame_support::traits::ExistenceRequirement::KeepAlive,
			)?;
			Self::transfer_kitty(&id, &owner, &buyer)?;
			// The price for the kitty must be cleared after transfer it to new owner,
			// or it can be bought by other people.
			KittiesPrice::<T>::remove(id);

			Self::deposit_event(Event::KittySold(
				id.clone(),
				owner.clone(),
				buyer.clone(),
				payment,
			));
			Ok(())
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

		fn get_next_kitty_id() -> Result<(T::KittyId, u32), DispatchError> {
			let count = match Self::kitties_count() {
				Some(count) => {
					ensure!(count != u32::MAX, Error::<T>::KittiesCountOverflow);
					count + 1
				}
				None => 1,
			};
			Ok((T::KittyId::from(count), count))
		}

		fn _kitties_count() {}

		fn create_kitty(dna: [u8; 16]) -> Result<T::KittyId, DispatchError> {
			let (id, count) = Self::get_next_kitty_id()?;
			Kitties::<T>::insert(id, Kitty { dna, birth_time: T::Time::now() });
			KittiesCount::<T>::put(count);

			Ok(id)
		}

		fn breed_kitty(
			id1: &T::KittyId,
			id2: &T::KittyId,
			who: &T::AccountId,
		) -> Result<T::KittyId, DispatchError> {
			let kitty1 = Kitties::<T>::get(id1).unwrap();
			let kitty2 = Kitties::<T>::get(id2).unwrap();
			ensure!(kitty1.gender() != kitty2.gender(), Error::<T>::CanNotBreedWithSameGender);

			let selector = Self::get_random_value(&who);
			let mut dna = [0u8; 16];
			for i in 0..dna.len() {
				dna[i] = (selector[i] & kitty1.dna[i]) | (selector[i] & kitty2.dna[i]);
			}
			Self::create_kitty(dna)
		}

		fn ensure_owner(id: &T::KittyId, owner: &T::AccountId) -> DispatchResult {
			match KittiesOwner::<T>::get(id) {
				Some(kitty_owner) => {
					ensure!(owner.clone() == kitty_owner, Error::<T>::NotOwnerOfKitty);
					Ok(())
				}
				None => fail!(Error::<T>::NotOwnerOfKitty),
			}
		}

		fn transfer_kitty(
			id: &T::KittyId,
			owner: &T::AccountId,
			new_owner: &T::AccountId,
		) -> DispatchResult {
			T::Currency::reserve(&new_owner, T::HoldingDepositForOneKitty::get())?;

			T::Currency::unreserve(&owner, T::HoldingDepositForOneKitty::get());
			KittiesOwner::<T>::insert(id, new_owner.clone());

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
