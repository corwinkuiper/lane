use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

use super::Card;

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
        board[self_index].position = new_position;

        Set::from_iter([self_index])
    }

    fn can_push(board: &Board, self_index: Index, direction: Direction) -> PushStatus {
        PushStatus::Success
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
