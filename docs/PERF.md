# Performance hunting: a field log

This documents, step by step and in the order it actually happened, how we
found why Rondelek burned ~100% CPU while idle and felt laggy — and how we
got it down to ~3% idle / ~28% while animating. Every command is an exact
copy from the session (only throwaway temp paths are shortened to `$SP`).

The symptom: fans spinning, `htop` showing 89–105% CPU even when the app sat
idle, and a noticeable delay between pressing a key and the app reacting.

Each step has two explanations:

- 🟢 **Plain**: for someone who codes (PHP counts!) but is new to Rust & Linux.
- 🔬 **Technical**: what really happened and why that tool, that flag.

The one method that matters, before any tool: **measure → form a hypothesis →
run an experiment that can prove the hypothesis wrong → repeat**. Every step
below is one turn of that loop. Never fix what you haven't measured; your
first guess will usually be wrong (ours was).

---

## Step 1 — Reproduce with a release build, measure the process

```sh
cargo build --release

./target/release/rondelek > /dev/null 2>&1 &
APP=$!
sleep 6
pidstat -p $APP 1 5
```

Output (the moment of truth):

```
22:51:47   UID   PID    %usr %system  %CPU  Command
Average:  1000 37373   35.53   39.52 75.05  rondelek
```

🟢 **Plain**: `cargo build --release` builds the optimized version (debug
builds are 10–100× slower — never profile those). The `&` runs the app in the
background; `$!` is the shell variable holding the id (PID) of the process we
just started. `pidstat -p <PID> 1 5` prints that process's CPU usage 5 times,
once per second — like `htop`, but as text you can log and compare. Verdict:
75% CPU doing *nothing*. Bug confirmed, with a number attached.

🔬 **Technical**: `pidstat` (from the `sysstat` package) splits CPU into
`%usr` (your code) and `%system` (time inside kernel syscalls). The split was
the first real clue: ~40% *system* time for a GUI app that should be asleep
means it's hammering syscalls — an event-loop problem, not heavy computation.
Heavy math (FFT, rendering) would show up as `%usr`.

## Step 2 — Which thread?

```sh
ps -L -o tid,pcpu,comm -p $APP | sort -k2 -rn | head -8
```

```
  37373 82.0 rondelek          <- main thread
  37385  0.4 smithay-clipboa
  37381  0.4 [vkps] Update
```

🟢 **Plain**: A process contains threads — separate workers sharing memory.
`ps -L` lists them with per-thread CPU. If some background worker (audio?)
were guilty, we'd see it here. Instead: the **main thread** (the one running
the UI loop) eats it all. Also a freebie: the thread named
`smithay-clipboard` told us the app runs on **Wayland** (the newer Linux
display system; the older one is X11) — remember that for later.

🔬 **Technical**: `-L` shows LWPs (threads), `-o tid,pcpu,comm` picks thread
id, CPU%, and thread name. Thread names are set via `pthread_setname_np` and
often identify the library that spawned them — `smithay-*` is the Rust
Wayland client stack, `[vkps]`/`[vkrt]` are NVIDIA driver threads. Reading
thread names is cheap fingerprinting of your dependency stack at runtime.

## Step 3 — Try the real profiler first (and hit the wall)

```sh
cat /proc/sys/kernel/perf_event_paranoid    # -> 4
which perf                                  # -> /usr/bin/perf
```

🟢 **Plain**: `perf` is Linux's gold-standard profiler: it interrupts the
program thousands of times a second, records where it was each time, and
builds a statistical picture of where time goes. `/proc` is a fake filesystem
where the kernel exposes its settings as readable files. This one says how
much profiling non-root users may do; `4` means "none at all". Blocked — so
we pick different tools. (On your own machine you could
`sudo sysctl kernel.perf_event_paranoid=1` and use
`perf record -g -p <PID>` + `perf report`.)

🔬 **Technical**: `perf_event_paranoid=4` is a hardened-kernel setting
(3 already blocks unprivileged sampling; 4 is distro-extra). Without
`CAP_PERFMON` there is no `perf_event_open`. Fallback hierarchy when denied:
strace (syscall layer), ltrace (library calls), gdb thread-apply-all-bt in a
loop ("poor man's profiler"), eBPF if you have root. We had high `%system`,
so the syscall layer was exactly where we wanted to look anyway.

## Step 4 — Syscall census with strace

Attaching to the running process failed:

```sh
timeout 5 strace -c -f -p $APP
# strace: attach: ptrace(PTRACE_SEIZE, 37705): Operation not permitted
cat /proc/sys/kernel/yama/ptrace_scope      # -> 1
```

So launch the app *under* strace instead (a parent may always trace its
child):

```sh
timeout 12 strace -c -f ./target/release/rondelek > /dev/null 2> $SP/strace.txt
```

Top of the summary table:

```
 %time     seconds  usecs/call     calls    errors syscall
 76.67   13.683922        1050     13022      3491 futex
 14.94    2.666395          31     84330           epoll_pwait
  2.59    0.462222           2    168681           epoll_ctl
  1.36    0.243207           2     87475     84330 read
  1.36    0.243043           2     84332           timerfd_settime
```

🟢 **Plain**: A *syscall* is any request a program makes to the kernel
("read this file", "wait for events", "sleep"). `strace` records every one.
`-c` makes a count/summary instead of a firehose, `-f` follows all threads.
The numbers are wild: in 12 seconds the app asked the kernel "any events for
me?" (`epoll_pwait`) **84,330 times** — about **7,000 times per second** —
and 84,330 of the 87,475 `read` calls answered "nothing there" (the errors
column). Imagine a PHP worker polling an empty queue in a `while(true)` with
no `sleep()` — that's this, in C clothing.

🔬 **Technical**: `-c` aggregates per-syscall time/calls/errors; error counts
are gold — `read` erroring 96% of the time means someone reads a
non-blocking fd that's empty, i.e. spurious wakeups. `epoll_pwait` is the
Linux event-wait primitive every event loop sits in; a *healthy* idle GUI
does a handful per second with long timeouts. 84k epoll_pwait + 168k
epoll_ctl (re-arming!) + 84k timerfd_settime forms a signature: an event loop
iterating ~7kHz, re-registering oneshot sources every lap. The futex 76%
"time" is mostly threads *parked waiting* (time-in-syscall includes blocked
time) — don't be fooled into chasing the biggest %time; cross-check calls ×
errors and what the syscall *is*. This misdirection is the classic strace -c
trap.

## Step 5 — First hypothesis… falsified

The code called `ui.ctx().request_repaint()` unconditionally every frame —
"repaint forever, as fast as you can". Prime suspect. Experiment: throttle it
to 60 FPS and re-measure.

```rust
// was: ui.ctx().request_repaint();
ui.ctx()
    .request_repaint_after(std::time::Duration::from_millis(16));
```

```sh
cargo build --release
./target/release/rondelek > /dev/null 2>&1 &
APP=$!; sleep 5
pidstat -p $APP 1 5 | tail -3
# Average: ... 76.40%  <- NO CHANGE
```

🟢 **Plain**: We changed one thing, re-measured, and the number refused to
move. That *hurts* but it's the method working: our favorite theory is dead,
and we didn't ship a "fix" that fixes nothing. This step is the whole reason
to measure before and after every change.

🔬 **Technical**: `request_repaint_after(16ms)` converts eframe's control
flow from Poll-ish to WaitUntil-based scheduling. If the spin persisted, the
spin source is *below* egui — in winit/calloop — or orthogonal to repaint
scheduling. One-variable experiments + a falsifiable prediction ("CPU should
drop to ~20%") is what separates profiling from folklore.

## Step 6 — Second hypothesis: the display backend. Bullseye.

Remember `smithay-clipboard` (Step 2)? The app runs on Wayland. Linux apps
fall back to X11 if Wayland is unavailable, and we can fake that per-process:

```sh
env -u WAYLAND_DISPLAY ./target/release/rondelek > /dev/null 2>&1 &
APP=$!; sleep 5
pidstat -p $APP 1 4 | tail -3
# Average: ... 14.00%   <- from 76% !
```

🟢 **Plain**: `env -u WAYLAND_DISPLAY cmd` runs the command with that one
environment variable removed — the app can't find Wayland, so it uses the
X11 compatibility path. Same binary, same screen: **76% → 14%**. The culprit
is not our drawing, our audio, or our visualizers — it's the *windowing
backend* the GUI library uses on Wayland. One command, and the search space
collapsed from "the whole app" to "one library's Wayland code".

🔬 **Technical**: This is differential diagnosis by environment: bisect over
runtime configuration before you bisect over code. Equivalents: `vblank_mode=0`
(vsync), `LIBGL_ALWAYS_SOFTWARE=1` (GPU vs CPU rendering), different
compositor, different machine. Each is a one-variable A/B with the same
binary — zero rebuilds, high information.

## Step 7 — Forensics: what exactly spins (optional depth, but satisfying)

Split the trace per thread, keep only the loop syscalls:

```sh
timeout 8 strace -ff -e trace=epoll_pwait,timerfd_settime,ppoll,ioctl \
    -o $SP/st ./target/release/rondelek > /dev/null 2>&1
wc -l $SP/st.* | sort -rn | head -3
#  102452 st.39626   <- main thread, ~13k syscalls/s
#    2190 st.39637
awk '{print $1}' $SP/st.39626 | sed 's/(.*//' | sort | uniq -c | sort -rn | head
#  50240 timerfd_settime
#  50239 epoll_pwait
#   1853 ioctl
grep -m3 "epoll_pwait" $SP/st.39626
# epoll_pwait(4, [...], 1024, -1,  ...) = 2     <- blocks, gets events
# epoll_pwait(4, [...], 1024, 0, ...)  = 1     <- timeout 0: just polls
# epoll_pwait(4, [], 1024, 0, ...)     = 0     <- polls again, NOTHING ready
```

Then identify the file descriptors involved:

```sh
ls -l /proc/$APP/fd            # list every open fd with what it points to
for n in 3 4 5 6 7 8 9; do echo -n "fd $n -> "; readlink /proc/$APP/fd/$n; done
# fd 3 -> socket:[277747]            (the Wayland socket)
# fd 4 -> anon_inode:[eventpoll]     (the epoll instance itself)
# fd 5 -> anon_inode:[eventfd]       (calloop's wakeup "ping")
# fd 6 -> anon_inode:[timerfd]       (calloop's timer)
```

🟢 **Plain**: `-ff -o name` writes one trace file per thread, so `wc -l`
instantly shows who's busy. The `grep` shows the smoking gun: the loop asks
"anything ready?" with a **timeout of 0** ("don't wait, answer now"), gets
"no", and asks again — thousands of times a second, in the gap before the
screen's next refresh. `/proc/<PID>/fd` is a directory where every file/
socket/pipe the process has open is visible as a numbered link — `readlink`
tells you what each number actually is. It's like var_dump for a process's
plumbing.

🔬 **Technical**: The pattern — per-iteration `EPOLL_CTL_MOD` re-arms of
ONESHOT sources (ping eventfd + timerfd), zero-timeout `epoll_pwait`, `read`
=> EAGAIN on the ping — is calloop dispatch overhead at ~6.3kHz. Meanwhile
`ioctl` ran at only ~230/s ≈ 60 paints/s × ~4 DRM submissions: real frames
were vsync-paced all along; the loop just *busy-waits between Wayland frame
callbacks* instead of blocking (winit 0.30.13 behavior; see winit #2690 /
discussion #3377). A `strace -e trace=write` pass showing *nobody* writes
fd 5 killed the "someone pings the loop" theory — drains were defensive.
Cross-referencing event **rates** (6.3k wakeups vs 65 frames vs 0 pings) is
what turns a trace from noise into a mechanism.

## Step 8 — Fix, then prove each fix with the same ruler

Fix 1: repaint policy (this alone fixed *idle* even on Wayland — with no
repaint permanently pending, the loop finally sleeps):

```rust
let animating = matches!(self.screen, AppScreen::Session | AppScreen::Calibrate)
    || self.config_panel.visible
    || self.camera.is_some()
    || self.auto_shot.is_some();
let delay = if animating { 33 } else { 100 };   // ~30 FPS / idle heartbeat
ui.ctx()
    .request_repaint_after(std::time::Duration::from_millis(delay));
```

Fix 2: opt-in X11 backend (`cargo run --release -- --x11`) via
`NativeOptions::event_loop_builder` + winit's `EventLoopBuilderExtX11`.

Verification matrix — same command, every configuration, longer windows
(`pidstat 1 10`) because single 4-second samples were ±5% noisy:

```sh
P=~/.local/share/rondelek/profiles/greg-2b50459b
m(){ sleep 6; pidstat -p $1 1 10 | tail -1; kill $1; sleep 1; }
for viz in 0 1 2; do
  RONDELEK_SESSION=$P/sessions/sm RONDELEK_PROFILE=$P RONDELEK_VIZ=$viz \
      ./target/release/rondelek --x11 >/dev/null 2>&1 & m $!
done
```

| Configuration | Before | After |
|---|---|---|
| Idle screen, Wayland | 76–90% | **3.5%** |
| Sampler, Wayland | ~79% | **28%** |
| Sampler, `--x11` | 31% | **27%** |

And attribution by subtraction, using the new "off" visualizer as baseline:
spectrum ≈ 27%, off ≈ 21% → the dot-matrix (FFT + tessellating hundreds of
circles) costs ~6 points; the rest is the 30 FPS UI loop itself.

🟢 **Plain**: Every fix gets measured with the *same* command as the original
symptom, or you can't claim it worked. The shell function `m(){ ...; }` is
just DRY for measurements. The "off visualizer" trick is worth stealing: to
find what a feature costs, build a version *without* it and subtract — no
profiler needed. We even verified `--x11` wasn't silently ignored:
`ls -l /proc/$APP/fd | grep -ci wayland` → `0` — zero Wayland connections.

🔬 **Technical**: Longer sampling windows shrink variance (~1/√n); pin down
your measurement protocol before comparing. Null-implementation baselines
(our `OffVisualizer`) give exact attribution where sampling profilers only
estimate. Final check that the loop actually sleeps now: re-run Step 4 and
watch `epoll_pwait` calls/s fall from ~7,000 to dozens.

---

## Part two — "the game burns 100%" (it didn't)

Everything above fixed the sampler (the egui app). Later, a new symptom: while
a voice-game (Vowel Runner) is running, `htop` shows a full core pegged at
100%. The obvious suspect is the game — it's the new, GPU-heavy thing on
screen. This part is the story of how the obvious suspect turned out to be
innocent, and the real culprit was a process we weren't even looking at.

The one twist that makes this worth writing down: **when two processes are
alive, "which PID?" comes before "which thread?"**. We skipped that question
once and it cost us.

## Step 9 — Measure the accused first

The game runs its own raylib window loop, capped with `set_target_fps(60)`.
There's a built-in headless harness (`RONDELEK_GAME_FRAMES=N` auto-quits after
N frames) so we can run the real loop unattended and put a number on it:

```sh
RONDELEK_GAME_FRAMES=3000 ./target/release/rondelek-game runner >/dev/null 2>&1 &
APP=$!; sleep 6
pidstat -p $APP 1 8
# Average: ... %usr 6.88  %system 2.88  %CPU 9.75  rondelek-game
ps -L -o tid,pcpu,comm -p $APP | sort -k2 -rn | head
#   ...  9.0 rondelek-game     <- main thread
```

🟢 **Plain**: Before believing "the game is the problem", we ran *just the
game* and measured it — the same first move as Step 1. Verdict: **~10% of one
core**, not 100%. The accused has an alibi. Whatever the user is seeing, it
isn't the game loop.

🔬 **Technical**: 600 frames took ~10 s (a clean 60 fps cap), and an
`strace -c` of the same run showed `clock_nanosleep` firing ~60×/s — the
`set_target_fps` sleep. raylib's default `WaitTime` is a partial-busy wait
(sleep ~95%, spin the last ~5%), which is exactly the ~10% we measured. The
loop sleeps; it is not the spinner.

## Step 10 — The clue that reframed everything: *how* it's launched

The number that finally matched the symptom came with one extra fact from the
user: 100% happens **when the game is launched from the app's Games screen**,
not when the game is run standalone. That's the whole ballgame — it means the
variable isn't the game, it's *the app that's still running behind it*.

The sampler spawns the game as a **sibling child process** and keeps running:

```rust
// app/src/app.rs::launch_game
Command::new(sibling).arg(id) /* + --profile <dir> */ .spawn()
```

🟢 **Plain**: The game opens its own fullscreen window *on top of* the sampler.
The sampler doesn't close — it's still there, hidden behind the game. So now
there are two programs running, and we'd only been measuring one of them. The
suspect list just doubled.

🔬 **Technical**: This is the differential-diagnosis reflex from Step 6, one
level up: when a symptom depends on *configuration* (launched-from-app vs
standalone), bisect the configuration. The only thing "launched from app"
adds is a live, now-occluded parent process. That's the new variable to
measure.

## Step 11 — Point pidstat at the *other* process

Same binary, but now measure the parent `rondelek` while a fullscreen game
sits on top of it — and compare Wayland against the X11 fallback from Step 8:

```sh
./target/release/rondelek >/dev/null 2>&1 &            APP=$!   # Wayland (default)
sleep 4
RONDELEK_GAME_FRAMES=100000 ./target/release/rondelek-game runner >/dev/null 2>&1 &
sleep 5
pidstat -p $APP 1 6 | tail -1
# Wayland, parent occluded:  ... %CPU 92.17  rondelek     <- there it is
```

```sh
./target/release/rondelek --x11 ...                              # X11 fallback
# X11, parent occluded:      ... %CPU  4.80  rondelek     <- immune
```

🟢 **Plain**: There's our 100%. It's the **sampler**, not the game — spinning
at 92% while completely hidden behind the game window. And it only does this on
Wayland; the `--x11` version stays at ~5%. Same bug family as Part one, same
cure worked, just a place we never thought to look because the window was
invisible.

🔬 **Technical**: The mechanism is the Step 7 spin, re-armed by a new trigger.
A Wayland surface that is fully occluded stops receiving `wl_surface` **frame
callbacks** — the compositor has nothing to show, so it never pings "draw the
next frame". winit's loop, if *any* repaint is pending, busy-waits for that
callback that will never come (winit #2690). Our Part-one fix left a 100 ms
idle heartbeat pending (`request_repaint_after(100)`); harmless while the
window is *visible* (real frame callbacks pace it to ~60 Hz), catastrophic
while it's *occluded* (no callbacks, so the pending repaint spins forever). An
`strace -c` of the occluded parent shows the same `epoll_pwait` +
`timerfd_settime` re-arm + `read`→`EAGAIN` fingerprint as Step 7.

## Step 12 — A measurement trap: the number that wouldn't sit still

One honest wrinkle worth recording. Early runs of the *visible* parent flip-
flopped: sometimes 84%, sometimes 4.5% — same binary, same command.

🟢 **Plain**: The reading changed between runs because it depended on something
we weren't controlling: whether the compositor had *actually* covered the
window at that instant (another window on top, or it happened to be
front-most). Frustrating, until we realised the flapping number *was itself
the clue* — the CPU tracks real on-screen occlusion, exactly what a
frame-callback-starvation bug would do.

🔬 **Technical**: The confound was occlusion state, which a backgrounded
launch doesn't pin down. The fix for the *experiment* is the same as always:
remove the uncontrolled variable. We forced deterministic occlusion by putting
a real fullscreen game on top every time (Step 11), turning a ±40-point noisy
reading into a stable 92%. If a benchmark won't hold still, you have a hidden
variable, not bad luck — find it before you trust any before/after.

## Step 13 — Fix, proven with forced occlusion

The parent has no reason to paint while it's hidden behind a game. So while a
game child is alive, request *nothing* — let the loop block in `epoll` — and
rely on the focus/occlusion event winit delivers when the game window closes
to wake it, reap the child, and resume:

```rust
self.frame_count += 1;
// Reap a finished game first, so the returning-focus frame unlocks the screen.
if let Some(child) = &mut self.game_child
    && matches!(child.try_wait(), Ok(Some(_)) | Err(_))
{ self.game_child = None; }

// While a game child owns the fullscreen window this window is occluded; on
// Wayland any pending repaint then busy-waits at ~100% (Step 11). Paint
// nothing — the focus event on game close wakes us to reap and resume.
if self.game_child.is_none() {
    let delay = if animating { 33 } else { 100 };
    ui.ctx().request_repaint_after(std::time::Duration::from_millis(delay));
}
```

Verified by adding a temporary `RONDELEK_TEST_HIDE` env toggle (the same
no-repaint path, forced on without needing a real child), then measuring the
parent under a forced fullscreen game — same ruler as Step 11:

| Parent `rondelek`, occluded by a fullscreen game (Wayland) | %CPU |
|---|---|
| Before — 100 ms heartbeat still pending | **92.2%** |
| After — no repaint requested while hidden | **0.7%** |

🟢 **Plain**: 92% → under 1%, measured the exact same way as the bug. The test
toggle was removed once it had done its job; the shipped code keys off the
app's own game child (`game_child.is_some()`), which is only true on the real
"launched from the app" path — the one that reproduced the bug.

🔬 **Technical**: No-repaint is the *only* lever that works here: a *slower*
heartbeat doesn't help, because a single pending repaint spins until a frame
callback that never arrives — interval is irrelevant while occluded (which is
why the 92% was the same shape at 100 ms as it would be at 1 s). The one
tradeoff is that reaping the child now depends on the compositor sending a
focus/occlusion event when the game closes (normal on standard Wayland); the
`try_wait()` runs on that returning frame. A timer-based reap is the fallback
if a compositor is ever found that doesn't refocus on close — deliberately not
built yet (`// ponytail:`), since no repaint is the whole point.

The lesson of Part two, in one line: **when the symptom appears, first ask
which *process* owns it — the loudest thing on screen is not always the one
burning the CPU.**

---

## The toolbox, in one table

| Tool | One-liner | Answers |
|---|---|---|
| `pidstat -p PID 1 N` | CPU per second, usr/system split | "how much, and code or kernel?" |
| `pidstat` on *each* live PID | measure the parent too, not just the accused | "which **process** owns the spin?" (Part two) |
| `ps -L -o tid,pcpu,comm -p PID` | per-thread CPU + names | "which thread, which library?" |
| `perf record -g` / `perf report` | sampling profiler (needs `perf_event_paranoid` ≤ 2) | "which functions?" |
| `strace -c -f CMD` | syscall census with error counts | "what does it ask the kernel, how often, how vainly?" |
| `strace -ff -e trace=... -o pfx CMD` | filtered per-thread traces | "which thread does the weird thing, exactly?" |
| `ls -l /proc/PID/fd`, `readlink` | fd inventory | "what is fd N, who talks to what?" |
| `cat /proc/sys/kernel/...` | kernel knobs | "why is my tool blocked?" |
| `env -u VAR CMD` | run minus one env var | backend/config A/B with zero rebuilds |
| null implementation | swap a feature for a stub | exact cost attribution by subtraction |

And the loop that ties them together: **measure → hypothesize → experiment
that could falsify → re-measure with the same ruler → repeat**. We were wrong
twice — the repaint theory in Step 5, and the whole accused process in Part
two — and each time measuring caught it in minutes; being wrong *without*
measuring costs releases.
