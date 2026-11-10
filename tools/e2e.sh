#!/usr/bin/env bash
# In-game check for Just Dire Things slice 1: the Simple Coal Generator on a
# real game.
#
# The arena is a stone floor at y=100 in the Overworld, so every block read and
# command is in the player's own dimension. The native bundle is compiled into
# the game.
#
# Default run (EMBEDDED=1): the game is launched with **only the native bundle**
# (no `--dev-mod`) and the content comes from the bundle's embedded copy
# (`mixins/justdirethings/data/justdirethings-content.hmod`, the AE2 pattern).
# The run also checks the file the helper wrote, and CONTROL=1 (default with
# EMBEDDED) first launches the same binary with `.horizon-cache` unusable, where
# the ids must be unknown.
# EMBEDDED=0: the game loads `content/` as a dev mod (the pre-embedding shape).
#
# Default run:
#  - control "no fuel": a placed generator makes no FE for 40 ticks and is not
#    burning;
#  - coal in the fuel slot (the container path, automation's way in) starts the
#    burn: one item is consumed, the burn is worth 1,600 x 15 = 24,000 FE at
#    60 FE/t, and the buffer fills;
#  - the carried Primal Coal (`coal_t1`, burn_time 4,800, speed_multiplier 2.0)
#    is worth 72,000 FE at 120 FE/t, so the content's fuel data reaches the
#    runtime (a generator beside it reports a different total and multiplier);
#  - control "full buffer": a generator set to its 1,000,000 FE capacity burns
#    no fuel (the burn bar holds still) until the buffer is drained again, when
#    it resumes;
#  - `/justdirethings set 2000000` reads back 1,000,000 (the capacity clamps);
#  - control "unfuelled neighbour": a second generator with no fuel stays at 0.
#
# SELFTEST=1 flips the "no fuel" assertion (it must FAIL), proving the scenario
# can fail.
set -euo pipefail
export LC_ALL=C

CLONE=${CAMPAIGN_ROOT:-$(cd "$(dirname "$0")/../../.." && pwd)}
PORT=$CLONE/ports/justdirethings
CTL=${CTL:-$CLONE/ports/_shared/bin/horizon-1d63e0a5/horizon-ctl}   # pin-matched shared ctl
GAME=${GAME:-$PORT/target/horizon-cargo/fast/horizon-client}
RES=${RES:-${HORIZON_RES:-${HORIZON_RES:-$HOME/.local/horizon-client/26.3}}}
SOCK=${SOCK:-/tmp/opencode/justdirethings-e2e.sock}
SELFTEST=${SELFTEST:-0}
KEEP=${KEEP:-0}
EMBEDDED=${EMBEDDED:-1}
CONTROL=${CONTROL:-$EMBEDDED}
WORLD=JustDireThingsE2E
CONTROL_WORLD=JustDireThingsControl

[ -x "$CTL" ] || { echo "missing horizon-ctl: $CTL" >&2; exit 1; }
[ -x "$GAME" ] || { echo "missing horizon-client: $GAME" >&2; exit 1; }
[ -d "$RES/assets" ] || { echo "missing prepared resources: $RES" >&2; exit 1; }

SCRATCH=$(mktemp -d /tmp/opencode/justdirethings-home.XXXXXX)
export HORIZON_HOME=$SCRATCH/game
export HORIZON_LAUNCHER_HOME=$SCRATCH/launcher
export XDG_DATA_HOME=$SCRATCH/data
export XDG_CONFIG_HOME=$SCRATCH/config
export HORIZON_NO_AUDIO=1
mkdir -p "$(dirname "$SOCK")" "$XDG_CONFIG_HOME"
rm -f "$SOCK"

ctl() { "$CTL" --socket "$SOCK" "$@"; }
say() { ctl chat "/$1" >/dev/null; }
fail() { echo "FAIL: $*" >&2; ctl logs --lines 80 >&2 || true; exit 1; }

# The non-null slots of the container at a position, as JSON.
slots() { ctl container "$1" "$2" "$3" | jq -c '[.slots[] | select(. != null)]'; }
# has X Y Z ITEM COUNT: the container holds exactly COUNT of ITEM.
has() { # has X Y Z ITEM COUNT
    local got
    got=$(slots "$1" "$2" "$3")
    jq -e --arg item "$4" --argjson count "$5" \
        '[.[] | select(.item == $item) | .count] | add // 0 | . == $count' <<<"$got" >/dev/null \
        || fail "expected $5 x $4 at $1,$2,$3; got $got"
    echo "  has($1,$2,$3) $4 x$5: ok"
}
# info X Y Z: this block's status line, after a fresh reply arrives (the t=
# field makes each reply unique).
info() { # info X Y Z
    local out line before
    before=$(ctl messages 20 | jq -r '.chat[]' | grep -E " at $1,$2,$3:" | head -1)
    say "justdirethings info $1 $2 $3"
    for _ in $(seq 1 25); do
        ctl wait 4 >/dev/null
        out=$(ctl messages 20)
        line=$(jq -r '.chat[]' <<<"$out" | grep -E " at $1,$2,$3:|No Just Dire Things block" | head -1)
        if [ -n "$line" ] && [ "$line" != "$before" ]; then
            echo "$line"
            return
        fi
    done
    echo "NO-INFO $(jq -c '{chat,action_bar}' <<<"$out")"
}
# set_energy X Y Z AMOUNT: the harness control `/justdirethings set`, retried
# until the block reads the value back (chat commands are asynchronous).
set_energy() { # X Y Z AMOUNT
    local want=$4 line
    say "justdirethings set $want $1 $2 $3"
    for _ in $(seq 1 10); do
        line=$(info "$1" "$2" "$3")
        if [ "$(energy_of "$line")" = "$want" ]; then
            echo "  set($1,$2,$3) = $line"
            return
        fi
        say "justdirethings set $want $1 $2 $3"
    done
    fail "could not set $1,$2,$3 to $want FE: $line"
}

# The fields of an info line.
energy_of() { sed -nE 's/.*: ([0-9]+)\/[0-9]+ FE.*/\1/p' <<<"$1"; }
fe_of()     { sed -nE 's/.*fe ([-0-9]+) of ([-0-9]+).*/\1/p' <<<"$1"; }
worth_of()  { sed -nE 's/.*fe ([-0-9]+) of ([-0-9]+).*/\2/p' <<<"$1"; }
burn_rem_of(){ sed -nE 's/.*burn ([0-9]+)\/([0-9]+).*/\1/p' <<<"$1"; }
mult_of()   { sed -nE 's/.*multiplier ([0-9]+).*/\1/p' <<<"$1"; }
burning_of(){ sed -nE 's/.*burning (true|false).*/\1/p' <<<"$1"; }

cleanup() {
    rc=$?
    ctl quit >/dev/null 2>&1 || true
    sleep 1
    if [ "$KEEP" = 1 ]; then
        echo "kept scratch: $SCRATCH" >&2
    else
        rm -rf "$SCRATCH" 2>/dev/null || true
    fi
    echo "EXIT:$rc"
}
trap cleanup EXIT

launch() { # launch [WORLD]
    local world=${1:-$WORLD}
    rm -f "$SOCK"
    cd "$SCRATCH"
    local args=(--game "$GAME" --world "$world" --seed 20261105 --creative --headless --wait
                --resources "$RES")
    if [ "$EMBEDDED" = 1 ]; then
        echo "  launch: no --dev-mod; the bundle carries its own content"
    else
        args+=(--dev-mod "$PORT/content")
    fi
    "$CTL" --socket "$SOCK" launch "${args[@]}" >/dev/null
    ctl wait-ready 120
    ctl wait 40
}

# The control for EMBEDDED=1: the same binary (every hook is in it) with the
# embedded content made unavailable — `.horizon-cache` is a plain file, so the
# helper can neither compare nor write its content file and falls back to
# `Continue`. Both ids must then be unknown; if they resolve, the run proves
# nothing about the embedded file.
#
# The helper writes to `std::env::current_dir()/.horizon-cache`; the game only
# calls `set_current_dir` for `--game-dir`, which this script never passes, so
# the cache is `$SCRATCH/.horizon-cache` (the launch `cd`s there). The control
# blocks exactly that path, and the assertion below catches a wrong guess: if
# the content still loads from somewhere else, the ids are known and the leg
# fails instead of passing vacuously.
control_leg() {
    echo "== control: the same game with no embedded content file =="
    rm -rf "$SCRATCH/.horizon-cache"
    : > "$SCRATCH/.horizon-cache"
    launch "$CONTROL_WORLD"
    arena
    ctl command "item replace entity @s hotbar.0 with justdirethings:coal_t1 1" >/dev/null || true
    ctl wait 4
    ctl inventory | jq -e '[.hotbar[], .main[] | select(.item == "justdirethings:coal_t1")] | length == 0' >/dev/null \
        || fail "the control game has a coal item, so the content did not come from the embedded file"
    ctl command "setblock 0 100 0 justdirethings:generatort1" >/dev/null || true
    ctl wait 4
    local name
    name=$(ctl block 0 100 0 | jq -r '.name // .block // empty')
    [ "$name" = "minecraft:air" ] || fail "the control game holds $name, so the block id was known"
    [ -f "$SCRATCH/.horizon-cache" ] || fail "the control's .horizon-cache was replaced: the helper wrote its content"
    echo "  without the embedded file: no coal item, the block stayed $name"
    ctl quit >/dev/null 2>&1 || true
    rm -f "$SCRATCH/.horizon-cache"
}

# EMBEDDED=1: the bundle must have written its content out at startup, byte for
# byte the delivered content mod.
embedded_content_check() {
    local cache=$SCRATCH/.horizon-cache/justdirethings-embedded-content.hmod
    [ -f "$cache" ] || fail "the bundle did not write its embedded content to $cache"
    cmp -s "$cache" "$PORT/content/justdirethings.hmod" \
        || fail "the embedded content differs from content/justdirethings.hmod"
    echo "  embedded content: $(wc -c < "$cache" | tr -d ' ') B at $cache, identical to content/justdirethings.hmod"
}

# A stone floor at y=99 with clear air above, in the Overworld, and the player
# standing on it at (0.5, 100, 3.5).
arena() {
    ctl tp 0.5 101 3.5 >/dev/null
    ctl wait 20
    ctl command "fill -8 99 -8 24 99 24 minecraft:stone" >/dev/null
    ctl command "fill -8 100 -8 24 110 24 minecraft:air" >/dev/null
    ctl command "kill @e[type=!player]" >/dev/null
    ctl tp 0.5 100 3.5 >/dev/null
    ctl wait 20
    local y feet floor
    y=$(ctl state | jq -r '.player.position[1] // 0')
    feet=$(ctl block 0 100 3 | jq -r '.name // .block // empty')
    floor=$(ctl block 0 99 3 | jq -r '.name // .block // empty')
    echo "  arena: y=$y floor(0,99,3)=$floor feet(0,100,3)=$feet"
    awk -v y="$y" 'BEGIN { exit !(y >= 99.9) }' || fail "the player is not on the arena floor (y=$y)"
    [ "$feet" = "minecraft:air" ] || fail "the arena volume is not clear ($feet at 0,100,3)"
    [ "$floor" = "minecraft:stone" ] || fail "the arena floor is missing ($floor at 0,99,3)"
}

place() { # place X Y Z ID
    ctl command "setblock $1 $2 $3 $4" >/dev/null
    ctl wait 4
    local got
    got=$(ctl block "$1" "$2" "$3" | jq -r '.name // .block // empty')
    echo "  block($1,$2,$3) = $got"
    [ "$got" = "$4" ] || fail "expected $4 at $1,$2,$3, got '$got'"
}

echo "== launch (Overworld arena at y=100, the native bundle) =="
if [ "$CONTROL" = 1 ]; then control_leg; fi
launch
if [ "$EMBEDDED" = 1 ]; then embedded_content_check; fi
arena

echo "== place two generators =="
place 0 100 0 justdirethings:generatort1
place 4 100 0 justdirethings:generatort1

echo "== control: no fuel means no FE =="
line=$(info 0 100 0)
echo "  generator: $line"
[ "$(energy_of "$line")" = 0 ] || fail "a generator with no fuel stored FE: $line"
[ "$(burning_of "$line")" = false ] || fail "a generator with no fuel is burning: $line"
ctl wait 40
line=$(info 0 100 0)
echo "  after 40 ticks: $line"
[ "$(energy_of "$line")" = 0 ] || fail "a generator with no fuel made FE: $line"
if [ "$SELFTEST" = 1 ]; then
    echo "  SELFTEST: expecting the empty generator to have made FE (this must fail)"
    [ "$(energy_of "$line")" != 0 ] || fail "SELFTEST flipped: the unfuelled generator made no FE, as it should"
fi
line4=$(info 4 100 0)
echo "  neighbour: $line4"
[ "$(energy_of "$line4")" = 0 ] || fail "an unfuelled generator stored FE: $line4"

echo "== coal starts the burn: 1,600 x 15 = 24,000 FE at 60 FE/t =="
ctl container 0 100 0 minecraft:coal 5 >/dev/null
ctl wait 6
# the first item is already burning on the next tick, so four are left
has 0 100 0 minecraft:coal 4
line=$(info 0 100 0)
echo "  $line"
[ "$(burning_of "$line")" = true ] || fail "the generator is not burning coal: $line"
[ "$(mult_of "$line")" = 1 ] || fail "vanilla coal's multiplier is not 1: $line"
[ "$(worth_of "$line")" = 24000 ] || fail "coal should be worth 24,000 FE: $line"
[ "$(energy_of "$line")" -gt 0 ] || fail "the generator made no FE from coal: $line"
[ "$(energy_of "$line")" -le 24000 ] || fail "the generator made more than one coal's FE: $line"

echo "== the carried Primal Coal is worth 72,000 FE at 120 FE/t =="
ctl container 4 100 0 justdirethings:coal_t1 5 >/dev/null
ctl wait 6
has 4 100 0 justdirethings:coal_t1 4
line4=$(info 4 100 0)
echo "  $line4"
[ "$(burning_of "$line4")" = true ] || fail "the generator is not burning Primal Coal: $line4"
[ "$(mult_of "$line4")" = 2 ] || fail "Primal Coal's multiplier is not 2: $line4"
[ "$(worth_of "$line4")" = 72000 ] || fail "Primal Coal should be worth 72,000 FE: $line4"
[ "$(energy_of "$line4")" -gt 0 ] || fail "the generator made no FE from Primal Coal: $line4"

echo "== control: a full buffer burns nothing until it is drained =="
set_energy 0 100 0 1000000
line=$(info 0 100 0)
fe_before=$(fe_of "$line")
ctl wait 40
line=$(info 0 100 0)
echo "  full buffer after 40 ticks: $line"
[ "$(energy_of "$line")" = 1000000 ] || fail "a full buffer changed: $line"
[ "$(fe_of "$line")" = "$fe_before" ] || fail "a full buffer kept burning ($fe_before -> $(fe_of "$line")): $line"

echo "== drained again, the burn resumes =="
# A running generator refills immediately, so read back a low value instead of
# an exact 0, then watch it climb.
say "justdirethings set 0 0 100 0"
ctl wait 8
line=$(info 0 100 0)
drained=$(energy_of "$line")
echo "  after draining: $line"
[ "$drained" -lt 1000 ] || fail "the buffer did not drain: $line"
ctl wait 40
line=$(info 0 100 0)
echo "  after 40 more ticks: $line"
[ "$(energy_of "$line")" -gt "$drained" ] || fail "the generator did not resume when drained: $line"

echo "== the capacity clamps what is set =="
say "justdirethings set 2000000 0 100 0"
ctl wait 8
line=$(info 0 100 0)
echo "  set 2000000 -> $line"
[ "$(energy_of "$line")" = 1000000 ] || fail "the buffer did not clamp to its capacity: $line"

echo "PASS: an unfuelled generator made no FE, coal burned at 60 FE/t for 24,000 FE, the carried Primal Coal burned at 120 FE/t for 72,000 FE, a full buffer burned nothing until drained, and the capacity clamped a larger set"
