use alloc::{boxed::Box, vec::Vec};
use lane_logic::{Move, MoveResult, Player, State};

use crate::async_evaluator::{self, Evaluator};
use async_recursion::async_recursion;

#[derive(Debug, Clone, Copy)]
pub struct AIControl {
    pub depth: u32,
    pub ai_type: AiControlType,
}

#[derive(Debug, Clone, Copy)]
pub enum AiControlType {
    Best,
    WithRandom(i32),
    Negative,
}

impl ScoreCalculator for AiControlType {
    fn score(&self, result: &MoveResult, node: &State, player: Player) -> i32 {
        match self {
            AiControlType::Best => calculate_state_score(result, node, player),
            AiControlType::WithRandom(random_parameter) => {
                calculate_state_score(result, node, player) + agb::rng::gen() % random_parameter
            }
            AiControlType::Negative => -calculate_state_score(result, node, player),
        }
    }
}

trait ScoreCalculator: Sync {
    fn score(&self, result: &MoveResult, node: &State, player: Player) -> i32;
}

impl AIControl {
    pub fn move_finder(&self, state: State) -> Evaluator<Option<Move>> {
        Evaluator::new(find_best_move(state, self.ai_type, self.depth, 1))
    }
}

pub enum ControlMode {
    TwoHuman,
    AI(AIControl, Player),
    TwoAI(AIControl, AIControl),
}

fn calculate_state_score(result: &MoveResult, node: &State, current_turn: Player) -> i32 {
    let mut score: i32 = 0;

    let alternate_turn = match current_turn {
        Player::A => Player::B,
        Player::B => Player::A,
    };

    if result.winner == Some(current_turn) {
        score += 100000000;
    }

    if result.winner == Some(alternate_turn) {
        score -= 100000000;
    }

    let my_score = result.score.player(current_turn) as i32;
    score += my_score.pow(2);

    let opponent_score = result.score.player(alternate_turn) as i32;
    score -= opponent_score.pow(3);

    score
}

fn randomise_list<T>(items: &mut Vec<T>) {
    // Randomise the move list
    for i in (1..items.len()).rev() {
        let j = agb::rng::gen() as usize % i;
        items.swap(i, j);
    }
}

async fn find_best_move(
    game_state: State,
    score_function: impl ScoreCalculator,
    max_depth: u32,
    yeild: usize,
) -> Option<Move> {
    let mut possible_moves = game_state
        .enumerate_possible_moves_async(yeild, async_evaluator::yeild)
        .await;

    async_evaluator::yeild().await;

    randomise_list(&mut possible_moves);

    async_evaluator::yeild().await;

    let player = game_state.turn();

    let mut alpha = i32::MIN;
    let beta = i32::MAX;

    let mut best_score = i32::MIN;

    let mut best_move = None;

    for move_to_check in possible_moves {
        let mut next_state = game_state.clone();
        let result = next_state.execute_move(&move_to_check);
        let resultant_score = score_function.score(&result, &next_state, player);
        async_evaluator::yeild().await;
        let score = minimax(
            &score_function,
            next_state,
            &result,
            max_depth - 1,
            player,
            alpha,
            beta,
        )
        .await;

        if score > best_score {
            best_move = Some((move_to_check, score, resultant_score));
        }

        best_score = best_score.max(score);
        alpha = best_score.max(alpha);
    }

    async_evaluator::yeild().await;

    let (desired_move, _, _) = best_move.unwrap();

    Some(desired_move)
}

#[async_recursion]
async fn minimax(
    score_function: &impl ScoreCalculator,
    node: State,
    move_result_to_get_here: &MoveResult,
    depth: u32,
    me: Player,
    mut alpha: i32,
    mut beta: i32,
) -> i32 {
    if depth == 0 || move_result_to_get_here.winner.is_some() {
        return score_function.score(move_result_to_get_here, &node, me);
    }

    let mut possible_moves = node
        .enumerate_possible_moves_async(1, async_evaluator::yeild)
        .await;

    async_evaluator::yeild().await;

    randomise_list(&mut possible_moves);

    async_evaluator::yeild().await;

    if node.turn() == me {
        let mut best_evaluation = i32::MIN;
        for next_move in possible_moves {
            let mut next_node = node.clone();
            let next_move_result = next_node.execute_move(&next_move);
            async_evaluator::yeild().await;
            let value_of_move = minimax(
                score_function,
                next_node,
                &next_move_result,
                depth - 1,
                me,
                alpha,
                beta,
            )
            .await;
            best_evaluation = best_evaluation.max(value_of_move);
            alpha = alpha.max(best_evaluation);
            if beta <= best_evaluation {
                break;
            }
        }
        best_evaluation
    } else {
        let mut worst_evaluation = i32::MAX;
        for next_move in possible_moves {
            let mut next_node = node.clone();
            let next_move_result = next_node.execute_move(&next_move);
            async_evaluator::yeild().await;
            let value_of_move = minimax(
                score_function,
                next_node,
                &next_move_result,
                depth - 1,
                me,
                alpha,
                beta,
            )
            .await;

            worst_evaluation = worst_evaluation.min(value_of_move);
            beta = beta.min(worst_evaluation);
            if worst_evaluation <= alpha {
                break;
            }
        }
        worst_evaluation
    }
}
