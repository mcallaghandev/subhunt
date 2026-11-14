use crate::{GameRules, PlayerId, TorpedoId, Vec2};

/// A torpedo running in a straight line until it hits something or runs out of fuel.
#[derive(Clone, Debug, PartialEq)]
pub struct Torpedo {
    pub id: TorpedoId,
    pub owner: PlayerId,
    pub position: Vec2,
    pub heading: f32,
    pub speed: f32,
    /// Seconds of fuel left.
    pub remaining: f32,
}

impl Torpedo {
    pub fn new(id: TorpedoId, owner: PlayerId, position: Vec2, heading: f32, rules: &GameRules) -> Self {
        Self { id, owner, position, heading, speed: rules.torpedo_speed, remaining: rules.torpedo_lifetime }
    }

    pub fn integrate(&mut self, dt: f32) {
        self.position += Vec2::from_angle(self.heading) * (self.speed * dt);
        self.remaining -= dt;
    }

    /// Out of fuel or out of the play area.
    pub fn is_spent(&self, rules: &GameRules) -> bool {
        let limit = rules.world_half_extent;
        self.remaining <= 0.0 || self.position.x.abs() > limit || self.position.y.abs() > limit
    }
}
