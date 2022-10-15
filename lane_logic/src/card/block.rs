use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

use super::Card;

#[derive(Debug, Clone, Default)]
pub struct Block {}

impl Card for Block {
    fn push(board: &mut Board, self_index: Index, direction: Direction) -> Set<Index> {
        Set::new()
    }

    fn can_push(board: &Board, self_index: Index, direction: Direction) -> PushStatus {
        PushStatus::Fail
    }

    fn can_place(
        board: &Board,
        card: Self,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> PlaceStatus {
        todo!()
    }

    fn place(
        board: &mut Board,
        card: Self,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> Set<Index> {
        todo!()
    }
}
