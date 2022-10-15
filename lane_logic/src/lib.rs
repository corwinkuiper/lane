#![no_std]
#[warn(clippy::all)]
use core::ops::Add;

use agb_fixnum::Vector2D;

use alloc::vec::Vec;
use slotmap::HopSlotMap;

extern crate alloc;

mod card;

use card::{CardData, CardType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PushStatus {
    Success,
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
    fn to_unit_vector(self) -> Vector2D<i32> {
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
struct Position(Vector2D<i32>);

impl Add<Direction> for Position {
    type Output = Self;
    fn add(self, rhs: Direction) -> Self::Output {
        Position(self.0 + rhs.to_unit_vector())
    }
}

#[derive(Debug, Clone)]
struct PlacedCard {
    belonging_player: Player,
    position: Position,
    card: CardData,
}

#[derive(Debug, Clone)]
pub struct State {
    turn: Player,
    board: Board,
    hands: [Hand; 2],
}

#[derive(Debug, Clone)]
enum HeldCard {
    Avaliable(CardType),
    Waiting(CardType, usize),
}

#[derive(Debug, Clone)]
struct Hand {
    cards: Vec<HeldCard>,
}

impl State {
    pub fn list_moves() {}
}

#[derive(Debug, Clone)]
struct Board {
    positions: HopSlotMap<slotmap::DefaultKey, PlacedCard>,
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Index(slotmap::DefaultKey);

impl Board {
    fn start_push(&mut self, position: Position, direction: Direction) -> Set<Index> {
        let card = self.get_card_position(position);

        CardData::push(
            self,
            card.expect("should pass me something that exists"),
            direction,
        )
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
            belonging_player: owner,
            position,
            card,
        });
        Index(idx)
    }

    fn should_card_be_removed(&self, card_idx: Index) -> bool {
        let my_player = self.get_card(card_idx).unwrap().belonging_player;
        let position = self[card_idx].position;

        let outer_cards = [
            self.get_card_position(position + Direction::North),
            self.get_card_position(position + Direction::East),
            self.get_card_position(position + Direction::South),
            self.get_card_position(position + Direction::West),
        ]
        .map(|v| {
            v.map_or(false, |idx| {
                self.get_card(idx).unwrap().belonging_player == my_player
            })
        });

        (outer_cards[Direction::North as usize] && outer_cards[Direction::South as usize])
            || (outer_cards[Direction::East as usize] && outer_cards[Direction::West as usize])
    }

    fn remove_cards(&mut self) -> Vec<PlacedCard> {
        let mut removed = Vec::new();
        for (idx, _) in self.positions.iter() {
            if self.should_card_be_removed(Index(idx)) {
                removed.push(Index(idx));
            }
        }
        removed
            .iter()
            .map(|idx| self.positions.remove(idx.0).unwrap())
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

pub struct PlaceCardMove {
    direction: Direction,
    coordinate: Position,
    card: CardType,
}

pub struct PushCardMove {
    place: Index,
    direction: Direction,
}

pub enum Move {
    PlaceCard(PlaceCardMove),
    PushCard(PushCardMove),
}

pub struct PlaceCardResult {}

pub struct MoveCardResult {}

pub struct RemoveCardResult {}

pub struct MoveResult {
    placed: Vec<PlaceCardResult>,
    moved: Vec<MoveCardResult>,
    removed: Vec<RemoveCardResult>,
    winner: Option<Player>,
}

type Set<I> = alloc::collections::BTreeSet<I>;
