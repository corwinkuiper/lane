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
            match CardData::can_push(board, next_index, direction) {
                PushStatus::Success(n) => PushStatus::Success(n + 1),
                PushStatus::Fail => PushStatus::Fail,
            }
        } else {
            PushStatus::Fail
        }
    }

    fn can_place(
        board: &Board,
        _player: Player,
        position: Position,
        direction: Direction,
    ) -> PlaceStatus {
        normal_placement_rule(board, position, direction)
    }

    fn place(
        board: &mut Board,
        player: Player,
        position: Position,
        direction: Direction,
    ) -> Set<Index> {
        normal_placement(
            board,
            player,
            position,
            direction,
            Self::as_type().to_data(),
        )
    }
}

pub(crate) fn normal_placement(
    board: &mut Board,
    player: Player,
    position: Position,
    direction: Direction,
    card_data: CardData,
) -> Set<Index> {
    let idx = board.add_card(player, position, card_data);
    CardData::push(board, idx, direction)
}

pub(crate) fn normal_placement_rule(
    board: &Board,
    position: Position,
    direction: Direction,
) -> PlaceStatus {
    if board.get_card_position(position + direction).is_some() {
        PlaceStatus::Success
    } else {
        PlaceStatus::Fail
    }
}
