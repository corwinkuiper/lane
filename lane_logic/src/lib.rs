#![no_std]
#[warn(clippy::all)]
use core::ops::Add;

use agb_fixnum::Vector2D;

use alloc::vec::Vec;
use slotmap::HopSlotMap;

extern crate alloc;

pub mod card;

use card::{CardData, CardType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushStatus {
    Success(u32),
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaceStatus {
    Success,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    North,
    East,
    South,
    West,
}

impl Direction {
    pub fn to_unit_vector(self) -> Vector2D<i32> {
        match self {
            Direction::North => (0, -1),
            Direction::East => (1, 0),
            Direction::South => (0, 1),
            Direction::West => (-1, 0),
        }
        .into()
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Player {
    A,
    B,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Position(pub Vector2D<i32>);

impl Add<Direction> for Position {
    type Output = Self;
    fn add(self, rhs: Direction) -> Self::Output {
        Position(self.0 + rhs.to_unit_vector())
    }
}

#[derive(Debug, Clone)]
pub struct PlacedCard {
    pub belonging_player: Option<Player>,
    pub position: Position,
    pub card: CardData,
}

#[derive(Debug, Clone)]
pub struct State {
    turn: Player,
    board: Board,
    hands: [Hand; 2],
}

#[derive(Debug, Clone)]
pub enum HeldCard {
    Avaliable(CardType),
    Waiting {
        card: CardType,
        turns_until_usable: usize,
    },
}

#[derive(Debug, Clone)]
struct Hand {
    cards: Vec<HeldCard>,
}

impl Hand {
    fn new(cards: Vec<HeldCard>) -> Self {
        Hand { cards }
    }
}

impl State {
    pub fn can_execute_move(&self, m: Move) -> bool {
        match m {
            Move::PlaceCard(place) => match self.hands[self.turn as usize].cards[place.card.0] {
                HeldCard::Avaliable(card) => {
                    self.board
                        .can_place(card, place.player, place.coordinate, place.direction)
                        == PlaceStatus::Success
                }
                HeldCard::Waiting {
                    card: _,
                    turns_until_usable: _,
                } => false,
            },
            Move::PushCard(push) => match self.board.can_push(push.place, push.direction) {
                PushStatus::Success(1..) => true,
                PushStatus::Success(0) => false,
                PushStatus::Fail => false,
            },
        }
    }

    pub fn execute_move(&mut self, m: Move) -> MoveResult {
        let (placed, moved) = match m {
            Move::PlaceCard(place) => match self.hands[self.turn as usize].cards[place.card.0] {
                HeldCard::Avaliable(card) => {
                    self.hands[self.turn as usize].cards.remove(place.card.0);

                    let (new_card, moved_cards) = self.board.start_place(
                        card,
                        place.player,
                        place.coordinate,
                        place.direction,
                    );

                    (Some(new_card), moved_cards)
                }
                HeldCard::Waiting {
                    card: _,
                    turns_until_usable: _,
                } => panic!("invalid move"),
            },
            Move::PushCard(push) => (None, self.board.start_push(push.place, push.direction)),
        };

        let removed = self.board.remove_cards();

        let placed = match placed {
            Some(placed) => alloc::vec![placed],
            None => Vec::new(),
        };

        let moved = moved.iter().cloned().collect();
        let removed = removed.iter().map(|(idx, _)| idx).cloned().collect();
        let winner = None;

        self.turn = match self.turn {
            Player::A => Player::B,
            Player::B => Player::A,
        };

        MoveResult {
            placed,
            moved,
            removed,
            winner,
        }
    }

    pub fn new(player_a: Vec<HeldCard>, player_b: Vec<HeldCard>, starting_player: Player) -> Self {
        State {
            turn: starting_player,
            board: Board::new(),
            hands: [Hand::new(player_a), Hand::new(player_b)],
        }
    }

    pub fn player_hand(&self, player: Player) -> &[HeldCard] {
        &self.hands[player as usize].cards
    }

    pub fn board_state(&self) -> impl Iterator<Item = (Index, &PlacedCard)> {
        self.board.positions.iter().map(|(k, v)| (Index(k), v))
    }

    pub fn card_at_position(&self, pos: Position) -> Option<(Index, &PlacedCard)> {
        self.board
            .get_card_position(pos)
            .map(|i| (i, &self.board[i]))
    }
}

#[derive(Debug, Clone)]
struct Board {
    positions: HopSlotMap<slotmap::DefaultKey, PlacedCard>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Index(slotmap::DefaultKey);

impl Index {
    pub fn to_slotmap_key(self) -> slotmap::DefaultKey {
        self.0
    }
}

impl Board {
    fn new() -> Self {
        let mut pos = HopSlotMap::new();

        pos.insert(PlacedCard {
            belonging_player: None,
            position: Position((0, 0).into()),
            card: CardType::Score.to_data(),
        });
        pos.insert(PlacedCard {
            belonging_player: None,
            position: Position((0, 1).into()),
            card: CardType::Score.to_data(),
        });

        Self { positions: pos }
    }

    fn start_push(&mut self, idx: Index, direction: Direction) -> Set<Index> {
        CardData::push(self, idx, direction)
    }

    fn start_place(
        &mut self,
        card: CardType,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> (Index, Set<Index>) {
        CardData::place(self, card, player, position, direction)
    }

    fn can_push(&self, idx: Index, direction: Direction) -> PushStatus {
        CardData::can_push(self, idx, direction)
    }

    fn can_place(
        &self,
        card: CardType,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> PlaceStatus {
        CardData::can_place(self, card, player, position, direction)
    }

    fn get_card(&self, idx: Index) -> Option<&PlacedCard> {
        self.positions.get(idx.0)
    }

    fn get_card_position(&self, position: Position) -> Option<Index> {
        self.positions
            .iter()
            .find(|(_, card)| card.position == position)
            .map(|(key, _)| Index(key))
    }

    fn get_card_mut(&mut self, idx: Index) -> Option<&mut PlacedCard> {
        self.positions.get_mut(idx.0)
    }

    fn remove_card(&mut self, idx: Index) {
        self.positions.remove(idx.0);
    }

    fn add_card(&mut self, owner: Player, position: Position, card: CardData) -> Index {
        let idx = self.positions.insert(PlacedCard {
            belonging_player: Some(owner),
            position,
            card,
        });
        Index(idx)
    }

    fn should_card_be_removed(&self, card_idx: Index) -> bool {
        let my_player = self.get_card(card_idx).unwrap().belonging_player;
        let position = self[card_idx].position;

        if my_player.is_none() {
            // don't kill cards not owned by players (probably scoring cards)
            return false;
        }

        let outer_cards = [
            self.get_card_position(position + Direction::North),
            self.get_card_position(position + Direction::East),
            self.get_card_position(position + Direction::South),
            self.get_card_position(position + Direction::West),
        ]
        .map(|v| {
            v.map_or(false, |idx| {
                self.get_card(idx).unwrap().belonging_player != my_player
            })
        });

        (outer_cards[Direction::North as usize] && outer_cards[Direction::South as usize])
            || (outer_cards[Direction::East as usize] && outer_cards[Direction::West as usize])
    }

    fn remove_cards(&mut self) -> Vec<(Index, PlacedCard)> {
        let mut removed = Vec::new();
        for (idx, _) in self.positions.iter() {
            if self.should_card_be_removed(Index(idx)) {
                removed.push(Index(idx));
            }
        }
        removed
            .iter()
            .map(|&idx| (idx, self.positions.remove(idx.0).unwrap()))
            .collect()
    }
}

impl core::ops::Index<Index> for Board {
    type Output = PlacedCard;

    fn index(&self, index: Index) -> &Self::Output {
        &self.positions[index.0]
    }
}

impl core::ops::IndexMut<Index> for Board {
    fn index_mut(&mut self, index: Index) -> &mut Self::Output {
        &mut self.positions[index.0]
    }
}

pub struct HeldCardIndex(pub usize);

pub struct PlaceCardMove {
    pub direction: Direction,
    pub coordinate: Position,
    pub card: HeldCardIndex,
    pub player: Player,
}

pub struct PushCardMove {
    place: Index,
    direction: Direction,
}

pub enum Move {
    PlaceCard(PlaceCardMove),
    PushCard(PushCardMove),
}

pub struct MoveResult {
    pub placed: Vec<Index>,
    pub moved: Vec<Index>,
    pub removed: Vec<Index>,
    pub winner: Option<Player>,
}

type Set<I> = alloc::collections::BTreeSet<I>;
