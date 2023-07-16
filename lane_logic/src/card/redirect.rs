use crate::Set;

use super::{normal::Normal, Card, CardData};

#[derive(Debug, Clone, Default)]
pub struct Redirect {}

impl Card for Redirect {
    fn push(
        board: &mut crate::Board,
        self_index: crate::Index,
        direction: crate::Direction,
    ) -> crate::Set<crate::Index> {
        let my_position = board[self_index].position;
        let mut moved_cards = Set::new();

        let push_directions = [direction.anticlockwise(), direction.clockwise(), direction];

        for push_direction in push_directions.into_iter() {
            let push_position = my_position + push_direction;
            if let Some(next_index) = board.get_card_position(push_position) {
                let moved = CardData::push(board, next_index, push_direction);
                moved_cards = moved_cards.union(&moved).copied().collect();
            }
        }

        moved_cards.insert(self_index);
        board.move_card(board[self_index].position, my_position + direction);

        moved_cards
    }

    fn can_push(
        board: &crate::Board,
        self_index: crate::Index,
        direction: crate::Direction,
    ) -> crate::PushStatus {
        Normal::can_push(board, self_index, direction)
    }

    fn can_place(
        board: &crate::Board,
        player: crate::Player,
        position: crate::Position,
        direction: crate::Direction,
    ) -> crate::PlaceStatus {
        Normal::can_place(board, player, position, direction)
    }

    fn place(
        board: &mut crate::Board,
        player: crate::Player,
        position: crate::Position,
        direction: crate::Direction,
    ) -> (crate::Index, crate::Set<crate::Index>) {
        super::normal::normal_placement(
            board,
            player,
            position,
            direction,
            Self::as_type().to_data(),
        )
    }
}
