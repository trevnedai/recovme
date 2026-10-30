# Package manager files

Ready-to-submit files that let people install RecovMe with one command. Update the version,
URL and SHA-256 (from the release's `SHA256SUMS.txt`) for each new release.

## Windows: winget

`winget/manifests/d/DhanDenvertAsuncion/RecovMe/<version>/` holds the three manifest files
for [microsoft/winget-pkgs](https://github.com/microsoft/winget-pkgs). No popularity minimum is
required. Before submitting:

1. Scan the installer on VirusTotal: any antivirus warning means rejection. A SignPath-signed
   installer is the safest to submit.
2. On a Windows PC: `winget validate --manifest <folder>` and a test install with
   `winget install --manifest <folder>`.
3. Submit with `wingetcreate submit <folder>` (or fork winget-pkgs, copy the folder to the same
   path and open a pull request). Sign Microsoft's contributor agreement when the bot asks.

After it's merged: `winget install DhanDenvertAsuncion.RecovMe`.
For later versions: `wingetcreate update DhanDenvertAsuncion.RecovMe --version <v> --urls <installer URL> --submit`.

## Mac: Homebrew tap

Homebrew's official cask list needs 225 stars (or 75 if someone else submits it) and a repository
at least 30 days old, so start with a personal tap:

1. Create a public GitHub repository named **homebrew-tap** under trevnedai.
2. Copy `homebrew-tap/Casks/recovme.rb` into it as `Casks/recovme.rb`.
3. People install with `brew install --cask trevnedai/tap/recovme`.

`brew audit --cask --strict trevnedai/tap/recovme` checks the file. RecovMe updates itself
(`auto_updates true`), so the cask only needs updating when you want new installs to get the
newest version. When submitting to Homebrew's official list later, disclose any AI help in the
pull request and leave AI co-author lines out of those commits: Homebrew's rules require it.
