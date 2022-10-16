use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

use super::{normal::Normal, Card};

#[derive(Debug, Clone, Default)]
pub struct Double {}

impl Card for Double {
    fn push(board: &mut Board, self_index: Index, direction: Direction) -> Set<Index> {
        let pushed = Normal::push(board, self_index, direction);
        if pushed.is_empty() {
            return pushed;
        }

        Normal::push(board, self_index, direction)
            .union(&pushed)
            .cloned()
            .collect()
    }

    fn can_push(board: &Board, self_index: Index, direction: Direction) -> PushStatus {
        Normal::can_push(board, self_index, direction)
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
    ) -> (Index, Set<Index>) {
        super::normal::normal_placement(
            board,
            player,
            position,
            direction,
            Self::as_type().to_data(),
        )
    }
}
