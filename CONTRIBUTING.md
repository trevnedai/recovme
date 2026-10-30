# Contributing to RecovMe

Thank you for helping. RecovMe is a free, open-source file and photo recovery app for Mac and
Windows, made by Dhan Denvert Asuncion. Every kind of help counts, and you don't need to write
code to make a difference.

## The most useful help: recovery reports

Recovery depends on the device. Reports from real cameras, dashcams, CCTV recorders, phones and
drives tell me what works and what doesn't. Open a
[recovery report](https://github.com/trevnedai/recovme/issues/new?template=recovery_report.yml)
with the device, its file system, what happened to the files and what came back. Reports where
recovery failed are just as valuable.

## Bugs and ideas

- **Bug:** open a [bug report](https://github.com/trevnedai/recovme/issues/new?template=bug_report.yml)
  with your RecovMe version (shown at the bottom of the sidebar), your system and the steps.
- **Idea:** open a [feature request](https://github.com/trevnedai/recovme/issues/new?template=feature_request.yml).
- **Security problem:** don't open a public issue; see [SECURITY.md](SECURITY.md).

## Code

1. Read the [README](README.md) for how RecovMe works and how to build it.
2. For anything bigger than a small fix, open an issue first so we can agree on the approach.
3. Keep changes focused, and test on the platform you changed (Mac or Windows).
4. RecovMe must stay **read-only** on the drive being scanned and must never save onto it. Changes
   that weaken this won't be merged.

## Website and guides

The website lives in `site/`. Guides, comparisons and the About, Privacy and Terms pages are
generated: edit the text in `site-src/`, run `npm run site`, and commit both the source and the
result. Facts about other products must link to the maker's own page and carry the date they were
checked.

## Translations

The website is in English and Italian. If you'd like to review the Italian pages or add a
language, open an issue first.

## License

By contributing, you agree that your contribution is licensed under the
[GNU GPL, version 3 or later](LICENSE), with the additional terms in [NOTICE.md](NOTICE.md).
There is no contributor agreement to sign.

If you publish a modified version, give it its own name and icon: the RecovMe name and logo are
reserved for the official app (see [TRADEMARKS.md](TRADEMARKS.md)).

Everyone taking part is expected to follow the [Code of Conduct](CODE_OF_CONDUCT.md).
