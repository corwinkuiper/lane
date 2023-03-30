use alloc::{boxed::Box, vec::Vec};
use lane_logic::{Move, MoveResult, Player, State};

use crate::async_evaluator::{self, Evaluator};

#[derive(Debug, Clone, Copy)]
pub enum AIControl {
    Best,
    WithRandom(i32),
    Negative,
}

impl AIControl {
    pub fn move_finder(&self, state: State) -> Evaluator<Option<Move>> {
        agb::println!("Creating move finder");

        match *self {
            AIControl::Best => {
                Evaluator::new(find_best_move(state, Box::new(calculate_state_score), 1))
            }
            AIControl::WithRandom(v) => Evaluator::new(find_best_move(
                state,
                Box::new(move |result, player| {
                    calculate_state_score(result, player) + agb::rng::gen() % v - v / 2
                }),
                1,
            )),
            AIControl::Negative => Evaluator::new(find_best_move(
                state,
                Box::new(|result, player| -calculate_state_score(result, player)),
                1,
            )),
        }
    }
}

pub enum ControlMode {
    TwoHuman,
    AI(AIControl, Player),
    TwoAI(AIControl, AIControl),
}

fn calculate_state_score(result: &MoveResult, current_turn: Player) -> i32 {
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

type ScoreCalculation = dyn Fn(&MoveResult, Player) -> i32;

async fn find_best_move(
    game_state: State,
    score_function: Box<ScoreCalculation>,
    yeild: usize,
) -> Option<Move> {
    let mut counter = 0;
    let mut should_yeild = || {
        counter += 1;
        if counter > yeild {
            counter = 0;
            true
        } else {
            false
        }
    };

    let possible_moves = game_state
        .enumerate_possible_moves_async(yeild, async_evaluator::yeild)
        .await;

    async_evaluator::yeild().await;

    let player = game_state.turn();

    let mut scored_moves = Vec::new();

    for move_to_check in possible_moves {
        let result = game_state.clone().execute_move(&move_to_check);
        let socre = score_function(&result, player);

        if should_yeild() {
            async_evaluator::yeild().await;
        }

        scored_moves.push((move_to_check, socre));
    }

    async_evaluator::yeild().await;

    let max_score = scored_moves.iter().max_by_key(|x| x.1)?.1;

    async_evaluator::yeild().await;

    scored_moves.retain(|(_, s)| *s == max_score);

    async_evaluator::yeild().await;

    let ran = agb::rng::gen() as usize;

    let (desired_move, _) = scored_moves.swap_remove(ran % scored_moves.len());

    Some(desired_move)
}
