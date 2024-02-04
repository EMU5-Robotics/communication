use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
pub enum Action {
    StartAt { pos: [f64; 2], heading: f64 },
    MoveRel { rel: f64 },
    MoveRelAbs { rel: f64 },
    MoveTo { pos: [f64; 2] },
    TurnRel { angle: f64 },
    TurnRelAbs { angle: f64 },
    TurnTo { heading: f64 },
}
