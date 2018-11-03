# xidlehook

*Because xautolock is annoying to work with.*

xidlehook is a general-purpose replacement for xautolock. It executes a command
when the computer has been idle for a specified amount of time.

Improvements over xautolock:
 - Allows "cancellers" which undo a timer action when new activity is detected.
 - Not specific to locking. Multiple instances can run at the same time.
 - Optionally only run once.
 - Optionally prevent locking when an application is fullscreen.
 - Optionally prevent locking when any application plays audio.

Missing features:
 - Magic corners.
 - Bell, because you should be using --notify.
 - All the instance related stuff (you should use unix sockets with --socket).

# Example

Here's a lock using i3lock, with screen dim support:

```Bash
xidlehook \
  `# Don't lock when there's a fullscreen application` \
  --not-when-fullscreen \
  `# Don't lock when there's audio playing` \
  --not-when-audio \
  `# Dim the screen after 60 seconds, undim if user becomes active` \
  --timer normal 60 \
    'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness .1'  \
    'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness 1' \
  `# Undim & lock after 10 more seconds` \
  --timer primary 10 'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness 1; i3lock' "" \
  `# Finally, suspend an hour after it locks` \
  --timer normal 3600 'systemctl suspend'
```

Note: Every command is passed through `sh -c`, so you should be able to mostly use normal syntax.

# Installation

Installation using `cargo`:

```
cargo install xidlehook
```

It's also available on Nix and the [AUR (not officially maintained)](https://aur.archlinux.org/packages/xidlehook/).

Or if you want to clone it manually:

```
git clone https://gitlab.com/jD91mZM2/xidlehook
cd xidlehook
cargo build --release
```

## Too bloaty?

Are you not using pulseaudio?  
You can disable that requirement completely with `--no-default-features`!  
This however gets rid of --not-when-audio.

# Socket API

The socket API is very simple. Each command is a single byte, sent over a unix
socket. Bind a file using `--socket /path/to/xidlehook.sock` (where the path is
whatever you want), and then you can send one of the following:

| Byte | Command                           |
| ---  | ---                               |
| 0    | Deactivate                        |
| 1    | Activate                          |
| 2    | Run the timer command immediately |

A common use case of `xidlehook` is using it to run a lockscreen. To then
manually lock the screen, you could bind this bash command to a shortcut:

```Bash
echo -ne "\x2" | socat - UNIX-CONNECT:/path/to/xidlehook.sock
```
