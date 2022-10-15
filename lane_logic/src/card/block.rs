use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

use super::{normal::Normal, Card};

#[derive(Debug, Clone, Default)]
pub struct Block {}

impl Card for Block {
    fn push(_board: &mut Board, _self_indexx: Index, _direction: Direction) -> Set<Index> {
        Set::new()
    }

    fn can_push(_board: &Board, _self_index: Index, _direction: Direction) -> PushStatus {
        PushStatus::Fail
    }

    fn can_place(
        board: &Board,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> PlaceStatus {
        Normal::can_place(board, player, position, direction)
    }

    fn place(
        board: &mut Board,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> Set<Index> {
        super::normal::normal_placement(
            board,
            player,
            position,
            direction,
            Self::as_type().to_data(),
        )
    }
}
