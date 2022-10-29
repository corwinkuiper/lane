use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

use super::{normal::Normal, Card, CardData};

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
        match Normal::can_push(board, self_index, direction) {
            PushStatus::Success(n @ 1..) => PushStatus::Success(n),
            PushStatus::Success(0) | PushStatus::Fail => {
                // check if our double push can push

                let my_position = board[self_index].position;
                let next_position = my_position + direction + direction;

                // find index of next item
                if let Some(next_index) = board.get_card_position(next_position) {
                    match CardData::can_push(board, next_index, direction) {
                        PushStatus::Success(n) => PushStatus::Success(n + 1),
                        PushStatus::Fail => PushStatus::Success(0),
                    }
                } else {
                    PushStatus::Success(0)
                }
            }
        }
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
