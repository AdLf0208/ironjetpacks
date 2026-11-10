#!/usr/bin/env bash
# In-game check for Iron Jetpacks slice 2 (iron tier, item-backed tank).
#
# A high Overworld arena: stone floor at y=99 (feet 100), no nearby mobs.
# Survival, cheats on so the mod's level-2 command reaches the player. The
# same held jump key is used in every phase:
#   * no jetpack (disabled): a vanilla jump peaks about 1.25 blocks above
#     the floor and must not exceed 1.6;
#   * an equipped jetpack with an empty tank and the engine off (both are the
#     fresh item's defaults) must stay within a vanilla jump: nothing to burn;
#   * `/ironjetpacks fuel 800000`, sent as a chat line, charges the item's tank
#     and its query answers `800000 / 800000 FE` (the client chat tree only
#     accepts it because the `commands` hook lists the mod's command);
#   * `ctl inventory` shows that 800000 FE in the chest stack's own
#     `minecraft:custom_data` (`horizon_energy`), i.e. the tank is on the item,
#     not in a server-side map or a block entity;
#   * with a full tank and the engine still off, the jetpack must not fly: the
#     engine toggle is what arms it;
#   * `/ironjetpacks engine` flips the engine component on the item, and the
#     same held jump must then lift the player more than 2 blocks;
#   * the fuel query after that flight answers a drained tank, and the chest
#     stack's own `horizon_energy` matches that reading exactly, so the flight
#     spent the item's energy;
#   * the input edge: the rise and the drain stop when the jump key is
#     released (`ctl key space up`), so `player_input`/the sampled key gates
#     the thrust instead of a free-running tick;
#   * an empty item tank with the engine on must not thrust: the last control,
#     which isolates the fuel from the engine.
# `NO_JETPACK=1` removes the chest item before the flight phase: the same
# assertion must then fail, proving the check can fail.
#
# Default run: the game loads `content/` as a dev mod.
# EMBEDDED=1: the game is launched with **only the native bundle** (no
# `--dev-mod`) and the content comes from the bundle's embedded copy
# (`mixins/<id>/data/…`, the AE2 pattern). The run also checks the file the
# helper wrote, and CONTROL=1 (default with EMBEDDED) first launches the same
# binary with `.horizon-cache` unusable, where the item id must be unknown.
set -euo pipefail

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/ironjetpacks
CTL=${CTL:-$PORT/target/horizon-cargo/fast/horizon-ctl}
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/ironjetpacks-e2e.sock}
EMBEDDED=${EMBEDDED:-0}
CONTROL=${CONTROL:-$EMBEDDED}
WORLD=IronJetpacksE2E
CONTROL_WORLD=IronJetpacksControl

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }
[ -f "$PORT/content/horizon-mod.toml" ] || { echo "missing content mod: $PORT/content" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/ironjetpacks-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
feet_y() { ctl state | jq -r '.player.position[1]'; }
vel_y() { ctl state | jq -r '.player.velocity[1]'; }
health() { ctl state | jq -r '.player.health'; }
# `.chat` is newest first, so `head -1` after a grep is the latest match.
messages() { ctl messages 40 | jq -r '.chat[]'; }
fail() { echo "FAIL: $*" >&2; exit 1; }
# rise ABOVE BELOW: print the difference, refusing a non-number.
rise() { awk -v a="$1" -v b="$2" 'BEGIN { if (a !~ /^-?[0-9.]+$/ || b !~ /^-?[0-9.]+$/) exit 2; print a - b }'; }
# The chest stack as one JSON line: item id, count and its components (SNBT).
chest_stack() { ctl inventory | jq -c '.armor[] | select(.part=="chest")'; }
chest_components() { chest_stack | jq -r '.components // ""'; }
# horizon_energy:N out of the stack's components, empty when there is none.
chest_energy() { chest_components | grep -o 'horizon_energy:[0-9]*' | head -1 | cut -d: -f2 || true; }
# The FE reading of the last `/ironjetpacks fuel` reply, empty if none.
query_fuel() { messages | grep 'Iron Jetpacks:.*FE, engine' | head -1 | grep -o '[0-9]* / 800000' | cut -d' ' -f1 || true; }

# Hold the jump key and sample the feet for one second; echo the peak.
hold_and_sample() { # hold_and_sample PHASE
    local phase=$1
    local start
    start=$(feet_y)
    local peak=$start
    ctl key space down >/dev/null
    for tick in $(seq 1 10); do
        sleep 0.1
        local y v
        y=$(feet_y)
        v=$(vel_y)
        echo "  $phase t=${tick} y=$y vy=$v" >&2
        peak=$(awk -v p="$peak" -v y="$y" 'BEGIN { print (y > p) ? y : p }')
    done
    ctl key space up >/dev/null
    printf '%s' "$peak"
}

cleanup() {
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    rm -rf "$SCRATCH" 2>/dev/null || true
}
# Print the run's real exit status: `PASS:` alone is not exit-status evidence.
finish() {
    local status=$?
    cleanup
    echo "EXIT:$status"
}
trap finish EXIT

# --creative creates the world with Allow Commands (owner permission 4); the
# gamemode is switched to survival below, so the tank really drains. In
# EMBEDDED=1 there is no `--dev-mod`: the bundle's own content file is the only
# source of the jetpack item.
launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    local args=(--game "$GAME" --world "$world" --seed 20261103 --headless --wait
                --creative --resources "$RES")
    if [ "$EMBEDDED" = 1 ]; then
        echo "  launch: no --dev-mod; the bundle carries its own content" >&2
    else
        args+=(--dev-mod "$PORT/content")
    fi
    "$CTL" --socket "$SOCK" launch "${args[@]}"
    ctl wait-ready 120
}

# A high Overworld arena: clear of the surface mobs and of the End's exit
# portal/dragon breath. The chunks around 0,0 are loaded by the spawn.
arena() {
    ctl command "gamemode survival @s"
    ctl command "tp @s 0.5 100 0.5"
    ctl wait 5
    ctl command "fill -8 99 -8 8 99 8 minecraft:stone"
    ctl command "fill -8 100 -8 8 116 8 minecraft:air"
    ctl command "kill @e[type=!player]"
    floor=
    for _ in $(seq 1 40); do
        floor=$(ctl command "execute if block 0 99 0 minecraft:stone run say FLOOR_OK")
        case "$floor" in *FLOOR_OK*) break ;; esac
        ctl command "fill -8 99 -8 8 99 8 minecraft:stone" >/dev/null
        sleep 0.25
    done
    case "$floor" in *FLOOR_OK*) ;; *) fail "the stone floor did not appear" ;; esac
    ctl command "tp @s 0.5 100 0.5"
    ground=
    for _ in $(seq 1 80); do
        ground=$(ctl state | jq -r '.player.on_ground')
        [ "$ground" = "true" ] && break
        sleep 0.25
    done
    [ "$ground" = "true" ] || fail "the player is not on the floor"
    awk -v y="$(feet_y)" 'BEGIN { exit !(y > 99.5) }' || fail "the player is not on the stone floor (y=$(feet_y))"
    [ "$(ctl state | jq -r '.player.flying')" = "false" ] || fail "the player starts flying"
    echo "  start y=$(feet_y) health=$(health)"
}

# The `.horizon-cache` folder the helper writes into is relative to the game's
# working directory; accept the scratch root or its game folder, so the check
# does not depend on the launcher's cwd choice.
cache_file() { # cache_file <name>
    local name=$1 path
    for path in "$SCRATCH/.horizon-cache/$name" "$SCRATCH/game/.horizon-cache/$name"; do
        if [ -f "$path" ]; then printf '%s' "$path"; return 0; fi
    done
    return 1
}

# The control for EMBEDDED=1: the same binary (every hook is in it) with the
# embedded content made unavailable — `.horizon-cache` is a plain file, so the
# helper can neither compare nor write its content file and falls back to
# `Continue`. The item id must then be unknown; if it resolves, the run proves
# nothing about the embedded file.
control_leg() {
    echo "== control: the same game with no embedded content file =="
    rm -rf "$SCRATCH/.horizon-cache" "$SCRATCH/game/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/game/.horizon-cache"
    launch "$CONTROL_WORLD"
    local reply
    reply=$(ctl command "item replace entity @s armor.chest with ironjetpacks:jetpack" 2>&1 || true)
    echo "  item reply: $(tr -d '\n' <<<"$reply" | cut -c1-200)"
    ctl command "item replace entity @s hotbar.0 with minecraft:stick" >/dev/null
    ctl wait 4 >/dev/null
    ctl inventory | jq -e '[.armor[], .hotbar[], .main[] | select(.item == "ironjetpacks:jetpack")] | length == 0' >/dev/null \
        || fail "the control game has the jetpack item, so the content did not come from the embedded file"
    ctl inventory | jq -e '[.armor[], .hotbar[], .main[] | select(.item == "minecraft:stick")] | length >= 1' >/dev/null \
        || fail "the control game cannot even give a vanilla item: the check is not a valid control"
    [ -f "$SCRATCH/.horizon-cache" ] && [ -f "$SCRATCH/game/.horizon-cache" ] \
        || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    echo "  without the embedded file: no jetpack item, a vanilla stick still lands"
    ctl quit >/dev/null 2>&1 || true
    rm -f "$SCRATCH/.horizon-cache" "$SCRATCH/game/.horizon-cache"
}

# EMBEDDED=1: the bundle must have written its content out at startup, byte for
# byte the delivered content mod.
embedded_content_check() {
    local cache
    if ! cache=$(cache_file ironjetpacks-embedded-content.hmod); then
        echo "  no embedded content written; cache locations seen:" >&2
        find "$SCRATCH" -maxdepth 3 -name '.horizon-cache' -o -maxdepth 3 -name '*embedded-content.hmod' 2>/dev/null | head -5 >&2
        fail "the bundle did not write its embedded content into .horizon-cache/"
    fi
    cmp -s "$cache" "$PORT/content/ironjetpacks.hmod" \
        || fail "the embedded content differs from content/ironjetpacks.hmod"
    echo "  embedded content: $(wc -c < "$cache" | tr -d ' ') B at $cache, identical to content/ironjetpacks.hmod"
}

echo "== launch (high Overworld, cheats on so a level-2 mod command runs) =="
if [ "$CONTROL" = 1 ]; then control_leg; fi
launch
if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi
arena

echo "== control: no jetpack (disabled), jump held =="
ctl command "item replace entity @s armor.chest with minecraft:air"
ctl wait 2
start=$(feet_y)
peak=$(hold_and_sample no-jetpack)
sleep 1.5
plain=$(rise "$peak" "$start")
echo "  no-jetpack peak rise: $plain"
awk -v r="$plain" 'BEGIN { exit !(r <= 1.6) }' || fail "a plain jump rose $plain blocks (sampling or physics changed)"

echo "== a fresh jetpack: empty tank, engine off, jump held =="
ctl command "item replace entity @s armor.chest with ironjetpacks:jetpack"
ctl wait 2
echo "  fresh chest: $(chest_stack)"
start=$(feet_y)
peak=$(hold_and_sample empty-tank)
sleep 1.5
empty=$(rise "$peak" "$start")
echo "  empty-tank peak rise: $empty"
awk -v r="$empty" 'BEGIN { exit !(r <= 1.6) }' || fail "the uncharged jetpack rose $empty blocks"

ctl chat "/ironjetpacks fuel" >/dev/null
ctl wait 3
charge_query=$(messages | grep 'Iron Jetpacks:' | head -1 || true)
echo "  fresh tank query: ${charge_query:-none}"
case "$charge_query" in
    *"0 / 800000"*) ;;
    *) fail "a fresh jetpack's tank is not 0/800000: ${charge_query:-no answer}" ;;
esac

if [ "${NO_JETPACK:-0}" != 1 ]; then
    echo "== charge the item: /ironjetpacks fuel 800000 (sent as chat) =="
    ctl chat "/ironjetpacks fuel 800000" >/dev/null
    ctl wait 3
    charged=$(messages | grep 'fuel set to 800000 / 800000' | head -1 || true)
    echo "  charge reply: ${charged:-none}"
    [ -n "$charged" ] || fail "the fuel command did not fill the item's tank"
    ctl chat "/ironjetpacks fuel" >/dev/null
    ctl wait 3
    full=$(messages | grep '800000 / 800000 FE, engine off' | head -1 || true)
    echo "  query reply: ${full:-none}"
    [ -n "$full" ] || fail "the query did not report a full tank with the engine off"

    echo "== the tank is on the chest stack, not in a map =="
    charged_stack=$(chest_stack)
    charged_energy=$(chest_energy)
    echo "  chest stack: $charged_stack"
    [ "$(chest_stack | jq -r '.item')" = "ironjetpacks:jetpack" ] \
        || fail "the chest slot does not hold the jetpack: $charged_stack"
    [ "$charged_energy" = "800000" ] \
        || fail "the chest stack carries ${charged_energy:-no horizon_energy}, not 800000"

    echo "== the engine is still off: a full tank alone must not fly =="
    start=$(feet_y)
    peak=$(hold_and_sample engine-off)
    sleep 1.5
    engine_off=$(rise "$peak" "$start")
    echo "  engine-off peak rise: $engine_off"
    awk -v r="$engine_off" 'BEGIN { exit !(r <= 1.6) }' || fail "the jetpack flew $engine_off blocks with the engine off"

    echo "== toggle the engine on: the same jump must now thrust =="
    ctl chat "/ironjetpacks engine" >/dev/null
    ctl wait 3
    toggled=$(messages | grep 'engine on' | head -1 || true)
    echo "  toggle reply: ${toggled:-none}"
    [ -n "$toggled" ] || fail "the engine toggle did not turn the engine on"
    ctl chat "/ironjetpacks fuel" >/dev/null
    ctl wait 3
    armed=$(messages | grep '800000 / 800000 FE, engine on' | head -1 || true)
    echo "  query reply: ${armed:-none}"
    [ -n "$armed" ] || fail "the query did not report the engine on"
fi

echo "== jetpack: iron tier, charged, engine on, jump held =="
if [ "${NO_JETPACK:-0}" = 1 ]; then
    echo "  NO_JETPACK=1: removing the jetpack for the counterfactual"
    ctl command "item replace entity @s armor.chest with minecraft:air"
    ctl wait 2
fi
start=$(feet_y)
peak=$(hold_and_sample jetpack)
sleep 1.5
thrust=$(rise "$peak" "$start")
echo "  jetpack peak rise: $thrust"
echo "  health after landing: $(health)"
awk -v r="$thrust" 'BEGIN { exit !(r > 2.0) }' || fail "the jetpack rose only $thrust blocks"

if [ "${NO_JETPACK:-0}" != 1 ]; then
    echo "== the flight burns the tank on the item =="
    ctl chat "/ironjetpacks fuel" >/dev/null
    ctl wait 3
    after=$(messages | grep 'Iron Jetpacks:' | head -1 || true)
    echo "  post-flight query: ${after:-none}"
    fuel_after=$(echo "$after" | grep -o '[0-9]* / 800000' | cut -d' ' -f1 || true)
    [ -n "$fuel_after" ] || fail "the post-flight query gave no reading: ${after:-none}"
    awk -v f="$fuel_after" 'BEGIN { exit !(f > 0 && f < 800000) }' \
        || fail "the tank did not drain during the flight (reported $fuel_after)"
    post_stack=$(chest_stack)
    post_energy=$(chest_energy)
    echo "  chest stack after the flight: $post_stack"
    [ "$post_energy" = "$fuel_after" ] \
        || fail "the chest stack holds ${post_energy:-no horizon_energy} but the query said $fuel_after"

    echo "== input edge: releasing the jump key stops thrust and drain =="
    ctl chat "/ironjetpacks fuel" >/dev/null
    ctl wait 3
    before_edge=$(query_fuel)
    [ -n "$before_edge" ] || fail "no fuel reading before the input-edge phase"
    ctl key space down >/dev/null
    sleep 0.4
    vy_held=$(vel_y)
    ctl key space up >/dev/null
    echo "  vy while the key was held: $vy_held (fuel $before_edge)"
    awk -v v="$vy_held" 'BEGIN { exit !(v > 0.2) }' || fail "no thrust while the jump key was held (vy=$vy_held)"
    sleep 0.4
    ctl chat "/ironjetpacks fuel" >/dev/null
    ctl wait 3
    released_one=$(query_fuel)
    sleep 1.4
    ctl chat "/ironjetpacks fuel" >/dev/null
    ctl wait 3
    released_two=$(query_fuel)
    vy_rest=$(vel_y)
    echo "  released: fuel $released_one then $released_two, vy $vy_rest"
    [ -n "$released_one" ] && [ -n "$released_two" ] || fail "no fuel reading after the key release"
    [ "$released_one" = "$released_two" ] \
        || fail "the tank kept draining with the key released ($released_one -> $released_two)"
    awk -v v="$vy_rest" 'BEGIN { exit !(v < 0.2) }' || fail "the jetpack still thrusts with the key released (vy=$vy_rest)"
    awk -v a="$before_edge" -v b="$released_one" 'BEGIN { exit !(b < a) }' \
        || fail "the held edge did not spend fuel ($before_edge -> $released_one)"

    echo "== empty tank, engine on: still no thrust =="
    ctl chat "/ironjetpacks fuel 0" >/dev/null
    ctl wait 3
    drained=$(messages | grep 'fuel set to 0 / 800000' | head -1 || true)
    echo "  drain reply: ${drained:-none}"
    [ -n "$drained" ] || fail "the fuel command did not empty the item's tank"
    ctl chat "/ironjetpacks fuel" >/dev/null
    ctl wait 3
    empty_armed=$(messages | grep '0 / 800000 FE, engine on' | head -1 || true)
    echo "  query reply: ${empty_armed:-none}"
    [ -n "$empty_armed" ] || fail "the query did not report 0 / 800000 FE with the engine on"
    # An empty battery carries no custom data at all: the pin drops the key
    # (`with_energy` removes `horizon_energy` at 0), so absent means empty.
    empty_stack_energy=$(chest_energy)
    echo "  chest stack after draining: $(chest_stack)"
    { [ -z "$empty_stack_energy" ] || [ "$empty_stack_energy" = "0" ]; } \
        || fail "the chest stack still carries horizon_energy=$empty_stack_energy after draining it"
    start=$(feet_y)
    peak=$(hold_and_sample empty-tank-armed)
    sleep 1.5
    empty_armed_rise=$(rise "$peak" "$start")
    echo "  empty-tank-armed peak rise: $empty_armed_rise"
    awk -v r="$empty_armed_rise" 'BEGIN { exit !(r <= 1.6) }' \
        || fail "an empty item tank still thrust $empty_armed_rise blocks with the engine on"

    echo "PASS: jetpack rose $thrust blocks with the engine on and a charged item tank; the disabled control (no jetpack) rose $plain; the fresh empty tank and the engine-off control rose $empty and $engine_off; the tank drained on the item to $fuel_after (stack horizon_energy $post_energy, matching the query), held at $released_one while the key was released, and an empty tank with the engine on rose only $empty_armed_rise"
else
    fail "the counterfactual rose $thrust blocks without a jetpack: the flight check is not fail-capable"
fi
echo "EXIT:0"
