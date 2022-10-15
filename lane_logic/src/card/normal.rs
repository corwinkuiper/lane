use alloc::vec::Vec;

use crate::{Board, Direction, Index, PlaceStatus, Player, Position, PushStatus, Set};

use super::{Card, CardData};

#[derive(Debug, Clone, Default)]
pub struct Normal {}

impl Card for Normal {
    fn push(board: &mut Board, self_index: Index, direction: Direction) -> Set<Index> {
        let my_position = board[self_index].position;
        let next_position = my_position + direction;

        // find index of next item
        let mut moved = if let Some(next_index) = board.get_card_position(next_position) {
            let moved = CardData::push(board, next_index, direction);

            if moved.is_empty() {
                return moved;
            }

            moved
        } else {
            Set::new()
        };

        moved.insert(self_index);
        board[self_index].position = next_position;

        moved
    }

    fn can_push(board: &Board, self_index: Index, direction: Direction) -> PushStatus {
        let my_position = board[self_index].position;
        let next_position = my_position + direction;

        // find index of next item
        if let Some(next_index) = board.get_card_position(next_position) {
            if CardData::can_push(board, next_index, direction) == PushStatus::Fail {
                return PushStatus::Fail;
            }
        }

        // yay, can push!
        PushStatus::Success
    }

    fn can_place(
        board: &Board,
        card: Self,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> PlaceStatus {
        PlaceStatus::Success
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
