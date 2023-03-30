use alloc::{boxed::Box, vec::Vec};
use lane_logic::{Move, MoveResult, Player, State};

use crate::async_evaluator::{self, Evaluator};
use async_recursion::async_recursion;

#[derive(Debug, Clone, Copy)]
pub enum AIControl {
    Best,
    WithRandom(i32),
    Negative,
}

impl ScoreCalculator for AIControl {
    fn score(&self, result: &MoveResult, node: &State, player: Player) -> i32 {
        match self {
            AIControl::Best => calculate_state_score(result, node, player),
            AIControl::WithRandom(random_parameter) => {
                calculate_state_score(result, node, player) + agb::rng::gen() % random_parameter
                    - random_parameter / 2
            }
            AIControl::Negative => -calculate_state_score(result, node, player),
        }
    }
}

trait ScoreCalculator: Sync {
    fn score(&self, result: &MoveResult, node: &State, player: Player) -> i32;
}

struct BestCalculator;

impl ScoreCalculator for BestCalculator {
    fn score(&self, result: &MoveResult, node: &State, player: Player) -> i32 {
        calculate_state_score(result, node, player)
    }
}

struct WithRandomCalculator {
    random_parameter: i32,
}
impl ScoreCalculator for WithRandomCalculator {
    fn score(&self, result: &MoveResult, node: &State, player: Player) -> i32 {
        calculate_state_score(result, node, player) + agb::rng::gen() % self.random_parameter
            - self.random_parameter / 2
    }
}

struct WorstCalculator;

impl ScoreCalculator for WorstCalculator {
    fn score(&self, result: &MoveResult, node: &State, player: Player) -> i32 {
        -calculate_state_score(result, node, player)
    }
}

impl AIControl {
    pub fn move_finder(&self, state: State) -> Evaluator<Option<Move>> {
        agb::println!("Creating move finder");

        Evaluator::new(find_best_move(state, *self, 1))
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

    score += result.score.player(current_turn) as i32 * 10;
    score -= result.score.player(alternate_turn) as i32 * 20;

    score += node.player_hand(current_turn).len() as i32;

    score
}

async fn find_best_move(
    game_state: State,
    score_function: impl ScoreCalculator,
    yeild: usize,
) -> Option<Move> {
    let possible_moves = game_state
        .enumerate_possible_moves_async(yeild, async_evaluator::yeild)
        .await;

    agb::println!("Starting eval of {} positions", possible_moves.len());

    async_evaluator::yeild().await;

    let player = game_state.turn();

    let mut scored_moves = Vec::new();

    let mut alpha = i32::MIN;
    let beta = i32::MAX;

    let mut best_score = i32::MIN;

    for move_to_check in possible_moves {
        let mut next_state = game_state.clone();
        let result = next_state.execute_move(&move_to_check);
        let resultant_score = score_function.score(&result, &next_state, player);
        let score = minimax(&score_function, next_state, &result, 1, player, alpha, beta).await;

        best_score = best_score.max(score);
        alpha = best_score.max(alpha);
        scored_moves.push((move_to_check, score, resultant_score));
    }

    if best_score < -100000 {
        agb::println!("From this position, loss is inevitable");
    }

    if best_score > 100000 {
        agb::println!("From this position, win is inevitable");
    }

    async_evaluator::yeild().await;

    scored_moves.retain(|(_, s, _)| *s == best_score);

    async_evaluator::yeild().await;

    let ran = agb::rng::gen() as usize;

    let (desired_move, _, resultant_score) = scored_moves.swap_remove(ran % scored_moves.len());

    agb::println!(
        "Playing a move that is rated in the long term {} and currently {}",
        best_score,
        resultant_score
    );

    Some(desired_move)
}

const MAX_DEPTH: u32 = 2;

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
    if depth >= MAX_DEPTH || move_result_to_get_here.winner.is_some() {
        return score_function.score(move_result_to_get_here, &node, me);
    }

    let possible_moves = node
        .enumerate_possible_moves_async(1, async_evaluator::yeild)
        .await;

    // agb::println!(
    //     "Eval of {} positions at a depth of {}. Optimising for player {:?} on player {:?}'s turn",
    //     possible_moves.len(),
    //     depth,
    //     me,
    //     node.turn()
    // );

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
                depth + 1,
                me,
                alpha,
                beta,
            )
            .await;
            best_evaluation = best_evaluation.max(value_of_move);
            alpha = alpha.max(best_evaluation);
            if beta <= alpha {
                break;
            }
        }
        agb::println!(
            "Searched my turn turn: d = {}, max = {}",
            depth,
            best_evaluation
        );

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
                depth + 1,
                me,
                alpha,
                beta,
            )
            .await;

            worst_evaluation = worst_evaluation.min(value_of_move);
            beta = beta.min(worst_evaluation);
            if beta <= alpha {
                break;
            }
        }
        agb::println!(
            "Searched opponents turn: d= {}, min = {}",
            depth,
            worst_evaluation
        );
        worst_evaluation
    }
}
