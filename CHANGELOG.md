# What's New in RecovMe

## 0.3.1

- The update bar has a **What's New** link, so you can read what changed before
  updating.
- The first release delivered by automatic update: if you have 0.3.0, RecovMe
  offers this version by itself.

## 0.3.0 — Automatic Updates

RecovMe now keeps itself up to date. When a new version is out, a bar says
so, and **Update and Restart** downloads it, installs it and opens RecovMe
again. It never interrupts a scan.

- Every update is signed, and RecovMe refuses any download that isn't signed
  with its own key.
- The check only asks GitHub whether a newer version exists. Your files never
  leave your computer. Turn it off under **Check for Updates** at the bottom of
  the sidebar.
- If you have 0.1.0 or 0.2.0, install 0.3.0 by hand once: those versions can't
  update themselves. From 0.3.0 on, updates are automatic.

### Genuine copies
- Get RecovMe only from recovme.app or its GitHub Releases page. Each release now
  lists `SHA256SUMS.txt` so you can check a download is the official one, and the
  app links to recovme.app at the bottom of the sidebar.
- The name, logo and icon are RecovMe's own: modified versions must use their own
  name and say they are not the official RecovMe (see NOTICE.md and TRADEMARKS.md).

## 0.2.0 — Phones and Tablets

RecovMe can now help with Android phones and iPhone or iPad backups, in the
same app, under **Phones & Tablets** in the sidebar.

**Please read this first.** A phone can't be searched block by block like a
memory card. iPhones never allow it, and Android phones encrypt their storage
and wipe the space of deleted files. So RecovMe only brings back files that
still exist somewhere. Files that are fully deleted from a phone can't be
brought back by RecovMe or by any other app. The app says this before every
first phone scan, and lists what it can and can't bring back.

### Android (USB debugging)
Copies, without changing anything on the phone:
- the system trash: photos and videos deleted in about the last 30 days, with
  their original names put back
- the trash of Samsung Gallery and My Files (back in their original folder),
  Xiaomi, Huawei, OPPO, OnePlus, vivo and Files by Google
- photos and downloads that never finished saving
- Telegram's cache (everything viewed in chats, even if never saved), and media
  from WhatsApp (statuses included), Telegram, Viber and other chat apps
- thumbnail previews (mostly useful on phones older than Android 10)
- optionally, everything in the phone's own folders

Files without an extension are recognised by their content.

### iPhone and iPad (backups on this computer)
Reads Finder, iTunes and Apple Devices backups, encrypted ones too (the
password is never saved), without changing them:
- Camera Roll with original names; photos that were in Recently Deleted and the
  Hidden album each get their own folder
- previews of photos kept only in iCloud, when the backup has one
- messages that were in Messages → Recently Deleted (iOS 16 and later), as a
  text file, with their photos
- Messages and WhatsApp attachments, voice memos with their titles, Notes
  attachments and app documents

Before making a new backup, archive the old one (Manage Backups → right-click
→ Archive): a new backup replaces it.

## 0.1.0
First release: Quick Scan and Deep Scan for cards, USB sticks and drives.
