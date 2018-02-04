# xidlehook

*Because xautolock is annoying to work with.*

**Warning:** This is a really new program, and I've already found (and fixed) a ton of subtle bugs.  
I'm not saying it's not going to work, but it might be a little buggy.  
Just don't judge it too hard during the first few days, will you?

xidlehook is a general-purpose replacement for xautolock.  
It basically just executes a command when the computer has been idle for \_ minutes.

Improvments over xautolock:
 - As well as a notifier, there is now a canceller, for when the user stops it from locking.
 - Not specific to locking. Multiple instances can run at the same time.
 - Optional communication using unix sockets to allow activating and deactivating it.
 - Optionally prevent locking when an application is fullscreen.

Missing features:
 - Magic corners
 - Killer stuff
 - Bell, because you should be using --notify
 - All the instance related stuff (-activate, -restart, -exit, etc)

# Example

Here's a lock using i3lock, with screen dim support.

```Bash
xidlehook \
  --time 5 \
  --timer 'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness 1; i3lock' \
  --notify 10 \
  --notifier  'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness .1' \
  --canceller 'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness 1' \
  --not-when-fullscreen
```

Note: Every command is passed through `sh -c`, so you should be able to mostly use normal syntax.

# Installation

Installation using `cargo`:

```
cargo install xidlehook
```

Or if you're on Arch Linux and prefer using the AUR:

[AUR package](https://aur.archlinux.org/packages/xidlehook/)

Or if you want to clone it:

```
git clone https://github.com/jD91mZM2/xidlehook
cd xidlehook
cargo build --release
```

## Too bloaty?

Does this application have too many dependencies for your taste?  
You can disable a few with `--no-default-features`.

# Socket API

The socket API is very simple. Each packet is a single byte.

**Note**: The socket API requires the "tokio" feature.

| Byte | Command                   |
|------|---------------------------|
| 0x0  | Deactivate                |
| 0x1  | Activate                  |
| 0x2  | Trigger the timer command |
