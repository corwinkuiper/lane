use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

use super::{normal::Normal, Card};

#[derive(Debug, Clone, Default)]
pub struct Score {}

impl Card for Score {
    fn push(
        board: &mut Board,
        self_index: Index,
        direction: Direction,
        depth: usize,
    ) -> Set<Index> {
        Normal::push(board, self_index, direction, depth)
    }

    fn can_push(
        board: &Board,
        self_index: Index,
        direction: Direction,
        depth: usize,
    ) -> PushStatus {
        Normal::can_push(board, self_index, direction, depth)
    }

    fn can_place(
        _board: &Board,
        _player: Player,
        _position: Position,
        _direction: Direction,
    ) -> PlaceStatus {
        panic!("Woah! You can't try to place the score card. The score card is placed by the game not by you.")
    }

    fn place(
        _board: &mut Board,
        _player: Player,
        _position: Position,
        _direction: Direction,
    ) -> (Index, Set<Index>) {
        panic!("Woah! You can't try to place the score card. The score card is placed by the game not by you.")
    }
}
