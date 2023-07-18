use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

trait Card: Default + core::fmt::Debug + Clone {
    fn push(board: &mut Board, self_index: Index, direction: Direction, depth: usize)
        -> Set<Index>;
    fn can_push(board: &Board, self_index: Index, direction: Direction, depth: usize)
        -> PushStatus;
    fn can_place(
        board: &Board,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> PlaceStatus;
    fn place(
        board: &mut Board,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> (Index, Set<Index>);
}

macro_rules! create_card_data{
    ($name:ident, $type_name:ident, $( $card_type: ident ),+) => {

        #[derive(Debug, Clone)]
        pub enum $name {
            $($card_type ($card_type)),+
        }

        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum $type_name {
            $($card_type),+
        }

        impl $type_name {
            pub(crate) fn to_data(self) -> $name {
                match self {
                    $( $type_name::$card_type => $name::$card_type(Default::default())),+
                }
            }
        }

        impl $name {
            pub fn to_type(&self) -> $type_name {
                match self {
                    $( $name::$card_type(_) => $type_name::$card_type),+
                }
            }

            fn is_of_type(&self, card_type: $type_name) -> bool {
                self.to_type() == card_type
            }
        }

        impl $name {
            fn pusher(&self) -> fn(&mut Board, Index, Direction, usize) -> Set<Index> {
                match self {
                    $( $name::$card_type(_) => $card_type::push),+
                }
            }

            fn can_pusher(&self) -> fn(&Board, Index, Direction, usize) -> PushStatus {
                match self {
                    $( $name::$card_type(_) => $card_type::can_push),+
                }
            }

            pub(crate) fn place(
                board: &mut Board,
                card: $type_name,
                player: Player,
                position: Position,
                direction: Direction,
            ) -> (Index, Set<Index>) {
                match card {
                    $( $type_name::$card_type => $card_type::place(board, player, position, direction)),+
                }
            }

            pub(crate) fn can_place(
                board: &Board,
                card: $type_name,
                player: Player,
                position: Position,
                direction: Direction,
            ) -> PlaceStatus {
                match card {
                    $( $type_name::$card_type => $card_type::can_place(board, player, position, direction)),+
                }
            }

        }

        $( impl $card_type {

            fn get_self_mut(board: &mut Board, self_idx: Index) -> &mut $card_type {
                match &mut board[self_idx].card {
                    $name::$card_type(a) => a,
                    _ => panic!("you've got the wrong card!")
                }
            }

            fn get_self(board: &Board, self_idx: Index) -> & $card_type {
                match &board[self_idx].card {
                    $name::$card_type(a) => a,
                    _ => panic!("you've got the wrong card!")
                }
            }

            fn as_type() -> $type_name {
                $type_name::$card_type
            }

        })+

    }
}

impl CardData {
    pub(crate) fn push(
        board: &mut Board,
        index: Index,
        direction: Direction,
        depth: usize,
    ) -> Set<Index> {
        board[index].card.pusher()(board, index, direction, depth + 1)
    }

    pub(crate) fn can_push(
        board: &Board,
        index: Index,
        direction: Direction,
        depth: usize,
    ) -> PushStatus {
        board[index].card.can_pusher()(board, index, direction, depth + 1)
    }
}

pub mod block;
pub mod double;
pub mod ghost;
pub mod normal;
pub mod redirect;
pub mod reverse;
pub mod score;

use block::Block;
use double::Double;
use ghost::Ghost;
use normal::Normal;
use redirect::Redirect;
use reverse::Reverse;
use score::Score;

// DO NOT PUT ANYTHING WITH INTERIOR MUTABILITY IN HERE
create_card_data!(CardData, CardType, Block, Normal, Double, Ghost, Score, Redirect, Reverse);
