# Changelog

## 0.6.1 (2026-11-18)

Profile pictures stay put.

- **Profile pictures:** they no longer vanish after the list refreshes, which showed up as pictures disappearing when you scrolled or moved a channel.

## 0.6.0 (2026-11-18)

The app is now called Collector.

- **Name:** the window, the tray icon and the exe are named Collector. Your list, settings and sign-in carry over.
- **Updating from 0.5:** copies of 0.5 look for updates under the old name and won't find this one. Download 0.6.0 once by hand, put the new exe beside your `data` folder, and it updates itself from then on.

## 0.5.3 (2026-11-18)

Profile pictures beside channel names, the channels being watched marked in your list, activity by channel, a prompt before removing a channel, and smoother movement throughout.

- **Profile pictures:** each channel shows its profile picture in your list, in the followed list and in the watch slots. Until one has arrived, the channel's initial stands in.
- **Watching:** the channels being watched right now are marked in your list with a purple rule and the word **Watching**, so you can see which two the order has picked.
- **Activity by channel:** **Activity › By channel** lists what each channel brought in over the period: time watched, bonuses, streaks and points. Click a channel to see only its figures and timeline; **Show all channels** goes back.
- **Removing a channel:** the bin asks first. It turns into **Remove** and **Keep** in the row; Escape keeps the channel. **Undo** is still offered afterwards.
- **Smoother:** switching between views inside a page settles in like switching pages, notices and the sign-in code ease in, profile pictures fade in as they arrive, the Activity figures count to their new value, and buttons give a little when pressed.
- **Reordering:** the up and down arrows are gone. Drag a row by its grip, or focus the grip and press Up or Down.

## 0.5.2 (2026-11-18)

A new app icon that stands out in a file listing and the tray.

- **App icon:** a purple tile with white bars, replacing the near-black tile that blended into a dark background. The exe, the tray icon and the welcome tour all use it.

## 0.5.1 (2026-11-18)

Purple throughout, a clearer drag when you reorder channels, and one scroll bar instead of two.

- **Colours:** the accent on active slots and the streak marker is purple instead of amber, and so is the app icon.
- **Reordering:** a channel you pick up lifts off the list with a shadow and an outline, and the other rows glide apart to show where it will land. Dragging near the top or bottom of the window scrolls the list.
- **Scroll bars:** a long list no longer puts a second scroll bar on the window beside the list's own.

## 0.5.0 (2026-11-18)

The app now updates itself, says so when watching isn't earning points, and keeps a log file.

- **Updates:** once a day while it's open, the app checks for a newer version, downloads it in the background and installs it when you quit. **Settings › Updates › Restart and update** installs it right away. Turn off **Update automatically** and it only tells you, with a **Download** button. Every download is checked against the release's SHA-256 and its signature before it's used.
- **No points arriving:** if a channel has been watched for 15 minutes and no points have come in, the slot says **No points for 15 min** and a strip explains that something may have changed in how points are earned.
- **Watch time not getting through:** if the watch reports are refused for a channel, the slot says **Not getting through** and stays that way until they go through again. It no longer flips the whole app in and out of an error.
- **Needs a newer version:** when the service answers in a way this version doesn't understand, the app says so and offers **Check for updates**.
- **Log file:** what the app has done and anything that went wrong is written to `log.txt` in the `data` folder. **Settings › Help and quitting › Show in folder** opens it. It never holds your sign-in, your account name or request addresses.
- **Quit:** **Settings › Help and quitting › Quit** closes the app. Closing the window still only hides it.

## 0.4.1 (2026-11-18)

Channels you add show their balances within seconds, and an unknown balance no longer looks like zero.

- **New channels:** a channel that hasn't been checked yet is checked ahead of the regular round, a third of a second after the last one, instead of waiting up to a minute and then ten seconds a channel.
- **Not known yet:** until it has been checked, a channel reads **Checking…** and its points read **–**. A zero only appears when the balance really is zero.

## 0.4.0 (2026-11-18)

Smooth animations throughout.

- **Controls** fade between resting, hover and pressed.
- **Pages** settle in when you switch to them, and **rows** glide to their new places when you move, remove or add a channel.
- **Points** count up to their new value when they arrive.
- **Welcome tour:** the backdrop fades, each stop slides in from the side it comes from, and the step pill slides along the dots.
- **Reduced motion:** with Windows set to show fewer animations, everything is instant.

## 0.3.0 (2026-11-18)

A welcome tour on first start, the window opens when you launch the app, and the tray view is simpler.

- **Welcome tour:** five stops walk through what the app is, signing in, adding the channels you follow, what watching does, and where the app lives. **Settings › Welcome tour › Show again** replays it.
- **Launching:** starting the app opens the window. Started by Windows at sign-in, it stays in the tray.
- **Tray view:** today's points, the state line, Pause, the two slots and the queue, with one button to open the window.

## 0.2.0 (2026-11-18)

A Watching page in the window, your followed channels, a search, and more.

- **Watching page:** the window shows what the tray view shows: today's points, Pause, the slots and the queue.
- **Followed channels:** Channels lists the channels you follow that aren't in your list, with **Add** per row and **Add all**.
- **Search:** find a channel in a long list and move it to the top.

## 0.1.0 (2026-11-17)

The first build: a tray app that earns channel points on the channels you choose.

- **Watching:** reports watch time for the two highest channels in your list that are live, without playing the stream.
- **Bonuses and streaks:** claims the bonus chest as soon as it's offered, and puts a channel with a watch streak due ahead of the others.
- **Activity:** points earned, bonuses claimed and streaks collected, by day.
- **Sign-in:** a device code you approve in your browser. The app never sees your password, and the sign-in is kept in Windows Credential Manager.
- **Portable:** one folder, nothing installed. Everything the app writes is in the `data` folder beside it.
