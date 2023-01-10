use bevy::prelude::*;

use crate::{
    level_pluggin::{spawn_food, LevelInstance, Walkable},
    movement_pluggin::GravityFall,
    snake_pluggin::{DespawnSnakePartEvent, Snake, SnakePart},
};

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum WalkableUpdateEvent {
    ClearPosition(IVec2, Walkable),
    FillPosition(IVec2),
}

/// History event marking that a snake stops falling, with distance fallen.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct EndFall {
    pub distance_fallen: i32,
    pub walkable_updates: Vec<WalkableUpdateEvent>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
pub enum MoveHistoryEvent {
    /// A history event that marks a player move action.
    PlayerSnakeMove,

    /// History event for the snake moving one tile in a direction, storing the old tails for undo.
    SnakeMoveForward((IVec2, IVec2)),

    /// History event for moving a snake with an offset fex: pushing.
    PassiveSnakeMove(IVec2),

    /// History event marking that a snake starts falling.
    BeginFall(Option<EndFall>),

    Grow,
    Eat(IVec2),
}

#[derive(Clone)]
pub struct SnakeHistoryEvent {
    pub event: MoveHistoryEvent,
    pub snake_index: i32,
    walkable_updates: Vec<WalkableUpdateEvent>,
}

pub struct UndoEvent;

/// A struct storing history events that can be undone.
#[derive(Resource, Default)]
pub struct SnakeHistory {
    pub move_history: Vec<SnakeHistoryEvent>,
}

impl SnakeHistory {
    pub fn push(&mut self, event: MoveHistoryEvent, snake_index: i32) {
        self.move_history.push(SnakeHistoryEvent {
            event,
            snake_index,
            walkable_updates: vec![],
        });
    }

    pub fn push_with_updates(
        &mut self,
        event: MoveHistoryEvent,
        snake_index: i32,
        walkable_updates: Vec<WalkableUpdateEvent>,
    ) {
        self.move_history.push(SnakeHistoryEvent {
            event,
            snake_index,
            walkable_updates,
        });
    }

    pub fn undo_last(
        &mut self,
        snakes: &mut [Mut<Snake>],
        level: &mut LevelInstance,
        commands: &mut Commands,
        despawn_snake_part_event: &mut EventWriter<DespawnSnakePartEvent>,
    ) {
        // Undo the stack until we reach the last player action.
        while let Some(top) = self.move_history.pop() {
            if MoveHistoryEvent::PlayerSnakeMove == top.event {
                return;
            }

            let snake: &mut Snake = snakes
                .iter_mut()
                .find(|snake| snake.index() == top.snake_index)
                .expect("Missing snake in query")
                .as_mut();

            match top.event {
                MoveHistoryEvent::PlayerSnakeMove => {
                    unreachable!("Should be handled as early return above.")
                }
                MoveHistoryEvent::SnakeMoveForward(old_tail) => {
                    snake.move_back(&old_tail);
                }
                MoveHistoryEvent::PassiveSnakeMove(offset) => {
                    snake.translate(-offset);
                }
                MoveHistoryEvent::BeginFall(None) => {
                    // Nothing to do.
                }
                MoveHistoryEvent::BeginFall(Some(end_fall)) => {
                    snake.move_up(end_fall.distance_fallen);
                    level.undo_updates(&end_fall.walkable_updates);
                }
                MoveHistoryEvent::Grow => {
                    despawn_snake_part_event.send(DespawnSnakePartEvent(SnakePart {
                        snake_index: snake.index(),
                        part_index: snake.len() - 1,
                    }));

                    snake.shrink();
                }
                MoveHistoryEvent::Eat(position) => {
                    spawn_food(commands, &position, level);
                }
            }

            level.undo_updates(&top.walkable_updates);
        }
    }
}

pub fn keyboard_undo_system(
    keyboard: Res<Input<KeyCode>>,
    mut trigger_undo_event: EventWriter<UndoEvent>,
    falling_snakes: Query<(With<Snake>, With<GravityFall>)>,
) {
    if !keyboard.just_pressed(KeyCode::Back) {
        return;
    }

    if !falling_snakes.is_empty() {
        return;
    }

    trigger_undo_event.send(UndoEvent);
}

pub fn undo_event_system(
    mut trigger_undo_event: EventReader<UndoEvent>,
    mut snake_history: ResMut<SnakeHistory>,
    mut level: ResMut<LevelInstance>,
    mut despawn_snake_part_event: EventWriter<DespawnSnakePartEvent>,
    mut commands: Commands,
    mut query: Query<&mut Snake>,
) {
    if trigger_undo_event.iter().next().is_none() {
        return;
    }

    if snake_history.move_history.is_empty() {
        return;
    }

    let mut snakes: Vec<Mut<Snake>> = query.iter_mut().collect();

    snake_history.undo_last(
        &mut snakes,
        &mut level,
        &mut commands,
        &mut despawn_snake_part_event,
    );
}
