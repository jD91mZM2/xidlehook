# xidlehook

*Because xautolock is annoying to work with.*

xidlehook is a general-purpose replacement for xautolock.  
It basically just executes a command when the computer has been idle for \_ minutes.

Improvements over xautolock:
 - As well as a notifier, there is now a canceller, for when the user stops it from locking.
 - Not specific to locking. Multiple instances can run at the same time.
 - Optionally only run once.
 - Optionally prevent locking when an application is fullscreen.
 - Optionally prevent locking when any application plays audio.

Missing features:
 - Magic corners.
 - --killer stuff whatever that is.
 - Bell, because you should be using --notify.
 - All the instance related stuff (you should use unix sockets with --socket).

*Note: xidlehook's timer is only accurate to the minute and may very well execute 59 seconds later than it's supposed to.
However, the delay between notifier and timer is accurate to the second.
See [#6](https://github.com/jD91mZM2/xidlehook/issues/6) for technical details.*

# Example

Here's a lock using i3lock, with screen dim support:

```Bash
xidlehook \
  --time 5 \
  --timer 'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness 1; i3lock' \
  --notify 10 \
  --notifier  'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness .1' \
  --canceller 'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness 1' \
  --not-when-fullscreen \
  --not-when-audio
```

Note: Every command is passed through `sh -c`, so you should be able to mostly use normal syntax.

For an example in practice,
[here's what I use personally](https://github.com/jD91mZM2/.dotfiles/blob/master/lock-timer.sh).

# Installation

Installation using `cargo`:

```
cargo install xidlehook
```

Or if you're on Arch Linux and prefer using the AUR:

[AUR package](https://aur.archlinux.org/packages/xidlehook/)

Or if you're using Nix:

```
nix-env -iA nixpkgs.xidlehook
```

Or if you want to clone it manually:

```
git clone https://github.com/jD91mZM2/xidlehook
cd xidlehook
cargo build --release
```

## Too bloaty?

Are you not using pulseaudio?  
You can disable that requirement completely with `--no-default-features`!  
This however gets rid of --not-when-audio.

# Socket API

The socket API is very simple. Each packet is a single byte.

| Byte | Command                   |
|------|---------------------------|
| 0x0  | Deactivate                |
| 0x1  | Activate                  |
| 0x2  | Trigger the timer command |

For example, if you wanted to lock the screen, you could bind the following to a keyboard shortcut:

```Bash
echo -ne "\x2" | socat - UNIX-CONNECT:/path/to/xidlehook.sock
```
