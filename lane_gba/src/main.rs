#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]
#![feature(drain_filter)]

use agb::{
    display::{
        object::{self, Graphics, Object, ObjectController, Sprite, Tag},
        HEIGHT, WIDTH,
    },
    fixnum::{Num, Rect, Vector2D},
    include_aseprite,
    input::{Button, ButtonController},
    interrupt::VBlank,
    sound::mixer::{Frequency, Mixer, SoundChannel},
};
use alloc::vec::Vec;
use lane_logic::{
    card::CardType, Direction, HeldCard, HeldCardIndex, Index, Move, MoveResult, PlaceCardMove,
    Player, Position, PushCardMove, State,
};
use slotmap::{DefaultKey, SecondaryMap};

const INCORRECT: &[u8] = agb::include_wav!("sfx/incorrect.wav");

extern crate alloc;

const CARDS: &Graphics = include_aseprite!(
    "gfx/cards.aseprite",
    "gfx/arrow-right.aseprite",
    "gfx/arrow-down.aseprite"
);

const ARROW_RIGHT: &Sprite = CARDS.tags().get("Arrow Right").sprite(0);
const ARROW_DOWN: &Sprite = CARDS.tags().get("Arrow Down").sprite(0);

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
    select: SelectBox<'controller>,
    camera_position: Vector2D<Num<i32, 8>>,
    select_arrow: Option<Object<'controller>>,
}

impl<'controller> MyState<'controller> {
    #[track_caller]
    fn average_position(&self) -> Vector2D<Num<i32, 8>> {
        let average_position: Vector2D<Num<i32, 8>> = self
            .cards
            .iter()
            .filter(|(_, card)| card.counts_to_average)
            .map(|(_, a)| a.position)
            .reduce(|a, b| a + b)
            .unwrap()
            / self.cards.len() as i32;
        average_position
    }

    fn new(initial_state: State, object: &'controller ObjectController) -> Self {
        let mut state = MyState {
            cards: Default::default(),
            playing_animations: Default::default(),
            game_state: initial_state,
            select: SelectBox {
                object: object.object_sprite(SELECT),
                state_stack: alloc::vec![SelectState::Hand { slot: 0 }],
            },
            select_arrow: None,
            camera_position: Default::default(),
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
        state.camera_position = state.average_position();

        state
    }

    fn frame(
        &mut self,
        object: &'controller ObjectController,
        input: &ButtonController,
        mixer: &mut Mixer,
    ) {
        // progress the animations
        self.update_animation();

        // Update rendered position of objects
        self.camera_position = (self.camera_position * 4 + self.average_position()) / 5;

        let position_difference: Vector2D<Num<i32, 8>> =
            Vector2D::new(WIDTH, HEIGHT).change_base() / 2 - self.camera_position;

        let screen_space = Rect::new((0, 0).into(), Vector2D::new(WIDTH, HEIGHT).change_base());

        for (_, board_card) in self.cards.iter_mut() {
            let pos = (board_card.position + position_difference - CONVERSION_FACTOR / 2).floor();
            board_card.card_object.set_position(pos);
            let bounding = Rect::new(pos, CONVERSION_FACTOR.floor());
            if bounding.touches(&screen_space) {
                board_card.card_object.show();
            } else {
                board_card.card_object.hide();
            }
            if let Some(colour) = &mut board_card.colour_object {
                colour.set_position(pos);
                colour.set_z(1);
            }
        }

        // process the select box
        if let Some(desired_move) =
            self.update_select_box(position_difference, input, object, mixer)
        {
            // validate the move is possible
            if self.game_state.can_execute_move(&desired_move) {
                // woah!
                let result = self.game_state.execute_move(&desired_move);
                self.update_representation(result, object);
                self.select.state_stack.clear();
                self.select.state_stack.push(SelectState::Hand { slot: 0 });
            } else {
                mixer.play_sound(SoundChannel::new(INCORRECT));
            }
        }
    }

    fn update_select_box(
        &mut self,
        position_difference: Vector2D<Num<i32, 8>>,
        input: &ButtonController,
        controller: &'controller ObjectController,
        mixer: &mut Mixer,
    ) -> Option<Move> {
        let input_vector: Vector2D<i32> = input.just_pressed_vector();

        match self.select.state_mut() {
            SelectState::Hand { slot } => {
                *slot = (*slot as i32 + input_vector.x)
                    .rem_euclid(self.game_state.player_hand(self.game_state.turn()).len() as i32)
                    as usize;

                if input.is_just_pressed(Button::L) {
                    agb::println!("Pressed L");
                    self.select
                        .state_stack
                        .push(SelectState::BoardSelectPosition {
                            position: (0, 0).into(),
                            reason: BoardSelect::Push,
                        });
                } else if input.is_just_pressed(Button::A) {
                    let slot = *slot;
                    agb::println!("Pressed A on card {}", slot);
                    self.select
                        .state_stack
                        .push(SelectState::BoardSelectPosition {
                            position: (0, 0).into(),
                            reason: BoardSelect::Place(slot),
                        });
                }
            }
            SelectState::BoardSelectPosition { position, reason } => {
                *position += input_vector;
                let position = *position;
                let reason = *reason;

                if input.is_just_pressed(Button::A) {
                    self.select
                        .state_stack
                        .push(SelectState::BoardSelectDirection { position, reason });
                } else if input.is_just_pressed(Button::B) {
                    self.select.state_stack.pop();
                }
                self.select.object.set_position(
                    (position.change_base().hadamard(CONVERSION_FACTOR) + position_difference
                        - CONVERSION_FACTOR / 2)
                        .floor(),
                );
            }
            SelectState::BoardSelectDirection { position, reason } => {
                let direction = Direction::from_vector(input.vector());
                let reason = *reason;
                let position = *position;

                if input.is_pressed(Button::A) {
                    let object = self
                        .select_arrow
                        .get_or_insert_with(|| controller.object_sprite(ARROW_RIGHT));

                    object.set_z(-1);
                    match direction {
                        Some(direction) => {
                            let adjustment = match direction {
                                Direction::North => {
                                    object.set_sprite(controller.sprite(ARROW_DOWN));
                                    object.set_hflip(false);
                                    object.set_vflip(true);
                                    (4, 32)
                                }
                                Direction::East => {
                                    object.set_sprite(controller.sprite(ARROW_RIGHT));
                                    object.set_hflip(false);
                                    object.set_vflip(false);
                                    (0, 4)
                                }
                                Direction::South => {
                                    object.set_sprite(controller.sprite(ARROW_DOWN));
                                    object.set_hflip(false);
                                    object.set_vflip(false);
                                    (4, 0)
                                }
                                Direction::West => {
                                    object.set_sprite(controller.sprite(ARROW_RIGHT));
                                    object.set_hflip(true);
                                    object.set_vflip(false);
                                    (32, 4)
                                }
                            }
                            .into();
                            object.set_position(
                                (position.change_base().hadamard(CONVERSION_FACTOR)
                                    + position_difference)
                                    .floor()
                                    - adjustment,
                            );
                            object.show();
                        }
                        None => {
                            object.hide();
                        }
                    }
                } else {
                    self.select_arrow = None;
                }
                if input.is_just_released(Button::A) {
                    self.select.state_stack.pop();

                    if let Some(direction) = direction {
                        // execute a move!
                        let desired_move = (|| match reason {
                            BoardSelect::Push => {
                                let (card, _place) =
                                    self.game_state.card_at_position(Position(position))?;
                                Some(Move::PushCard(PushCardMove {
                                    place: card,
                                    direction,
                                }))
                            }
                            BoardSelect::Place(index) => Some(Move::PlaceCard(PlaceCardMove {
                                direction,
                                coordinate: Position(position),
                                card: HeldCardIndex(index),
                            })),
                        })();

                        if let Some(desired_move) = desired_move {
                            return Some(desired_move);
                        } else {
                            mixer.play_sound(SoundChannel::new(INCORRECT));
                        }
                    }
                }
            }
        }

        None
    }

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
                CardAnimationStatus::Dying => { /* TODO: Death affect */ }
            }
        }

        let playing_animations = &mut self.playing_animations[0];
        let cards = &self.cards;

        for (idx, animation) in playing_animations
            .drain_filter(|(idx, animation)| match animation {
                CardAnimationStatus::Placed(pos) | CardAnimationStatus::MoveTowards(pos) => {
                    (cards[idx.to_slotmap_key()].position - *pos).manhattan_distance() < 1.into()
                }
                CardAnimationStatus::Dying => true, /* TODO: Death finaliser */
            })
            .collect::<Vec<_>>()
        {
            match animation {
                CardAnimationStatus::Placed(pos) | CardAnimationStatus::MoveTowards(pos) => {
                    self.cards[idx.to_slotmap_key()].position = pos;
                    self.cards[idx.to_slotmap_key()].counts_to_average = true;
                }
                CardAnimationStatus::Dying => {
                    self.cards.remove(idx.to_slotmap_key());
                }
            }
        }

        if self.playing_animations[0].is_empty() {
            self.playing_animations.remove(0);
        }

        CompletedAnimation::Running
    }
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
    object: Object<'controller>,
    state_stack: Vec<SelectState>,
}

impl SelectBox<'_> {
    fn state_mut(&mut self) -> &mut SelectState {
        self.state_stack
            .last_mut()
            .expect("should have the last state available")
    }

    fn state(&self) -> &SelectState {
        self.state_stack
            .last()
            .expect("should have the last state available")
    }
}

#[derive(Debug, Clone, Copy)]
enum BoardSelect {
    Push,
    Place(usize),
}

#[derive(Debug, Clone, Copy)]
enum SelectState {
    Hand {
        slot: usize,
    },
    BoardSelectPosition {
        position: Vector2D<i32>,
        reason: BoardSelect,
    },
    BoardSelectDirection {
        position: Vector2D<i32>,
        reason: BoardSelect,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CompletedAnimation {
    Completed,
    Running,
}

const CONVERSION_FACTOR: Vector2D<Num<i32, 8>> = Vector2D {
    x: Num::from_raw(16 << 8),
    y: Num::from_raw(16 << 8),
};

fn battle(gba: &mut agb::Gba) {
    let object = gba.display.object.get();

    let vblank = VBlank::get();
    let mut input = ButtonController::new();

    let mut mixer = gba.mixer.mixer(Frequency::Hz32768);
    let _irq = mixer.setup_interrupt_handler();
    mixer.enable();

    let game_state = State::new(
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

    let mut state = MyState::new(game_state, &object);

    let update = state
        .game_state
        .execute_move(&Move::PlaceCard(PlaceCardMove {
            direction: Direction::East,
            coordinate: Position((-1, 0).into()),
            card: HeldCardIndex(0),
        }));

    state.update_representation(update, &object);

    loop {
        mixer.frame();
        vblank.wait_for_vblank();
        object.commit();
        input.update();

        state.frame(&object, &input, &mut mixer);
    }
}
