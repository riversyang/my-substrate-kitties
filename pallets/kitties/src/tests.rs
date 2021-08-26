use crate::{mock::*, Error};
use frame_support::{assert_noop, assert_ok};

#[test]
fn create_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Kitties::create(Origin::signed(1)));
		assert_eq!(Kitties::kitties_count(), Some(1 as u32));
		System::assert_last_event(Event::Kitties(crate::Event::KittyCreated(1)));
	});
}

#[test]
fn adopt_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Kitties::create(Origin::signed(1)));

		let balance_before_adopt = Balances::free_balance(1);
		assert_ok!(Kitties::adopt(Origin::signed(1), 1));
		System::assert_last_event(Event::Kitties(crate::Event::KittyAdopted(1, 1)));
		assert_eq!(balance_before_adopt - Balances::free_balance(1), 10_000);

		assert_noop!(Kitties::adopt(Origin::signed(1), 2), Error::<Test>::KittyNotExists);
		assert_noop!(
			Kitties::adopt(Origin::signed(2), 1),
			Error::<Test>::CanNotAdoptKittyWithAnOwner
		);
	});
}

#[test]
fn abandon_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Kitties::create(Origin::signed(1)));
		assert_ok!(Kitties::adopt(Origin::signed(1), 1));

		assert_noop!(Kitties::abandon(Origin::signed(1), 2), Error::<Test>::KittyNotExists);
		assert_noop!(Kitties::abandon(Origin::signed(2), 1), Error::<Test>::NotOwnerOfKitty);

		let balance_before_adopt = Balances::free_balance(1);
		assert_ok!(Kitties::abandon(Origin::signed(1), 1));
		System::assert_last_event(Event::Kitties(crate::Event::KittyAbandoned(1)));
		assert_eq!(Balances::free_balance(1) - balance_before_adopt, 10_000);
		assert_eq!(Kitties::kitties_price(1), Option::None);

		assert_ok!(Kitties::adopt(Origin::signed(2), 1));
	});
}

#[test]
fn transfer_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Kitties::create(Origin::signed(1)));
		assert_ok!(Kitties::adopt(Origin::signed(1), 1));

		assert_noop!(Kitties::transfer(Origin::signed(1), 2, 2), Error::<Test>::KittyNotExists);
		assert_noop!(Kitties::transfer(Origin::signed(2), 1, 3), Error::<Test>::NotOwnerOfKitty);
		let owner_balance_before_transfer = Balances::free_balance(1);
		let new_owner_balance_before_transfer = Balances::free_balance(2);
		assert_ok!(Kitties::transfer(Origin::signed(1), 1, 2));
		System::assert_last_event(Event::Kitties(crate::Event::KittyTransfered(1, 1, 2)));
		assert_eq!(Kitties::kitties_owner(1), Some(2));
		assert_eq!(Balances::free_balance(1) - owner_balance_before_transfer, 10_000);
		assert_eq!(new_owner_balance_before_transfer - Balances::free_balance(2), 10_000);
	});
}

#[test]
fn breed_works() {
	new_test_ext().execute_with(|| {
		let mut block_number = 1;
		System::set_block_number(block_number);
		assert_ok!(Kitties::create(Origin::signed(1)));
		block_number += 1;
		System::set_block_number(block_number);
		assert_ok!(Kitties::create(Origin::signed(1)));
		let kitty1 = Kitties::kitties(1).unwrap();
		let mut kitty2 = Kitties::kitties(2).unwrap();
		let mut kitty2_index = 2;
		if kitty1.gender() == kitty2.gender() {
			assert_noop!(
				Kitties::breed(Origin::signed(1), 1, 2),
				Error::<Test>::CanNotBreedWithSameGender
			);
			loop {
				block_number += 1;
				System::set_block_number(block_number);
				assert_ok!(Kitties::create(Origin::signed(1)));
				kitty2_index = Kitties::kitties_count().unwrap();
				kitty2 = Kitties::kitties(kitty2_index).unwrap();
				if kitty2.gender() != kitty1.gender() {
					break;
				}
			}
		}
		block_number += 1;
		System::set_block_number(block_number);
		assert_ok!(Kitties::breed(Origin::signed(1), 1, kitty2_index));
		let new_kitty_index = Kitties::kitties_count().unwrap();
		System::assert_last_event(Event::Kitties(crate::Event::KittyBorn(
			new_kitty_index,
			1,
			kitty2_index,
		)));
		assert_eq!(Kitties::kitties_owner(new_kitty_index), Option::None);
	});
}

#[test]
fn set_and_clear_price_works() {
	new_test_ext().execute_with(|| {
		System::set_block_number(1);
		assert_ok!(Kitties::create(Origin::signed(1)));
		assert_ok!(Kitties::adopt(Origin::signed(1), 1));

		assert_noop!(Kitties::set_price(Origin::signed(1), 2, 200), Error::<Test>::KittyNotExists);
		assert_noop!(Kitties::set_price(Origin::signed(2), 1, 200), Error::<Test>::NotOwnerOfKitty);
		assert_eq!(Kitties::kitties_price(1), Option::None);
		assert_ok!(Kitties::set_price(Origin::signed(1), 1, 200));
		System::assert_last_event(Event::Kitties(crate::Event::KittyPriceSet(1, 200)));
		assert_eq!(Kitties::kitties_price(1), Some(200));

		assert_noop!(Kitties::clear_price(Origin::signed(1), 2), Error::<Test>::KittyNotExists);
		assert_noop!(Kitties::clear_price(Origin::signed(2), 1), Error::<Test>::NotOwnerOfKitty);
		assert_ok!(Kitties::clear_price(Origin::signed(1), 1));
		System::assert_last_event(Event::Kitties(crate::Event::KittyPriceCleared(1)));
		assert_eq!(Kitties::kitties_price(1), Option::None);
	});
}

#[test]
fn buy_works() {
	new_test_ext().execute_with(|| {
		let mut block_number = 1;
		System::set_block_number(block_number);
		assert_ok!(Kitties::create(Origin::signed(1)));
		block_number += 1;
		System::set_block_number(block_number);
		assert_ok!(Kitties::create(Origin::signed(1)));
		assert_ok!(Kitties::adopt(Origin::signed(1), 1));

		assert_noop!(Kitties::buy(Origin::signed(1), 3), Error::<Test>::KittyNotExists);
		assert_noop!(Kitties::buy(Origin::signed(1), 2), Error::<Test>::NoNeedToBuyKittyWithoutAnOwner);
		assert_noop!(Kitties::buy(Origin::signed(1), 1), Error::<Test>::KittyNotForSell);

		assert_ok!(Kitties::set_price(Origin::signed(1), 1, 200_000));
		let owner_balance_before_transfer = Balances::free_balance(1);
		let new_owner_balance_before_transfer = Balances::free_balance(2);
		assert_ok!(Kitties::buy(Origin::signed(2), 1));
		System::assert_last_event(Event::Kitties(crate::Event::KittySold(1, 1, 2, 200_000)));
		assert_eq!(Kitties::kitties_owner(1), Some(2));
		assert_eq!(Balances::free_balance(1) - owner_balance_before_transfer, 210_000);
		assert_eq!(new_owner_balance_before_transfer - Balances::free_balance(2), 210_000);
		assert_eq!(Kitties::kitties_price(1), Option::None);
	});
}
