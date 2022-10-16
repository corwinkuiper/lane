#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]

use agb::{
    display::{
        object::{Graphics, Object, Sprite, Tag},
        HEIGHT, WIDTH,
    },
    fixnum::{Num, Vector2D},
    include_aseprite,
    input::{Button, ButtonController},
    interrupt::VBlank,
};
use lane_logic::{
    card::CardType, Direction, HeldCard, HeldCardIndex, Move, PlaceCardMove, Player, Position,
    State,
};
use slotmap::{DefaultKey, SecondaryMap};

extern crate alloc;

const CARDS: &Graphics = include_aseprite!("gfx/cards.aseprite");

const SELECT: &Sprite = CARDS.tags().get("Select").sprite(0);

fn card_type_to_sprite(t: CardType) -> &'static Sprite {
    macro_rules! deconstify {
        ($t: expr) => {{
            const A: &'static Tag = $t;
            A
        }};
    }
    match t {
        CardType::Block => deconstify!(CARDS.tags().get("Block")).sprite(0),
        CardType::Normal => deconstify!(CARDS.tags().get("Normal")).sprite(0),
        CardType::Double => deconstify!(CARDS.tags().get("Double")).sprite(0),
        CardType::Ghost => deconstify!(CARDS.tags().get("Ghost")).sprite(0),
        CardType::Score => deconstify!(CARDS.tags().get("Score")).sprite(0),
    }
}

fn colour_for_player(t: Player) -> &'static Sprite {
    macro_rules! deconstify {
        ($t: expr) => {{
            const A: &'static Tag = $t;
            A
        }};
    }
    match t {
        Player::A => deconstify!(CARDS.tags().get("Green")).sprite(0),
        Player::B => deconstify!(CARDS.tags().get("Blue")).sprite(0),
    }
}

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    battle(&mut gba);

    panic!("not supposed to get here!");
}

struct CardOnBoard<'controller> {
    card_object: Object<'controller>,
    colour_object: Option<Object<'controller>>,
    position: Vector2D<Num<i32, 8>>,
    animation: CardAnimationStatus,
}

enum CardAnimationStatus {
    Stationary,
    Placed(Vector2D<Num<i32, 8>>),
    MoveTowards(Vector2D<Num<i32, 8>>),
    Dying,
}

struct SelectBox<'controller> {
    position: Vector2D<i32>,
    object: Object<'controller>,
}

fn battle(gba: &mut agb::Gba) {
    let object = gba.display.object.get();

    let vblank = VBlank::get();
    let mut input = ButtonController::new();

    let mut game_state = State::new(
        alloc::vec![
            HeldCard::Avaliable(CardType::Double),
            HeldCard::Avaliable(CardType::Normal),
            HeldCard::Avaliable(CardType::Normal),
            HeldCard::Avaliable(CardType::Ghost)
        ],
        alloc::vec![
            HeldCard::Avaliable(CardType::Double),
            HeldCard::Avaliable(CardType::Normal),
            HeldCard::Avaliable(CardType::Normal),
            HeldCard::Avaliable(CardType::Ghost)
        ],
        Player::A,
    );

    let conversion_factor: Vector2D<Num<i32, 8>> = (32, 32).into();

    let mut my_board_state: SecondaryMap<DefaultKey, CardOnBoard> = SecondaryMap::new();

    for (idx, card) in game_state.board_state() {
        my_board_state.insert(
            idx.to_slotmap_key(),
            CardOnBoard {
                card_object: object.object_sprite(card_type_to_sprite(card.card.to_type())),
                colour_object: card
                    .belonging_player
                    .map(|p| object.object_sprite(colour_for_player(p))),
                position: conversion_factor.hadamard(card.position.0.change_base()),
                animation: CardAnimationStatus::Stationary,
            },
        );
    }

    let mut select_box = SelectBox {
        position: (0, 0).into(),
        object: object.object_sprite(SELECT),
    };
    select_box.object.set_z(-1);

    loop {
        vblank.wait_for_vblank();
        object.commit();

        input.update();

        select_box.position += (
            (input.is_just_pressed(Button::RIGHT) as i32
                - input.is_just_pressed(Button::LEFT) as i32),
            (input.is_just_pressed(Button::DOWN) as i32 - input.is_just_pressed(Button::UP) as i32),
        )
            .into();

        let average_position: Vector2D<Num<i32, 8>> = my_board_state
            .iter()
            .map(|(_, a)| a.position)
            .reduce(|a, b| a + b)
            .unwrap()
            / my_board_state.len() as i32;

        let position_difference: Vector2D<Num<i32, 8>> =
            Vector2D::new(WIDTH, HEIGHT).change_base() / 2 - average_position;

        select_box.object.set_position(
            (select_box
                .position
                .change_base()
                .hadamard(conversion_factor)
                + position_difference
                - conversion_factor / 2)
                .floor(),
        );

        for (_, board_card) in my_board_state.iter_mut() {
            let pos = (board_card.position + position_difference - conversion_factor / 2).floor();
            board_card.card_object.set_position(pos);
            board_card.card_object.show();
            if let Some(colour) = &mut board_card.colour_object {
                colour.set_position(pos);
                colour.set_z(1);
            }
        }
    }
}
