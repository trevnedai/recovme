cask "recovme" do
  version "0.3.1"
  sha256 "e53fd5bcfe93982f2ee1945ba53000c007045f4190c7281acd5bc030722a843f"

  url "https://github.com/trevnedai/recovme/releases/download/v#{version}/RecovMe_#{version}_aarch64.dmg",
      verified: "github.com/trevnedai/recovme/"
  name "RecovMe"
  desc "File and photo recovery for cards, drives, phones and backups"
  homepage "https://recovme.app/"

  livecheck do
    url :url
    strategy :github_latest
  end

  auto_updates true
  depends_on arch: :arm64
  depends_on macos: ">= :big_sur"

  app "RecovMe.app"

  zap trash: [
    "~/Library/Application Support/app.recovme.recovery",
    "~/Library/Caches/app.recovme.recovery",
    "~/Library/Preferences/app.recovme.recovery.plist",
    "~/Library/Saved Application State/app.recovme.recovery.savedState",
    "~/Library/WebKit/app.recovme.recovery",
  ]
end
