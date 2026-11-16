//! Use case: keep a player's picture of the world up to date.
//!
//! This is the client's "brain", and it has nothing to do with Bevy. Bevy
//! just feeds it network messages and frame times, and draws what it holds.
//! Because of that, the fading-echo logic below can be unit tested.

use std::collections::VecDeque;

use subhunt_domain::{Contact, ContactKind, PlayerId, SensorPicture, Vec2};

use crate::{ClientInbound, Notice, ServerMessage};

/// How long a ping echo stays on the scope.
const ECHO_LIFETIME: f32 = 6.0;
/// How long a notice stays on screen.
const NOTICE_LIFETIME: f32 = 8.0;
const MAX_NOTICES: usize = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConnectionStatus {
    AwaitingWelcome,
    Playing,
    Lost,
}

/// A remembered ping echo that fades out over time.
#[derive(Clone, Debug, PartialEq)]
pub struct Echo {
    pub position: Vec2,
    pub kind: ContactKind,
    pub age: f32,
}

impl Echo {
    /// 1.0 when fresh, falling to 0.0 when it's about to disappear.
    pub fn intensity(&self) -> f32 {
        (1.0 - self.age / ECHO_LIFETIME).clamp(0.0, 1.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TimedNotice {
    pub notice: Notice,
    pub age: f32,
}

#[derive(Debug)]
pub struct ClientSession {
    status: ConnectionStatus,
    player_id: Option<PlayerId>,
    world_half_extent: f32,
    ping_range: f32,
    picture: Option<SensorPicture>,
    echoes: Vec<Echo>,
    notices: VecDeque<TimedNotice>,
    banner: Option<String>,
    /// How many echoes have ever arrived, so a listener can spot new ones.
    echo_count: u64,
}

impl Default for ClientSession {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientSession {
    pub fn new() -> Self {
        Self {
            status: ConnectionStatus::AwaitingWelcome,
            player_id: None,
            world_half_extent: 0.0,
            ping_range: 1.0,
            picture: None,
            echoes: Vec::new(),
            notices: VecDeque::new(),
            banner: None,
            echo_count: 0,
        }
    }

    /// Feed in one thing the network delivered.
    pub fn handle(&mut self, inbound: ClientInbound) {
        match inbound {
            ClientInbound::Message(ServerMessage::Welcome { player_id, world_half_extent, ping_range }) => {
                self.player_id = Some(player_id);
                self.world_half_extent = world_half_extent;
                self.ping_range = ping_range;
                self.status = ConnectionStatus::Playing;
            }
            ClientInbound::Message(ServerMessage::Picture(picture)) => {
                // Ping echoes only arrive once, so remember them.
                for contact in &picture.contacts {
                    if let Contact::Fix { position, kind } = *contact {
                        self.echoes.push(Echo { position, kind, age: 0.0 });
                        self.echo_count += 1;
                    }
                }
                self.picture = Some(picture);
            }
            ClientInbound::Message(ServerMessage::Notice(notice)) => {
                self.notices.push_front(TimedNotice { notice, age: 0.0 });
                self.notices.truncate(MAX_NOTICES);
            }
            ClientInbound::ConnectionLost => self.status = ConnectionStatus::Lost,
            ClientInbound::Banner(text) => self.banner = Some(text),
        }
    }

    /// Advance client-side time (fading echoes and notices).
    pub fn advance(&mut self, dt: f32) {
        for echo in &mut self.echoes {
            echo.age += dt;
        }
        self.echoes.retain(|e| e.age < ECHO_LIFETIME);
        for notice in &mut self.notices {
            notice.age += dt;
        }
        self.notices.retain(|n| n.age < NOTICE_LIFETIME);
    }

    pub fn status(&self) -> ConnectionStatus {
        self.status
    }

    pub fn player_id(&self) -> Option<PlayerId> {
        self.player_id
    }

    /// A permanent line of text for the player, such as "Practice vs 3 bots".
    pub fn banner(&self) -> Option<&str> {
        self.banner.as_deref()
    }

    pub fn world_half_extent(&self) -> f32 {
        self.world_half_extent
    }

    /// How many metres the edge of the sonar scope represents.
    pub fn display_range(&self) -> f32 {
        self.ping_range.max(1.0)
    }

    /// Rust note: returning `Option<&SensorPicture>` lends the caller a
    /// read-only view without copying the picture.
    pub fn picture(&self) -> Option<&SensorPicture> {
        self.picture.as_ref()
    }

    /// Echoes still on the screen, oldest first.
    pub fn echoes(&self) -> &[Echo] {
        &self.echoes
    }

    /// The total number of echoes ever received. If it has gone up by 3 since
    /// you last looked, the last 3 entries of [`echoes`](Self::echoes) are new.
    pub fn echo_count(&self) -> u64 {
        self.echo_count
    }

    /// Newest first.
    pub fn notices(&self) -> impl Iterator<Item = &TimedNotice> {
        self.notices.iter()
    }
}

#[cfg(test)]
mod tests {
    use subhunt_domain::OwnShipStatus;

    use super::*;

    fn picture_with(contacts: Vec<Contact>) -> SensorPicture {
        SensorPicture {
            tick: 1,
            own_ship: OwnShipStatus {
                position: Vec2::ZERO,
                heading: 0.0,
                speed: 0.0,
                throttle: 0.0,
                noise: 0.2,
                torpedo_cooldown: 0.0,
                ping_cooldown: 0.0,
            },
            own_torpedoes: vec![],
            contacts,
            score: 0,
        }
    }

    #[test]
    fn welcome_starts_play() {
        let mut session = ClientSession::new();
        assert_eq!(session.status(), ConnectionStatus::AwaitingWelcome);
        session.handle(ClientInbound::Message(ServerMessage::Welcome {
            player_id: PlayerId(3),
            world_half_extent: 2000.0,
            ping_range: 1500.0,
        }));
        assert_eq!(session.status(), ConnectionStatus::Playing);
        assert_eq!(session.player_id(), Some(PlayerId(3)));
    }

    #[test]
    fn ping_echoes_are_remembered_then_fade() {
        let mut session = ClientSession::new();
        let fix = Contact::Fix { position: Vec2::new(10.0, 0.0), kind: ContactKind::Submarine };
        session.handle(ClientInbound::Message(ServerMessage::Picture(picture_with(vec![fix]))));
        // The next picture has no echo, but we still remember it.
        session.handle(ClientInbound::Message(ServerMessage::Picture(picture_with(vec![]))));
        assert_eq!(session.echoes().len(), 1);
        assert_eq!(session.echo_count(), 1);

        session.advance(ECHO_LIFETIME / 2.0);
        assert!((session.echoes()[0].intensity() - 0.5).abs() < 1e-5);

        session.advance(ECHO_LIFETIME);
        assert!(session.echoes().is_empty());
    }

    #[test]
    fn losing_the_connection_is_reported() {
        let mut session = ClientSession::new();
        session.handle(ClientInbound::ConnectionLost);
        assert_eq!(session.status(), ConnectionStatus::Lost);
    }
}
