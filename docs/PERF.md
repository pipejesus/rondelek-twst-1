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

## The toolbox, in one table

| Tool | One-liner | Answers |
|---|---|---|
| `pidstat -p PID 1 N` | CPU per second, usr/system split | "how much, and code or kernel?" |
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
once (Step 5) and it cost ten minutes; being wrong without measuring costs
releases.
