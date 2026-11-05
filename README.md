# shield-statuses (Horizon port)

A Rust port of [Walksy's Shield Statuses](https://github.com/Walksy/ShieldStatus)
(MIT): it tints players' shields by whether the shield is disabled, so you can
see at a glance whether a shield is live or axe-disabled, and estimate when it
comes back.

## Behavior (1:1 with upstream)

- Enabled shields show the enabled color, disabled shields the disabled color.
- With interpolation on, the disabled color gradually transitions to the
  enabled color as the shield's cooldown runs out.
- The shown-players setting gates whose shields are tinted: self only,
  others only, or all. The keybind toggle cycles it.
- With the mod off, no tint anywhere.

## How it works in Horizon

The state logic (config, per-player shield-cooldown tracker, tint and
interpolation math, shown-players gating) lives in
mixins/shield_statuses/runtime.rs. The cooldown facts arrive through the
client packet hook (API 408, read-only clientbound play packets), which hears
the shield cooldown reports the server sends this client.

## Horizon seams

- Held-item/equipment render redirect: upstream replaces the shield special
  renderer with a tinted model submit (PlayerItemInHandLayer + shield
  renderer redirect). Horizon's client mod API has no held-item render hook
  (native_mob_submit covers mobs, native_render_item_in_frame covers item
  frames only), so the tint cannot reach the shield model yet. The logic is
  complete and ready to drive it the day the surface lands.
- Other players' shield states: upstream's other-player feature is
  server-assisted through its WalksyLib payload. Horizon has no equivalent
  surface; the tracker here keys on what the client itself hears.
- Filed on horizon-rust/horizon per standing policy; tracked until patched.

## License

Upstream Walksy's Shield Statuses: MIT (c) Walksy. This port carries the
same license; see LICENSE.
