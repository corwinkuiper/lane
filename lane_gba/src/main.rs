#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]
#![feature(drain_filter)]

use agb::{
    display::{
        object::{Graphics, Object, ObjectController, Sprite, Tag},
        HEIGHT, WIDTH,
    },
    fixnum::{Num, Vector2D},
    include_aseprite,
    input::{Button, ButtonController},
    interrupt::VBlank,
};
use alloc::vec::Vec;
use lane_logic::{
    card::CardType, Direction, HeldCard, HeldCardIndex, Index, Move, MoveResult, PlaceCardMove,
    Player, Position, State,
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

struct MyState<'controller> {
    cards: SecondaryMap<DefaultKey, CardOnBoard<'controller>>,
    playing_animations: Vec<Vec<(Index, CardAnimationStatus)>>,
    game_state: State,
}

struct CardOnBoard<'controller> {
    card_object: Object<'controller>,
    colour_object: Option<Object<'controller>>,
    position: Vector2D<Num<i32, 8>>,
    counts_to_average: bool,
}

enum CardAnimationStatus {
    Placed(Vector2D<Num<i32, 8>>),
    MoveTowards(Vector2D<Num<i32, 8>>),
    Dying,
}

struct SelectBox<'controller> {
    position: Vector2D<i32>,
    object: Object<'controller>,
    state: SelectState,
}

enum SelectState {
    Hand,
    BoardPush,
    Place,
}

impl<'controller> MyState<'controller> {
    fn update_representation(&mut self, update: MoveResult, object: &'controller ObjectController) {
        // add the newly placed cards
        for (idx, direction, new_card) in &update.placed {
            self.cards.insert(
                idx.to_slotmap_key(),
                CardOnBoard {
                    card_object: object.object_sprite(card_type_to_sprite(new_card.card.to_type())),
                    colour_object: new_card
                        .belonging_player
                        .map(|player| object.object_sprite(colour_for_player(player))),
                    position: new_card
                        .position
                        .0
                        .change_base()
                        .hadamard(CONVERSION_FACTOR)
                        - direction
                            .to_unit_vector()
                            .change_base()
                            .hadamard(CONVERSION_FACTOR)
                            * 10,
                    counts_to_average: false,
                },
            );
        }
        self.playing_animations.push(
            update
                .placed
                .iter()
                .map(|(idx, _, new_card)| {
                    (
                        *idx,
                        CardAnimationStatus::Placed(
                            new_card
                                .position
                                .0
                                .change_base()
                                .hadamard(CONVERSION_FACTOR),
                        ),
                    )
                })
                .collect(),
        );

        self.playing_animations.push(
            update
                .moved
                .iter()
                .map(|(idx, card)| {
                    (
                        *idx,
                        CardAnimationStatus::MoveTowards(
                            card.position.0.change_base().hadamard(CONVERSION_FACTOR),
                        ),
                    )
                })
                .collect(),
        );

        self.playing_animations.push(
            update
                .removed
                .iter()
                .map(|(idx, _)| (*idx, CardAnimationStatus::Dying))
                .collect(),
        );
    }

    fn update_animation(&mut self) -> CompletedAnimation {
        if self.playing_animations.is_empty() {
            return CompletedAnimation::Completed;
        }

        let animations_to_run = &self.playing_animations[0];

        for (card, animation) in animations_to_run {
            match animation {
                CardAnimationStatus::Placed(destination) => {
                    let current = self.cards[card.to_slotmap_key()].position;
                    self.cards[card.to_slotmap_key()].position += (*destination - current)
                        .fast_normalise()
                        * ((*destination - current).manhattan_distance().min(8.into()))
                }
                CardAnimationStatus::MoveTowards(destination) => {
                    let current = self.cards[card.to_slotmap_key()].position;
                    self.cards[card.to_slotmap_key()].position += (*destination - current)
                        .fast_normalise()
                        * ((*destination - current).manhattan_distance().min(4.into()))
                }
                CardAnimationStatus::Dying => todo!(),
            }
        }

        let playing_animations = &mut self.playing_animations[0];
        let cards = &self.cards;

        for (idx, animation) in playing_animations
            .drain_filter(|(idx, animation)| match animation {
                CardAnimationStatus::Placed(pos) | CardAnimationStatus::MoveTowards(pos) => {
                    (cards[idx.to_slotmap_key()].position - *pos).manhattan_distance() < 1.into()
                }
                CardAnimationStatus::Dying => todo!(),
            })
            .collect::<Vec<_>>()
        {
            match animation {
                CardAnimationStatus::Placed(pos) | CardAnimationStatus::MoveTowards(pos) => {
                    self.cards[idx.to_slotmap_key()].position = pos;
                    self.cards[idx.to_slotmap_key()].counts_to_average = true;
                }
                CardAnimationStatus::Dying => todo!(),
            }
        }

        if self.playing_animations[0].is_empty() {
            self.playing_animations.remove(0);
        }

        CompletedAnimation::Running
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletedAnimation {
    Completed,
    Running,
}

const CONVERSION_FACTOR: Vector2D<Num<i32, 8>> = Vector2D {
    x: Num::from_raw(32 << 8),
    y: Num::from_raw(32 << 8),
};

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

    let my_board_state: SecondaryMap<DefaultKey, CardOnBoard> = SecondaryMap::new();

    let mut state = MyState {
        cards: my_board_state,
        playing_animations: Vec::new(),
        game_state,
    };

    for (idx, card) in state.game_state.board_state() {
        state.cards.insert(
            idx.to_slotmap_key(),
            CardOnBoard {
                card_object: object.object_sprite(card_type_to_sprite(card.card.to_type())),
                colour_object: card
                    .belonging_player
                    .map(|p| object.object_sprite(colour_for_player(p))),
                position: CONVERSION_FACTOR.hadamard(card.position.0.change_base()),
                counts_to_average: true,
            },
        );
    }

    let update = state
        .game_state
        .execute_move(Move::PlaceCard(PlaceCardMove {
            direction: Direction::East,
            coordinate: Position((-1, 0).into()),
            card: HeldCardIndex(0),
        }));

    state.update_representation(update, &object);

    let mut select_box = SelectBox {
        position: (0, 0).into(),
        object: object.object_sprite(SELECT),
        state: SelectState::BoardPush,
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

        let average_position: Vector2D<Num<i32, 8>> = state
            .cards
            .iter()
            .filter(|(_, card)| card.counts_to_average)
            .map(|(_, a)| a.position)
            .reduce(|a, b| a + b)
            .unwrap()
            / state.cards.len() as i32;

        let position_difference: Vector2D<Num<i32, 8>> =
            Vector2D::new(WIDTH, HEIGHT).change_base() / 2 - average_position;

        select_box.object.set_position(
            (select_box
                .position
                .change_base()
                .hadamard(CONVERSION_FACTOR)
                + position_difference
                - CONVERSION_FACTOR / 2)
                .floor(),
        );

        state.update_animation();

        for (_, board_card) in state.cards.iter_mut() {
            let pos = (board_card.position + position_difference - CONVERSION_FACTOR / 2).floor();
            board_card.card_object.set_position(pos);
            board_card.card_object.show();
            if let Some(colour) = &mut board_card.colour_object {
                colour.set_position(pos);
                colour.set_z(1);
            }
        }
    }
}
