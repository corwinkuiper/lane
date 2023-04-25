#![no_std]
#![warn(clippy::all)]
use core::ops::Add;
use core::{future::Future, hash::Hash, ops::Neg};

use agb_fixnum::Vector2D;

use alloc::vec::Vec;
use hashbrown::HashMap;
use slotmap::HopSlotMap;
mod rustc_hash;

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

const DIRECTIONS: [Direction; 4] = [
    Direction::North,
    Direction::East,
    Direction::South,
    Direction::West,
];

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

    pub fn from_vector(v: Vector2D<i32>) -> Option<Self> {
        match (v.x, v.y) {
            (0, -1) => Some(Direction::North),
            (1, 0) => Some(Direction::East),
            (0, 1) => Some(Direction::South),
            (-1, 0) => Some(Direction::West),
            _ => None,
        }
    }
}

impl Neg for Direction {
    type Output = Self;

    fn neg(self) -> Self::Output {
        match self {
            Direction::North => Direction::South,
            Direction::East => Direction::West,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Player {
    A,
    B,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct Position(pub Vector2D<i32>);

impl Hash for Position {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.0.x.hash(state);
        self.0.y.hash(state);
    }
}

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
    pub fn turn(&self) -> Player {
        self.turn
    }

    pub fn can_execute_move(&self, m: &Move) -> bool {
        match m {
            Move::PlaceCard(place) => match self.hands[self.turn as usize].cards[place.card.0] {
                HeldCard::Avaliable(card) => {
                    self.board
                        .no_cards_in_direction(place.coordinate, -place.direction)
                        && self.board.get_card_position(place.coordinate).is_none()
                        && self
                            .board
                            .can_place(card, self.turn, place.coordinate, place.direction)
                            == PlaceStatus::Success
                }
                HeldCard::Waiting { .. } => false,
            },
            Move::PushCard(push) => {
                self.board
                    .get_card(push.place)
                    .map_or(false, |c| c.belonging_player == Some(self.turn))
                    && match self.board.can_push(push.place, push.direction) {
                        PushStatus::Success(1..) => true,
                        PushStatus::Success(0) => false,
                        PushStatus::Fail => false,
                    }
            }
            Move::PickCard(pick) => self
                .board
                .get_card(pick.card)
                .map_or(false, |c| c.belonging_player == Some(self.turn)),
        }
    }

    pub fn execute_move(&mut self, m: &Move) -> MoveResult {
        let (placed, moved, mut picked) = match m {
            Move::PlaceCard(place) => match self.hands[self.turn as usize].cards[place.card.0] {
                HeldCard::Avaliable(card) => {
                    self.hands[self.turn as usize].cards.remove(place.card.0);

                    let (new_card, moved_cards) =
                        self.board
                            .start_place(card, self.turn, place.coordinate, place.direction);

                    (
                        alloc::vec![(
                            new_card,
                            place.direction,
                            PlacedCard {
                                belonging_player: Some(self.turn),
                                position: place.coordinate,
                                card: card.to_data()
                            }
                        )],
                        moved_cards,
                        Vec::new(),
                    )
                }
                HeldCard::Waiting { .. } => panic!("invalid move"),
            },
            Move::PushCard(push) => (
                Vec::new(),
                self.board.start_push(push.place, push.direction),
                Vec::new(),
            ),
            Move::PickCard(pick) => {
                let card = self.board.remove_card(pick.card);
                (Vec::new(), Set::new(), alloc::vec![(pick.card, card)])
            }
        };

        for card_in_hand in self.hands.iter_mut().flat_map(|x| x.cards.iter_mut()) {
            match card_in_hand {
                HeldCard::Avaliable(_) => {}
                HeldCard::Waiting {
                    card,
                    turns_until_usable,
                } => {
                    *turns_until_usable -= 1;
                    if *turns_until_usable == 0 {
                        *card_in_hand = HeldCard::Avaliable(*card);
                    }
                }
            }
        }

        let moved = moved
            .iter()
            .map(|&idx| (idx, self.board[idx].clone()))
            .collect();

        let mut removed = self.board.remove_cards();
        removed.append(&mut picked);

        for (_, card) in removed.iter() {
            if let Some(player) = card.belonging_player {
                self.hands[player as usize].cards.push(HeldCard::Waiting {
                    card: card.card.to_type(),
                    turns_until_usable: 1,
                });
            }
        }

        let score = self.scores();

        let winner = match (score.player(Player::A) >= 4, score.player(Player::B) >= 4) {
            (true, false) => Some(Player::A),
            (false, true) => Some(Player::B),
            (_, _) => None,
        };

        self.turn = match self.turn {
            Player::A => Player::B,
            Player::B => Player::A,
        };

        MoveResult {
            placed,
            moved,
            removed,
            winner,
            score,
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

    pub fn card(&self, idx: Index) -> Option<&PlacedCard> {
        self.board.get_card(idx)
    }

    pub fn turn_hand(&self) -> &[HeldCard] {
        self.player_hand(self.turn())
    }

    pub fn scores(&self) -> Score {
        Score {
            scores: self.board.score(),
        }
    }

    pub fn enumerate_possible_moves(&self) -> Vec<Move> {
        let mut moves = Vec::new();

        // create list of positions that is valid to place
        let mut possible = Vec::new();

        for (_idx, card) in self.board.positions.iter() {
            for direction in DIRECTIONS {
                let desired_spot = card.position + direction;
                if self.board.no_cards_in_direction(card.position, direction) {
                    possible.push((desired_spot, -direction));
                }
            }
        }

        // go over the hand of the current player
        for (idx, card) in self.turn_hand().iter().enumerate() {
            if let HeldCard::Avaliable(_card) = card {
                for (position, direction) in possible.iter().copied() {
                    moves.push(Move::PlaceCard(PlaceCardMove {
                        direction,
                        coordinate: position,
                        card: HeldCardIndex(idx),
                    }))
                }
            }
        }

        // all possible pushing moves
        for (idx, card) in self.board.positions.iter() {
            if card.belonging_player != Some(self.turn()) {
                continue;
            }

            moves.push(Move::PickCard(PickCardMove { card: Index(idx) }));

            for direction in DIRECTIONS {
                let m = Move::PushCard(PushCardMove {
                    place: Index(idx),
                    direction,
                });
                if self.can_execute_move(&m) {
                    moves.push(m);
                }
            }
        }

        moves
    }

    pub async fn enumerate_possible_moves_async<F, Fut>(&self, yeild: F) -> Vec<Move>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = ()>,
    {
        let mut moves = Vec::new();

        // create list of positions that is valid to place
        let mut possible = Vec::new();

        for (_idx, card) in self.board.positions.iter() {
            for direction in DIRECTIONS {
                let desired_spot = card.position + direction;
                if self.board.no_cards_in_direction(card.position, direction) {
                    possible.push((desired_spot, -direction));
                }
                yeild().await;
            }
        }

        // go over the hand of the current player
        for (idx, card) in self.turn_hand().iter().enumerate() {
            if let HeldCard::Avaliable(_card) = card {
                for (position, direction) in possible.iter().copied() {
                    moves.push(Move::PlaceCard(PlaceCardMove {
                        direction,
                        coordinate: position,
                        card: HeldCardIndex(idx),
                    }));
                    yeild().await;
                }
            }
        }
        yeild().await;

        // all possible pushing moves
        for (idx, card) in self.board.positions.iter() {
            if card.belonging_player != Some(self.turn()) {
                continue;
            }

            yeild().await;

            moves.push(Move::PickCard(PickCardMove { card: Index(idx) }));

            for direction in DIRECTIONS {
                let m = Move::PushCard(PushCardMove {
                    place: Index(idx),
                    direction,
                });
                if self.can_execute_move(&m) {
                    moves.push(m);
                }
                yeild().await;
            }
        }

        moves
    }
}

#[derive(Debug, Clone)]
struct Board {
    positions: HopSlotMap<slotmap::DefaultKey, PlacedCard>,
    position_cache: HashMap<Position, Index, rustc_hash::FxBuildHasher>,
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

        let k1 = pos.insert(PlacedCard {
            belonging_player: None,
            position: Position((0, 0).into()),
            card: CardType::Score.to_data(),
        });
        let k2 = pos.insert(PlacedCard {
            belonging_player: None,
            position: Position((1, 0).into()),
            card: CardType::Score.to_data(),
        });

        let mut map = HashMap::with_hasher(rustc_hash::FxBuildHasher);

        map.insert(Position((0, 0).into()), Index(k1));
        map.insert(Position((1, 0).into()), Index(k2));
        Self {
            positions: pos,
            position_cache: map,
        }
    }

    fn score(&self) -> [usize; 2] {
        let mut scores = [0, 0];

        let mut score_cards = Vec::new();

        for (_idx, card) in self.positions.iter() {
            if card.card.to_type() == CardType::Score {
                score_cards.push(card.position);
            }
        }

        let directions = [
            Direction::North,
            Direction::East,
            Direction::South,
            Direction::West,
        ];

        let scoring_cards = score_cards
            .iter()
            .flat_map(|&x| directions.iter().map(move |&y| x + y))
            .flat_map(|x| self.get_card_position(x))
            .collect::<Set<_>>();

        for idx in scoring_cards {
            if let Some(player) = self.get_card(idx).unwrap().belonging_player {
                scores[player as usize] += 1;
            }
        }

        scores
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
        self.position_cache.get(&position).copied()
    }

    fn move_card(&mut self, current_position: Position, next_position: Position) {
        let old = self
            .position_cache
            .remove(&current_position)
            .expect("the given position should exist");
        self[old].position = next_position;
        self.position_cache.insert(next_position, old);
    }

    fn remove_card(&mut self, idx: Index) -> PlacedCard {
        let card = self.positions.remove(idx.0).unwrap();
        self.position_cache.remove(&card.position);
        card
    }

    fn add_card(&mut self, owner: Player, position: Position, card: CardData) -> Index {
        let idx = self.positions.insert(PlacedCard {
            belonging_player: Some(owner),
            position,
            card,
        });
        self.position_cache.insert(position, Index(idx));
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
                let card = self.get_card(idx).unwrap().belonging_player;
                card.is_some() && card != my_player
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
            .map(|&idx| (idx, self.remove_card(idx)))
            .collect()
    }

    fn no_cards_in_direction(&self, position: Position, direction: Direction) -> bool {
        for (_, card) in self.positions.iter() {
            match direction {
                Direction::North => {
                    if card.position.0.x == position.0.x && card.position.0.y < position.0.y {
                        return false;
                    }
                }
                Direction::East => {
                    if card.position.0.y == position.0.y && card.position.0.x > position.0.x {
                        return false;
                    }
                }
                Direction::South => {
                    if card.position.0.x == position.0.x && card.position.0.y > position.0.y {
                        return false;
                    }
                }
                Direction::West => {
                    if card.position.0.y == position.0.y && card.position.0.x < position.0.x {
                        return false;
                    }
                }
            }
        }

        true
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
}

pub struct PushCardMove {
    pub place: Index,
    pub direction: Direction,
}

pub struct PickCardMove {
    pub card: Index,
}

pub enum Move {
    PlaceCard(PlaceCardMove),
    PushCard(PushCardMove),
    PickCard(PickCardMove),
}

#[derive(Debug)]
pub struct MoveResult {
    pub placed: Vec<(Index, Direction, PlacedCard)>,
    pub moved: Vec<(Index, PlacedCard)>,
    pub removed: Vec<(Index, PlacedCard)>,
    pub winner: Option<Player>,
    pub score: Score,
}

#[derive(Debug)]
pub struct Score {
    scores: [usize; 2],
}

impl Score {
    pub fn player(&self, player: Player) -> usize {
        self.scores[player as usize]
    }
}

type Set<I> = alloc::collections::BTreeSet<I>;
