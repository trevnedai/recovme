# What's New in RecovMe

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
