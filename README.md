# xidlehook

*Because xautolock is annoying to work with.*

xidlehook is a general-purpose replacement for xautolock.  
It basically just executes a command when the computer has been idle for \_ minutes.

Improvments over xautolock:
 - As well as a notifier, there is now a canceller, for when the user stops it from locking.
 - Not specific to locking. Multiple instances can run at the same time.

Missing features:
 - Magic corners
 - Killer stuff
 - Bell, because you should be using --notify
 - All the instance related stuff (-activate, -restart, -exit, etc)

Planned features:
 - Optionally prevent locking when an application is fullscreen.
 - Optional communication using unix sockets to allow activating and deactivating it.

# Example

Here's a lock using i3lock, with screen dim support.

```Bash
xidlehook \
  --time 5 \
  --timer "i3lock" \
  --notify 10 \
  --notifier  'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness .1' \
  --canceller 'xrandr --output "$(xrandr | grep primary | cut -d " " -f 1)" --brightness 1'
```

# Installation

Clone and compile it yourself.

[AUR package](https://aur.archlinux.org/packages/xidlehook/)
