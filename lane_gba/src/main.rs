#![no_std]
#![no_main]
#![cfg_attr(test, feature(custom_test_frameworks))]
#![cfg_attr(test, reexport_test_harness_main = "test_main")]
#![cfg_attr(test, test_runner(agb::test_runner::test_runner))]
#![feature(drain_filter)]
#![feature(allocator_api)]

use core::cell::RefCell;

use agb::{
    display::{
        object::{Graphics, Object, ObjectController, Sprite, Tag},
        tiled::{
            DynamicTile, MapLoan, RegularBackgroundSize, RegularMap, TileSetting, TiledMap,
            VRamManager,
        },
        Font, Priority, HEIGHT, WIDTH,
    },
    fixnum::{Num, Rect, Vector2D},
    include_aseprite,
    input::{Button, ButtonController},
    interrupt::{Interrupt, VBlank},
    sound::mixer::{Frequency, Mixer, SoundChannel},
};
use alloc::vec::Vec;
use async_evaluator::Evaluator;
use game_tree_search::{AIControl, ControlMode};
use lane_logic::{
    card::CardType, Direction, HeldCard, HeldCardIndex, Index, Move, MoveResult, PickCardMove,
    PlaceCardMove, Player, Position, PushCardMove, State,
};
use slotmap::{DefaultKey, SecondaryMap};

mod async_evaluator;
mod game_tree_search;

const FONT_20: Font = agb::include_font!("fnt/VCR_OSD_MONO_1.001.ttf", 20);

const FONT_15: Font = agb::include_font!("fnt/VCR_OSD_MONO_1.001.ttf", 15);

const INCORRECT: &[u8] = agb::include_wav!("sfx/incorrect.wav");

extern crate alloc;

const CARDS: &Graphics = include_aseprite!(
    "gfx/cards.aseprite",
    "gfx/arrow-right.aseprite",
    "gfx/arrow-down.aseprite",
    "gfx/cards_double.aseprite",
    "gfx/action.aseprite",
    "gfx/chevron.aseprite"
);
const CHEVRON: &Sprite = CARDS.tags().get("Chevron").sprite(0);
const ARROW_RIGHT: &Sprite = CARDS.tags().get("Arrow Right").sprite(0);
const ARROW_DOWN: &Sprite = CARDS.tags().get("Arrow Down").sprite(0);

const REFRESH: &Sprite = CARDS.tags().get("Refresh Double").sprite(0);

const SELECT: &Sprite = CARDS.tags().get("Select").sprite(0);
const SELECT_DOUBLE: &Sprite = CARDS.tags().get("Select Double").sprite(0);

const PICK: &Tag = CARDS.tags().get("Pick");
const PUSH: &Tag = CARDS.tags().get("Push");

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

fn card_type_to_sprite_double(t: CardType) -> &'static Sprite {
    macro_rules! deconstify {
        ($t: expr) => {{
            const A: &'static Tag = $t;
            A
        }};
    }
    match t {
        CardType::Block => deconstify!(CARDS.tags().get("Block Double")).sprite(0),
        CardType::Normal => deconstify!(CARDS.tags().get("Normal Double")).sprite(0),
        CardType::Double => deconstify!(CARDS.tags().get("Double Double")).sprite(0),
        CardType::Ghost => deconstify!(CARDS.tags().get("Ghost Double")).sprite(0),
        CardType::Score => deconstify!(CARDS.tags().get("Score Double")).sprite(0),
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

fn colour_for_player_double(t: Player) -> &'static Sprite {
    macro_rules! deconstify {
        ($t: expr) => {{
            const A: &'static Tag = $t;
            A
        }};
    }
    match t {
        Player::A => deconstify!(CARDS.tags().get("Green Double")).sprite(0),
        Player::B => deconstify!(CARDS.tags().get("Blue Double")).sprite(0),
    }
}

#[agb::entry]
fn main(mut gba: agb::Gba) -> ! {
    battle(&mut gba);

    panic!("not supposed to get here!");
}

struct CardInHand<'controller> {
    card_object: Object<'controller>,
    colour_object: Object<'controller>,
    cached_position: Vector2D<i32>,
}

struct MyState<'controller> {
    cards: SecondaryMap<DefaultKey, CardOnBoard<'controller>>,
    playing_animations: Vec<Vec<(Index, CardAnimationStatus)>>,
    game_state: State,
    select: SelectBox<'controller>,
    camera_position: Vector2D<Num<i32, 8>>,
    select_arrow: Option<Object<'controller>>,
    hand: Vec<CardInHand<'controller>>,
    move_finder: Option<Evaluator<Option<Move>>>,
    control_mode: ControlMode,
    pick_help: PickHelp<'controller>,
    winner: Option<Player>,
}

struct PickHelp<'controller> {
    pick: Object<'controller>,
    push: Object<'controller>,
}

impl<'controller> PickHelp<'controller> {
    fn new(object: &'controller ObjectController) -> Self {
        let mut pick = object.object_sprite(PICK.sprite(0));
        pick.set_position((WIDTH - 32, 0).into());
        pick.set_z(-10);

        let mut push = object.object_sprite(PUSH.sprite(0));
        push.set_position((0, 0).into());
        push.set_z(-10);

        Self { pick, push }
    }

    fn hide(&mut self) {
        self.pick.hide();
        self.push.hide();
    }

    fn show(&mut self) {
        self.pick.show();
        self.push.show();
    }

    fn pick(&mut self, object: &'controller ObjectController) {
        self.pick.set_sprite(object.sprite(PICK.sprite(1)));
        self.push.set_sprite(object.sprite(PUSH.sprite(0)));
    }

    fn push(&mut self, object: &'controller ObjectController) {
        self.pick.set_sprite(object.sprite(PICK.sprite(0)));
        self.push.set_sprite(object.sprite(PUSH.sprite(1)));
    }

    fn reset(&mut self, object: &'controller ObjectController) {
        self.pick.set_sprite(object.sprite(PICK.sprite(0)));
        self.push.set_sprite(object.sprite(PUSH.sprite(0)));
    }
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

    fn update_hand_objects(&mut self, object: &'controller ObjectController) {
        self.hand.clear();
        let player = self.game_state.turn();
        let held_cards = self.game_state.turn_hand();

        let number_of_held_cards = held_cards.len();

        let first_x = (WIDTH / 2) - number_of_held_cards as i32 * 34 / 2;

        for (count, card_in_hand) in held_cards.iter().enumerate() {
            let count = count as i32;
            let position = (first_x + 34 * count, HEIGHT - 34).into();

            match card_in_hand {
                HeldCard::Avaliable(card) => {
                    let mut held = CardInHand {
                        card_object: object.object_sprite(card_type_to_sprite_double(*card)),
                        colour_object: object.object_sprite(colour_for_player_double(player)),
                        cached_position: position,
                    };

                    held.card_object.set_position(position);
                    held.colour_object.set_position(position);
                    held.colour_object.set_z(1);
                    self.hand.push(held);
                }
                HeldCard::Waiting {
                    card,
                    turns_until_usable: _,
                } => {
                    let mut held = CardInHand {
                        card_object: object.object_sprite(card_type_to_sprite_double(*card)),
                        colour_object: object.object_sprite(REFRESH),
                        cached_position: position,
                    };

                    held.card_object.set_position(position);
                    held.colour_object.set_position(position);
                    held.colour_object.set_z(1);
                    self.hand.push(held);
                }
            }
        }
    }

    fn new(
        initial_state: State,
        object: &'controller ObjectController,
        control: ControlMode,
    ) -> Self {
        let mut state = MyState {
            cards: Default::default(),
            playing_animations: Default::default(),
            game_state: initial_state,
            select: SelectBox {
                object: object.object_sprite(SELECT),
                pick_box: object.object_sprite(SELECT_DOUBLE),
                state_stack: alloc::vec![SelectState::Hand { slot: 0 }],
            },
            select_arrow: None,
            camera_position: Default::default(),
            hand: Vec::new(),
            move_finder: None,
            control_mode: control,
            pick_help: PickHelp::new(object),
            winner: None,
        };

        state.pick_help.hide();

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
        state.camera_position = state.average_position() + (1, 1).into();

        state
    }

    fn frame(
        &mut self,
        object: &'controller ObjectController,
        input: &ButtonController,
        mixer: &mut Mixer,
        text: &mut TextRender,
    ) {
        // progress the animations
        self.update_animation();

        // Update rendered position of objects

        let old_camera_position = self.camera_position;
        self.camera_position = (self.camera_position * 4 + self.average_position()) / 5;

        let position_difference: Vector2D<Num<i32, 8>> =
            Vector2D::new(WIDTH, HEIGHT - 36).change_base() / 2 - self.camera_position;

        let screen_space = Rect::new((0, 0).into(), Vector2D::new(WIDTH, HEIGHT).change_base());

        if !self.playing_animations.is_empty() || self.camera_position != old_camera_position {
            for (_, board_card) in self.cards.iter_mut() {
                let pos =
                    (board_card.position + position_difference - CONVERSION_FACTOR / 2).floor();
                board_card.card_object.set_position(pos);
                let bounding = Rect::new(pos, CONVERSION_FACTOR.floor());
                board_card.card_object.set_priority(Priority::P1);
                board_card
                    .colour_object
                    .as_mut()
                    .map(|f| f.set_priority(Priority::P1));
                if bounding.touches(screen_space) {
                    board_card.card_object.show();
                    board_card.colour_object.as_mut().map(|f| f.show());
                } else {
                    board_card.card_object.hide();
                    board_card.colour_object.as_mut().map(|f| f.hide());
                }
                if let Some(colour) = &mut board_card.colour_object {
                    colour.set_position(pos);
                    colour.set_z(1);
                }
            }
        }

        if self.winner.is_none() && self.move_finder.is_none() {
            match self.control_mode {
                ControlMode::TwoHuman => {}
                ControlMode::AI(ai, player) => {
                    if player == self.game_state.turn() {
                        self.prepare_ai_turn(ai);
                    }
                }
                ControlMode::TwoAI(ai1, ai2) => {
                    match self.game_state.turn() {
                        Player::A => self.prepare_ai_turn(ai1),
                        Player::B => self.prepare_ai_turn(ai2),
                    };
                }
            }
        }

        if self.playing_animations.is_empty() && self.winner.is_none() {
            match self.control_mode {
                ControlMode::TwoHuman => {
                    self.do_human_turn(position_difference, input, object, mixer);
                    match self.winner {
                        Some(Player::A) => text.write(
                            &FONT_20,
                            (10_u16, 10_u16).into(),
                            format_args!("The winner is\n  Player A"),
                        ),
                        Some(Player::B) => text.write(
                            &FONT_20,
                            (10_u16, 10_u16).into(),
                            format_args!("The winner is\n  Player B"),
                        ),
                        None => {}
                    }
                }
                ControlMode::AI(ai, player) => {
                    if player == self.game_state.turn() {
                        self.do_ai_turn(ai, object);
                    } else {
                        self.do_human_turn(position_difference, input, object, mixer);
                    }

                    if let Some(p) = self.winner {
                        if p == player {
                            text.write(
                                &FONT_20,
                                (5_u16, 5_u16).into(),
                                format_args!("The winner is\nThe Computer"),
                            )
                        } else {
                            text.write(
                                &FONT_20,
                                (5_u16, 5_u16).into(),
                                format_args!("The winner is\n     You"),
                            )
                        }
                    }
                }
                ControlMode::TwoAI(ai1, ai2) => {
                    match self.game_state.turn() {
                        Player::A => self.do_ai_turn(ai1, object),
                        Player::B => self.do_ai_turn(ai2, object),
                    };
                    match self.winner {
                        Some(Player::A) => text.write(
                            &FONT_20,
                            (5_u16, 5_u16).into(),
                            format_args!("The winner is\n    AI 1"),
                        ),
                        Some(Player::B) => text.write(
                            &FONT_20,
                            (5_u16, 5_u16).into(),
                            format_args!("The winner is\n    AI 2"),
                        ),
                        None => {}
                    }
                }
            };

            if self.winner.is_some() {
                text.write(
                    &FONT_15,
                    (8_u16, 18_u16).into(),
                    format_args!("Press Start"),
                );
            }
        }
    }

    fn prepare_ai_turn(&mut self, ai_mode: AIControl) {
        self.move_finder
            .get_or_insert_with(|| ai_mode.move_finder(self.game_state.clone()));
    }

    fn do_ai_turn(
        &mut self,
        ai_mode: AIControl,
        object: &'controller ObjectController,
    ) -> Option<MoveResult> {
        self.select.object.hide();
        self.select.pick_box.hide();
        self.pick_help.hide();

        let move_finder = self
            .move_finder
            .get_or_insert_with(|| ai_mode.move_finder(self.game_state.clone()));

        if let Some(m) = move_finder.result() {
            let m = m.as_ref().unwrap();
            let result = self.game_state.execute_move(m);

            self.winner = result.winner;

            self.update_representation(&result, object);

            self.move_finder = None;

            Some(result)
        } else {
            None
        }
    }

    fn do_human_turn(
        &mut self,
        position_difference: Vector2D<Num<i32, 8>>,
        input: &ButtonController,
        object: &'controller ObjectController,
        mixer: &mut Mixer,
    ) -> Option<MoveResult> {
        self.pick_help.show();

        if self.hand.is_empty() {
            self.update_hand_objects(object);
        }
        if let Some(desired_move) =
            self.update_select_box(position_difference, input, object, mixer)
        {
            // validate the move is possible
            if self.game_state.can_execute_move(&desired_move) {
                // woah!
                let result = self.game_state.execute_move(&desired_move);

                self.winner = result.winner;

                self.update_representation(&result, object);

                self.select.state_stack.clear();
                self.select.state_stack.push(SelectState::Hand { slot: 0 });
                self.hand.clear();
                self.select.pick_box.hide();

                self.select.object.hide();

                Some(result)
            } else {
                mixer.play_sound(SoundChannel::new(INCORRECT));
                None
            }
        } else {
            None
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
                let num_cards = self.game_state.turn_hand().len();

                self.pick_help.reset(controller);

                if num_cards != 0 {
                    *slot = (*slot as i32 + input_vector.x).rem_euclid(num_cards as i32) as usize;

                    let slot = *slot;

                    if input.is_just_pressed(Button::A) {
                        let slot = slot;
                        agb::println!("Pressed A on card {}", slot);
                        self.select
                            .state_stack
                            .push(SelectState::BoardSelectPosition {
                                position: (0, 0).into(),
                                reason: BoardSelect::Place(slot),
                            });
                    }
                    self.select
                        .object
                        .set_sprite(controller.sprite(SELECT_DOUBLE));
                    self.select
                        .object
                        .set_position(self.hand[slot].cached_position);

                    self.select.object.show();
                } else {
                    self.select.object.hide();
                }
                if input.is_just_pressed(Button::L) {
                    agb::println!("Pressed L");
                    self.select
                        .state_stack
                        .push(SelectState::BoardSelectPosition {
                            position: (0, 0).into(),
                            reason: BoardSelect::Push,
                        });
                } else if input.is_just_pressed(Button::R) {
                    self.select
                        .state_stack
                        .push(SelectState::BoardSelectPosition {
                            position: (0, 0).into(),
                            reason: BoardSelect::Pick,
                        });
                }
                self.select.pick_box.hide();
            }
            SelectState::BoardSelectPosition { position, reason } => {
                *position += input_vector;
                let position = *position;
                let reason = *reason;
                self.select.object.show();
                self.select.object.set_sprite(controller.sprite(SELECT));

                match reason {
                    BoardSelect::Push => self.pick_help.push(controller),
                    BoardSelect::Pick => self.pick_help.pick(controller),
                    BoardSelect::Place(idx) => {
                        self.select
                            .pick_box
                            .set_position(self.hand[idx].cached_position);
                        self.select.pick_box.show();
                    }
                }

                if input.is_just_pressed(Button::A) {
                    if reason == BoardSelect::Pick {
                        if let Some(card) = self.game_state.card_at_position(Position(position)) {
                            return Some(Move::PickCard(PickCardMove { card: card.0 }));
                        } else {
                            mixer.play_sound(SoundChannel::new(INCORRECT));
                        }
                    } else {
                        self.select
                            .state_stack
                            .push(SelectState::BoardSelectDirection { position, reason });
                    }
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
                if input.is_just_released(Button::A) && reason != BoardSelect::Pick {
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
                            BoardSelect::Pick => None,
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

    fn update_representation(
        &mut self,
        update: &MoveResult,
        object: &'controller ObjectController,
    ) {
        // add the newly placed cards
        for (idx, direction, new_card) in &update.placed {
            self.cards.insert(
                idx.to_slotmap_key(),
                CardOnBoard {
                    card_object: {
                        let mut obj =
                            object.object_sprite(card_type_to_sprite(new_card.card.to_type()));
                        obj.hide();
                        obj
                    },
                    colour_object: new_card.belonging_player.map(|player| {
                        let mut obj = object.object_sprite(colour_for_player(player));
                        obj.hide();
                        obj
                    }),
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
    pick_box: Object<'controller>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BoardSelect {
    Push,
    Place(usize),
    Pick,
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

struct TextRender<'gfx> {
    bg: MapLoan<'gfx, RegularMap>,
    vram: &'gfx RefCell<VRamManager>,
    tile: DynamicTile<'gfx>,
}

impl<'gfx> TextRender<'gfx> {
    fn new(bg: MapLoan<'gfx, RegularMap>, vram: &'gfx RefCell<VRamManager>) -> Self {
        let dyn_tile = vram.borrow_mut().new_dynamic_tile().fill_with(0);
        let mut tr = TextRender {
            bg,
            vram,
            tile: dyn_tile,
        };
        tr.clear();
        tr.bg.show();
        tr
    }

    fn commit(&mut self) {
        self.bg.commit(&mut self.vram.borrow_mut());
    }

    fn clear(&mut self) {
        let vram = &mut self.vram.borrow_mut();

        for y in 0..20u16 {
            for x in 0..30u16 {
                self.bg.set_tile(
                    vram,
                    (x, y).into(),
                    &self.tile.tile_set(),
                    TileSetting::from_raw(self.tile.tile_index()),
                );
            }
        }
    }

    fn write(&mut self, font: &Font, position: Vector2D<u16>, output: core::fmt::Arguments) {
        use core::fmt::Write;

        let vram = &mut self.vram.borrow_mut();
        let mut writer = font.render_text(position);
        {
            let mut writer = writer.writer(1, 0, &mut self.bg, vram);
            let _ = write!(&mut writer, "{}", output);
        }

        writer.commit(&mut self.bg, vram);
    }
}

impl Drop for TextRender<'_> {
    fn drop(&mut self) {
        self.bg.clear(&mut self.vram.borrow_mut());
    }
}

struct MenuCursor<'controller> {
    object_left: Object<'controller>,
    object_right: Object<'controller>,
    position: usize,
}

struct Menu<'controller> {
    cursor: MenuCursor<'controller>,
}

const MENU_OPTIONS: &[&str] = &["Trivial", "Medium", "Hard", "Watch", "Pass the Console"];

impl<'controller> Menu<'controller> {
    fn new(object: &'controller ObjectController, text: &mut TextRender) -> Self {
        text.write(
            &FONT_15,
            (5_u16, 2_u16).into(),
            format_args!("{}", MENU_OPTIONS.join("\n")),
        );

        let mut object_left = object.object_sprite(CHEVRON);
        let mut object_right = object.object_sprite(CHEVRON);

        object_left.hide();
        object_right.hide();

        object_right.set_hflip(true);

        Self {
            cursor: MenuCursor {
                object_left,
                object_right,
                position: 0,
            },
        }
    }

    fn frame(&mut self, input: &ButtonController) -> Option<ControlMode> {
        self.cursor.position = ((self.cursor.position as i32) + input.just_pressed_y_tri() as i32)
            .rem_euclid(MENU_OPTIONS.len() as i32) as usize;

        let y_pos = 2 * 8 + 16 * self.cursor.position as i32 + 3;

        let x_pos = 5 * 8 + 9 * MENU_OPTIONS[self.cursor.position].len() as i32 + 8;

        self.cursor.object_left.set_position((3 * 8, y_pos).into());
        self.cursor.object_right.set_position((x_pos, y_pos).into());

        self.cursor.object_left.show();
        self.cursor.object_right.show();

        if input.is_just_pressed(Button::A) {
            match self.cursor.position {
                0 => Some(ControlMode::AI(AIControl::Negative, Player::B)),
                1 => Some(ControlMode::AI(AIControl::WithRandom(50), Player::B)),
                2 => Some(ControlMode::AI(AIControl::Best, Player::B)),
                3 => Some(ControlMode::TwoAI(
                    AIControl::WithRandom(4),
                    AIControl::WithRandom(4),
                )),
                4 => Some(ControlMode::TwoHuman),
                _ => unreachable!(),
            }
        } else {
            None
        }
    }
}

fn battle(gba: &mut agb::Gba) {
    let object = gba.display.object.get();
    let (gfx, mut vram) = gba.display.video.tiled0();

    vram.set_background_palette_raw(&[0x0000, 0xffff]);

    let vram_cell = RefCell::new(vram);

    let mut text_render = TextRender::new(
        gfx.background(Priority::P0, RegularBackgroundSize::Background32x32),
        &vram_cell,
    );

    let vblank = VBlank::get();
    let mut input = ButtonController::new();

    let mut mixer = gba.mixer.mixer(Frequency::Hz32768);
    mixer.enable();

    let game_state = State::new(
        alloc::vec![
            HeldCard::Avaliable(CardType::Double),
            HeldCard::Avaliable(CardType::Normal),
            HeldCard::Avaliable(CardType::Normal),
            HeldCard::Avaliable(CardType::Block),
            HeldCard::Avaliable(CardType::Ghost)
        ],
        alloc::vec![
            HeldCard::Avaliable(CardType::Double),
            HeldCard::Avaliable(CardType::Normal),
            HeldCard::Avaliable(CardType::Normal),
            HeldCard::Avaliable(CardType::Block),
            HeldCard::Avaliable(CardType::Ghost)
        ],
        Player::A,
    );

    loop {
        let mode = {
            let mut menu = Menu::new(&object, &mut text_render);

            loop {
                mixer.frame();
                vblank.wait_for_vblank();
                text_render.commit();
                object.commit();
                input.update();
                let _ = agb::rng::gen();

                if let Some(mode) = menu.frame(&input) {
                    break mode;
                }
            }
        };

        let frame_counter = agb::sync::Static::new(0);
        let mut expected_frame_counter = frame_counter.read();

        let _v_frame_count = agb::interrupt::add_interrupt_handler(Interrupt::VBlank, |_cs| {
            frame_counter.write(frame_counter.read() + 1);
        });

        text_render.clear();
        {
            let mut state = MyState::new(game_state.clone(), &object, mode);

            loop {
                mixer.frame();
                let before_move_finder = get_vcount();

                if frame_counter.read() == expected_frame_counter {
                    if let Some(finder) = &mut state.move_finder {
                        while get_vcount() >= 160 {
                            if finder.do_work().is_some() {
                                break;
                            }
                        }
                        while get_vcount() < 100 {
                            if finder.do_work().is_some() {
                                break;
                            }
                        }
                    }
                }
                expected_frame_counter = frame_counter.read() + 1;

                let finish_clock = get_vcount();

                vblank.wait_for_vblank();
                text_render.commit();
                object.commit();
                input.update();

                agb::println!("Between {} and {}", before_move_finder, finish_clock);

                state.frame(&object, &input, &mut mixer, &mut text_render);

                if input.is_just_pressed(Button::START) && state.winner.is_some() {
                    break;
                }
            }
        }
        text_render.clear();
    }
}

fn get_vcount() -> u32 {
    unsafe { (0x0400_0006 as *mut u16).read_volatile() as u32 }
}
