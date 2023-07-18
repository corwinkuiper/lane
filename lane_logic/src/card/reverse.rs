use super::{
    normal::{normal_placement, normal_placement_rule, Normal},
    Card,
};

#[derive(Debug, Clone, Default)]
pub struct Reverse {}

impl Card for Reverse {
    fn push(
        board: &mut crate::Board,
        self_index: crate::Index,
        direction: crate::Direction,
        depth: usize,
    ) -> crate::Set<crate::Index> {
        Normal::push(board, self_index, -direction, depth)
    }

    fn can_push(
        board: &crate::Board,
        self_index: crate::Index,
        direction: crate::Direction,
        depth: usize,
    ) -> crate::PushStatus {
        Normal::can_push(board, self_index, -direction, depth)
    }

    fn can_place(
        board: &crate::Board,
        _player: crate::Player,
        position: crate::Position,
        direction: crate::Direction,
    ) -> crate::PlaceStatus {
        normal_placement_rule(board, position, direction)
    }

    fn place(
        board: &mut crate::Board,
        player: crate::Player,
        position: crate::Position,
        direction: crate::Direction,
    ) -> (crate::Index, crate::Set<crate::Index>) {
        normal_placement(
            board,
            player,
            position,
            direction,
            Self::as_type().to_data(),
        )
    }
}
