//! shield-statuses runtime (port of Walksy's Shield Statuses, MIT).
//!
//! The mod tints players' shields by whether the shield is disabled: an
//! enabled shield shows the enabled color, a disabled one the disabled color,
//! and with interpolation on the disabled color gradually transitions to the
//! enabled color as the shield's cooldown runs out, so you can estimate when
//! it reactivates. The `shown players` setting gates whose shields are
//! tinted. Upstream reads the tint from each rendered player's shield item
//! cooldown; on Horizon the cooldown facts arrive through the client packet
//! hook (API 408) and the tint would ride the held-item render surface, which
//! does not exist yet (README seams). This file carries the 1:1 state logic.

pub const MOD_ID: &str = "shield-statuses";

/// How many ticks a shield stays disabled after an axe disable (vanilla).
pub const SHIELD_DISABLE_TICKS: u32 = 100;

/// Whose shields the mod tints, upstream's shown-players setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShownPlayers {
    SelfOnly,
    OthersOnly,
    All,
}

/// One ARGB tint color.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StatusColor(pub u8, pub u8, pub u8);

impl StatusColor {
    fn argb(self, alpha: u8) -> u32 {
        ((alpha as u32) << 24) | ((self.0 as u32) << 16) | ((self.1 as u32) << 8) | self.2 as u32
    }
    fn lerp(self, other: StatusColor, t: f32) -> StatusColor {
        let t = t.clamp(0.0, 1.0);
        StatusColor(
            (self.0 as f32 + (other.0 as f32 - self.0 as f32) * t).round() as u8,
            (self.1 as f32 + (other.1 as f32 - self.1 as f32) * t).round() as u8,
            (self.2 as f32 + (other.2 as f32 - self.2 as f32) * t).round() as u8,
        )
    }
}

#[derive(Clone, Debug)]
pub struct ShieldStatusConfig {
    pub mod_enabled: bool,
    pub shown_players: ShownPlayers,
    pub enabled_color: StatusColor,
    pub disabled_color: StatusColor,
    /// Transition from the disabled color to the enabled one as the cooldown
    /// runs out, instead of two static colors.
    pub interpolation: bool,
}

impl Default for ShieldStatusConfig {
    fn default() -> Self {
        Self {
            mod_enabled: true,
            shown_players: ShownPlayers::All,
            // Upstream defaults: green for enabled, red for disabled.
            enabled_color: StatusColor(0x00, 0xFF, 0x00),
            disabled_color: StatusColor(0xFF, 0x00, 0x00),
            interpolation: true,
        }
    }
}

/// What the mod knows about one player's shield.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ShieldState {
    /// Shield usable; tinted the enabled color.
    Enabled,
    /// Shield disabled; `remaining` ticks and `duration` total of cooldown
    /// left, for the interpolation estimate.
    Disabled { remaining: u32, duration: u32 },
}

/// Per-player shield cooldown facts, fed by the packet wiring.
#[derive(Default, Debug)]
pub struct CooldownTracker {
    /// player entity id -> cooldown end tick (absolute client ticks).
    ends: std::collections::HashMap<u64, u64>,
    now: u64,
}

impl CooldownTracker {
    /// The client reported a shield cooldown of `duration` ticks starting now
    /// for `player`.
    pub fn shield_disabled(&mut self, player: u64, duration: u32) {
        if duration == 0 {
            self.ends.remove(&player);
            return;
        }
        self.ends.insert(player, self.now + duration as u64);
    }

    pub fn tick(&mut self) {
        self.now += 1;
    }

    /// This player's shield state right now.
    pub fn state_of(&self, player: u64) -> ShieldState {
        match self.ends.get(&player) {
            None => ShieldState::Enabled,
            Some(&end) => {
                let remaining = end.saturating_sub(self.now) as u32;
                if remaining == 0 {
                    ShieldState::Enabled
                } else {
                    ShieldState::Disabled {
                        remaining,
                        duration: SHIELD_DISABLE_TICKS,
                    }
                }
            }
        }
    }

    pub fn forget(&mut self, player: u64) {
        self.ends.remove(&player);
    }
}

/// Should this player be tinted at all, per the shown-players setting?
pub fn shown(kind: PlayerKind, cfg: &ShieldStatusConfig) -> bool {
    if !cfg.mod_enabled {
        return false;
    }
    match (kind, cfg.shown_players) {
        (PlayerKind::SelfPlayer, ShownPlayers::OthersOnly) => false,
        (PlayerKind::Other, ShownPlayers::SelfOnly) => false,
        _ => true,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerKind {
    SelfPlayer,
    Other,
}

/// The tint for a player's shield right now, as the ARGB the renderer passes
/// to the shield model submit. Untinted shields answer None (keep vanilla).
pub fn tint_for(
    state: ShieldState,
    cfg: &ShieldStatusConfig,
) -> Option<u32> {
    if !cfg.mod_enabled {
        return None;
    }
    match state {
        ShieldState::Enabled => Some(cfg.enabled_color.argb(0xFF)),
        ShieldState::Disabled { remaining, duration } => {
            if cfg.interpolation && duration > 0 {
                // The disabled color eases toward the enabled one as the
                // cooldown runs out: t is the recovered fraction.
                let recovered = 1.0 - remaining as f32 / duration as f32;
                Some(
                    cfg.disabled_color
                        .lerp(cfg.enabled_color, recovered)
                        .argb(0xFF),
                )
            } else {
                Some(cfg.disabled_color.argb(0xFF))
            }
        }
    }
}

/// One rendered player's answer: gate on shown-players, then tint.
pub fn shield_tint(
    kind: PlayerKind,
    state: ShieldState,
    cfg: &ShieldStatusConfig,
) -> Option<u32> {
    if !shown(kind, cfg) {
        return None;
    }
    tint_for(state, cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> ShieldStatusConfig {
        ShieldStatusConfig::default()
    }

    #[test]
    fn enabled_shields_show_the_enabled_color() {
        assert_eq!(
            shield_tint(PlayerKind::Other, ShieldState::Enabled, &cfg()),
            Some(0xFF00FF00)
        );
    }

    #[test]
    fn static_disabled_color_when_interpolation_is_off() {
        let mut c = cfg();
        c.interpolation = false;
        let s = ShieldState::Disabled { remaining: 50, duration: 100 };
        assert_eq!(shield_tint(PlayerKind::Other, s, &c), Some(0xFFFF0000));
    }

    #[test]
    fn interpolation_eases_from_disabled_to_enabled() {
        // Halfway through the disable, the tint sits between red and green.
        let s = ShieldState::Disabled { remaining: 50, duration: 100 };
        let tint = shield_tint(PlayerKind::Other, s, &cfg()).unwrap();
        let r = (tint >> 16) & 0xFF;
        let g = (tint >> 8) & 0xFF;
        assert_eq!(r, 0x80, "halfway red channel");
        assert_eq!(g, 0x80, "halfway green channel");
        // Near the end it is almost the enabled color.
        let s = ShieldState::Disabled { remaining: 5, duration: 100 };
        let tint = shield_tint(PlayerKind::Other, s, &cfg()).unwrap();
        assert!(((tint >> 8) & 0xFF) > 0xF0, "green near full");
    }

    #[test]
    fn shown_players_gates_self_and_others() {
        let mut c = cfg();
        c.shown_players = ShownPlayers::SelfOnly;
        assert_eq!(shield_tint(PlayerKind::Other, ShieldState::Enabled, &c), None);
        assert!(shield_tint(PlayerKind::SelfPlayer, ShieldState::Enabled, &c).is_some());
        c.shown_players = ShownPlayers::OthersOnly;
        assert_eq!(shield_tint(PlayerKind::SelfPlayer, ShieldState::Enabled, &c), None);
        assert!(shield_tint(PlayerKind::Other, ShieldState::Enabled, &c).is_some());
    }

    #[test]
    fn mod_off_means_no_tint_anywhere() {
        let mut c = cfg();
        c.mod_enabled = false;
        assert_eq!(shield_tint(PlayerKind::SelfPlayer, ShieldState::Enabled, &c), None);
        assert_eq!(shield_tint(PlayerKind::Other, ShieldState::Disabled { remaining: 9, duration: 100 }, &c), None);
    }

    #[test]
    fn cooldown_tracker_transitions() {
        let mut t = CooldownTracker::default();
        t.tick();
        t.shield_disabled(7, 100);
        assert_eq!(
            t.state_of(7),
            ShieldState::Disabled { remaining: 100, duration: 100 }
        );
        for _ in 0..100 {
            t.tick();
        }
        assert_eq!(t.state_of(7), ShieldState::Enabled);
        // A zero-duration report clears the disable.
        t.shield_disabled(7, 100);
        t.shield_disabled(7, 0);
        assert_eq!(t.state_of(7), ShieldState::Enabled);
    }

    #[test]
    fn leaving_players_are_forgotten() {
        let mut t = CooldownTracker::default();
        t.shield_disabled(3, 100);
        t.forget(3);
        assert_eq!(t.state_of(3), ShieldState::Enabled);
    }
}
