use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

use super::{normal::Normal, Card};

#[derive(Debug, Clone, Default)]
pub struct Ghost {}

impl Card for Ghost {
    fn push(board: &mut Board, self_index: Index, direction: Direction) -> Set<Index> {
        let mut current_index = self_index;
        let new_position = loop {
            let my_position = board[current_index].position;
            let next_position = my_position + direction;
            match board.get_card_position(next_position) {
                Some(idx) => current_index = idx,
                None => break next_position,
            }
        };

        board.move_card(self_index, new_position);

        Set::from_iter([self_index])
    }

    fn can_push(board: &Board, self_index: Index, direction: Direction) -> PushStatus {
        match Normal::can_push(board, self_index, direction) {
            PushStatus::Success(0) => PushStatus::Success(0),
            PushStatus::Success(_) => PushStatus::Success(1),
            PushStatus::Fail => PushStatus::Success(0),
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
